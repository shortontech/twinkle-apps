//! Public chart API projections. These functions never consume random numbers or
//! modify the simulation. Every chart retains exact rows for inspection/tables.
use crate::simulation::{CATEGORIES, OUTCOMES, Simulation, TIERS};
use std::{sync::Arc, time::Instant};
use twinkle_charts::*;
use twinkle_ui::{
    Rect,
    retained_surface::{RetainedSurfaceInput, RetainedSurfaceRevisions},
};

pub const PANEL_BOUNDS: Rect = Rect::new(0.0, 0.0, 780.0, 410.0);
pub fn limits() -> ChartLimits {
    ChartLimits {
        max_rows: 30_000,
        max_series: 256,
        max_layers: 8,
        max_geometry_vertices: 300_000,
        max_ticks_per_axis: 256,
        max_total_label_bytes: 8 * 1024 * 1024,
        max_steady_bytes: 128 * 1024 * 1024,
        max_peak_bytes: 256 * 1024 * 1024,
        ..ChartLimits::default()
    }
}
fn num(v: impl Into<f64>) -> DataValue {
    DataValue::Number(v.into())
}
fn cat(v: impl Into<Arc<str>>) -> DataValue {
    DataValue::Category(v.into())
}
fn column(name: &str, values: Vec<DataValue>) -> Column {
    Column::new(name, values)
}
fn data(revision: u64, datasets: Vec<Dataset>) -> Result<Arc<ChartData>, ChartError> {
    Ok(Arc::new(ChartData::new(revision, datasets, &limits())?))
}
fn field(dataset: &str, name: &str) -> FieldRef {
    FieldRef::new(dataset, name)
}
fn axis(scale: &str, title: &str, ticks: usize) -> Axis {
    Axis {
        scale: scale.into(),
        title: Some(title.into()),
        unit: None,
        tick_count: Some(ticks),
        grid: false,
    }
}
fn domain(spec: &mut ChartSpec, name: &str, bounds: [f64; 2]) {
    let scale = spec
        .scales
        .iter_mut()
        .find(|s| s.name.as_ref() == name)
        .expect("recipe scale");
    scale.domain = Some(bounds);
    scale.clamp = true;
}
fn band(spec: &mut ChartSpec, name: &str, reversed: bool) {
    let scale = spec
        .scales
        .iter_mut()
        .find(|s| s.name.as_ref() == name)
        .unwrap();
    scale.kind = ScaleKind::Band {
        inner_padding: 0.0,
        outer_padding: 0.0,
    };
    scale.reversed = reversed;
}

#[derive(Clone, Copy, Default)]
pub struct HeatOptions {
    pub category: Option<usize>,
    pub detail: bool,
}

/// Immutable panel snapshots and their exact dataset IDs for table projection.
pub struct Projection {
    pub data: Vec<Arc<ChartData>>,
    pub specs: Vec<Arc<ChartSpec>>,
    pub table_ids: Vec<Arc<str>>,
    pub rows: [usize; 4],
    pub changed_heat_cells: Option<usize>,
}
impl Projection {
    /// Compare actual values at every visible cell position, including the
    /// oldest column that has just left the simulation's bounded history.
    pub fn compare_previous(&mut self, previous: &Self) {
        let current = &self.data[1]
            .dataset("heat")
            .unwrap()
            .column("requests_per_second")
            .unwrap()
            .values;
        let old = &previous.data[1]
            .dataset("heat")
            .unwrap()
            .column("requests_per_second")
            .unwrap()
            .values;
        self.changed_heat_cells = (current.len() == old.len()).then(|| {
            current
                .iter()
                .zip(old.iter())
                .filter(|(a, b)| a != b)
                .count()
        });
    }
    pub fn build(s: &Simulation, revision: u64, options: HeatOptions) -> Result<Self, ChartError> {
        let mut keys = Vec::with_capacity(1800);
        let mut age = Vec::with_capacity(1800);
        let mut outcome = Vec::with_capacity(1800);
        let mut rate = Vec::with_capacity(1800);
        let mut tick = Vec::with_capacity(1800);
        for b in &s.traffic {
            for (i, name) in OUTCOMES.iter().enumerate() {
                keys.push(RowKey::from(format!("{}:{i}", b.tick)));
                age.push(num((b.tick - s.tick) as f64 / 10.0));
                tick.push(num(b.tick as f64));
                outcome.push(cat(*name));
                rate.push(num((b.outcomes[i] * 10) as f64));
            }
        }
        let traffic = data(
            revision,
            vec![Dataset::new(
                "traffic",
                keys,
                vec![
                    column("age_seconds", age),
                    column("tick", tick),
                    column("outcome", outcome),
                    column("requests_per_second", rate),
                ],
            )?],
        )?;
        let mut traffic_spec = Recipes::line(
            "Request traffic · 60 s",
            field("traffic", "age_seconds"),
            field("traffic", "requests_per_second"),
        );
        traffic_spec.layers[0].encodings.push(Encoding {
            channel: Channel::Series,
            field: field("traffic", "outcome"),
            scale: Some("outcomes".into()),
        });
        traffic_spec.scales.push(Scale {
            name: "outcomes".into(),
            kind: ScaleKind::Color,
            domain: None,
            clamp: true,
            reversed: false,
            unit: None,
        });
        traffic_spec.legends.push(Legend {
            scale: "outcomes".into(),
            title: Some("Requests/sec".into()),
            toggleable: true,
        });
        domain(&mut traffic_spec, "x", [-60.0, 0.0]);
        let capacity = s.services.iter().map(|v| v.capacity).sum::<u64>() as f64;
        domain(
            &mut traffic_spec,
            "y",
            [0.0, (capacity * 1.25 / 10000.0).ceil() * 10000.0],
        );
        traffic_spec.axes = vec![axis("x", "Seconds ago", 5), axis("y", "Requests/sec", 5)];

        let mut heat_keys = Vec::new();
        let mut x = Vec::new();
        let mut y = Vec::new();
        let mut intensity = Vec::new();
        let mut raw = Vec::new();
        let mut capacities = Vec::new();
        let mut ticks = Vec::new();
        let mut members = Vec::new();
        let mut categories = vec![Vec::new(); 6];
        let group_labels = (0..s.preset.groups())
            .map(|g| Arc::<str>::from(format!("G{g:03}")))
            .collect::<Vec<_>>();
        let member_labels = (0..s.preset.groups())
            .map(|g| {
                Arc::<str>::from(
                    s.services[g * s.preset.group_size()..(g + 1) * s.preset.group_size()]
                        .iter()
                        .map(|v| v.label())
                        .collect::<Vec<_>>()
                        .join(", "),
                )
            })
            .collect::<Vec<_>>();
        // Detail is a current service-group × category matrix; benchmark fixes
        // the dense rolling matrix. Numeric columns preserve every exact count.
        let buckets: Vec<_> = if options.detail {
            s.heat.iter().rev().take(1).collect()
        } else {
            s.heat.iter().collect()
        };
        for b in buckets {
            for (g, values) in b.groups.iter().enumerate() {
                let passes = if options.detail && options.category.is_none() {
                    6
                } else {
                    1
                };
                for index in 0..passes {
                    let c = if options.detail {
                        options.category.unwrap_or(index)
                    } else {
                        index
                    };
                    let count = if options.detail {
                        values[c]
                    } else {
                        options
                            .category
                            .map_or_else(|| values.iter().sum(), |c| values[c])
                    };
                    heat_keys.push(RowKey::from(format!("{}:{g}:{c}", b.tick)));
                    x.push(cat(if options.detail {
                        Arc::from(CATEGORIES[c])
                    } else {
                        Arc::from(format!(
                            "-{:02}.{}s",
                            (s.tick - b.tick) / 10,
                            (s.tick - b.tick) % 10
                        ))
                    }));
                    y.push(cat(group_labels[g].clone()));
                    let cap = s.group_capacity(g);
                    raw.push(num((count * 10) as f64));
                    capacities.push(num(cap as f64));
                    intensity.push(num(count as f64 * 1000.0 / cap as f64));
                    ticks.push(num(b.tick as f64));
                    members.push(cat(member_labels[g].clone()));
                    for j in 0..6 {
                        categories[j].push(num((values[j] * 10) as f64));
                    }
                }
            }
        }
        let heat_rows = heat_keys.len();
        let mut columns = vec![
            column("age", x),
            column("group", y),
            column("severity_percent", intensity),
            column("requests_per_second", raw),
            column("capacity", capacities),
            column("tick", ticks),
            column("members", members),
        ];
        for (i, values) in categories.into_iter().enumerate() {
            columns.push(column(CATEGORIES[i], values));
        }
        let heat = data(revision, vec![Dataset::new("heat", heat_keys, columns)?])?;
        let mut heat_spec = Recipes::heatmap(
            if options.detail {
                "Threat categories · service groups"
            } else {
                "Threat activity · last 10 s"
            },
            field("heat", "age"),
            field("heat", "group"),
            field("heat", "severity_percent"),
        );
        band(&mut heat_spec, "x", false);
        band(&mut heat_spec, "y", false);
        domain(&mut heat_spec, "color", [0.0, 100.0]);
        heat_spec.axes = vec![
            axis(
                "x",
                if options.detail {
                    "Threat category"
                } else {
                    "Seconds ago"
                },
                5,
            ),
            axis("y", "Service group", 10),
        ];
        heat_spec.legends.push(Legend {
            scale: "color".into(),
            title: Some("Suspicious traffic (% of capacity)".into()),
            toggleable: false,
        });

        let mut tier_datasets = Vec::new();
        let mut bubble_spec = Recipes::bubble(
            "Service risk · affected hosts",
            field("tier0", "requests_per_second"),
            field("tier0", "threat_score"),
            field("tier0", "hosts"),
        );
        let base_layer = bubble_spec.layers[0].clone();
        bubble_spec.layers.clear();
        for (tier, tier_name) in TIERS.iter().enumerate() {
            let id = format!("tier{tier}");
            let mut keys = Vec::new();
            let mut x = Vec::new();
            let mut risk = Vec::new();
            let mut hosts = Vec::new();
            let mut names = Vec::new();
            let mut tiers = Vec::new();
            for v in s.services.iter().filter(|v| v.tier() == tier) {
                keys.push(RowKey::from(format!("service:{}", v.id)));
                x.push(num((v.requests * 10) as f64));
                risk.push(num(v.score_milli as f64 / 1000.0));
                hosts.push(num(v.hosts as f64));
                names.push(cat(v.label()));
                tiers.push(cat(*tier_name));
            }
            tier_datasets.push(Dataset::new(
                id.clone(),
                keys,
                vec![
                    column("requests_per_second", x),
                    column("threat_score", risk),
                    column("hosts", hosts),
                    column("service", names),
                    column("tier", tiers),
                ],
            )?);
            let mut layer = base_layer.clone();
            for encoding in &mut layer.encodings {
                encoding.field.dataset = id.clone().into();
            }
            bubble_spec.layers.push(layer);
        }
        let bubble = data(revision, tier_datasets)?;
        domain(
            &mut bubble_spec,
            "x",
            [0.0, 10_000.0 + (s.preset.group_size() - 1) as f64 * 2500.0],
        );
        domain(&mut bubble_spec, "y", [0.0, 100.0]);
        domain(&mut bubble_spec, "size", [0.0, 200.0]);
        bubble_spec.axes = vec![
            axis("x", "Requests/sec", 5),
            axis("y", "Threat score (0–100)", 5),
        ];

        let ranked = s.ranked_sources();
        let mut keys = Vec::new();
        let mut labels = Vec::new();
        let mut counts = Vec::new();
        let mut targets = Vec::new();
        let mut cats = Vec::new();
        for source in ranked {
            keys.push(RowKey::from(format!("source:{}", source.id)));
            labels.push(cat(source.label()));
            counts.push(num(source.rolling_count as f64));
            targets.push(cat(s.services[source.service].label()));
            cats.push(cat(CATEGORIES[source.category]));
        }
        let sources = data(
            revision,
            vec![Dataset::new(
                "sources",
                keys,
                vec![
                    column("source", labels),
                    column("requests", counts),
                    column("latest_target", targets),
                    column("latest_category", cats),
                ],
            )?],
        )?;
        let mut source_spec = Recipes::horizontal_bar(
            "Attack sources · top 20 / 10 s",
            field("sources", "source"),
            field("sources", "requests"),
        );
        for (rank, source) in s.ranked_sources().iter().enumerate() {
            source_spec.annotations.push(Annotation::Text {
                text: source.rolling_count.to_string().into(),
                x: source.rolling_count as f64,
                y: rank as f64,
            });
        }
        domain(&mut source_spec, "x", [0.0, 4000.0]);
        band(&mut source_spec, "y", false);
        source_spec.axes = vec![
            axis("x", "Suspicious requests in last 10 s", 5),
            axis("y", "Source", 20),
        ];
        Ok(Self {
            data: vec![traffic, heat, bubble, sources],
            specs: vec![
                Arc::new(traffic_spec),
                Arc::new(heat_spec),
                Arc::new(bubble_spec),
                Arc::new(source_spec),
            ],
            table_ids: vec![
                "traffic".into(),
                "heat".into(),
                "tier0".into(),
                "sources".into(),
            ],
            rows: [1800, heat_rows, s.preset.services(), 20],
            changed_heat_cells: None,
        })
    }
}

pub fn input(revision: u64, bounds: Rect, scale: f32) -> RetainedSurfaceInput {
    RetainedSurfaceInput::new(
        bounds,
        scale,
        RetainedSurfaceRevisions {
            content: revision,
            ..Default::default()
        },
    )
}
fn panel_tooltip(panel: usize) -> ChartTooltipSpec {
    use ChartTooltipFormat::{Automatic, Number};
    let fields: Vec<(&str, &str, Option<&str>, ChartTooltipFormat)> = match panel {
        0 => vec![
            ("outcome", "Outcome", None, Automatic),
            (
                "age_seconds",
                "Time",
                Some("s"),
                Number { decimal_places: 1 },
            ),
            (
                "requests_per_second",
                "Traffic",
                Some("requests/s"),
                Number { decimal_places: 0 },
            ),
        ],
        1 => vec![
            ("group", "Service group", None, Automatic),
            ("age", "Time / category", None, Automatic),
            (
                "severity_percent",
                "Capacity used",
                Some("%"),
                Number { decimal_places: 1 },
            ),
            (
                "requests_per_second",
                "Suspicious traffic",
                Some("requests/s"),
                Number { decimal_places: 0 },
            ),
        ],
        2 => vec![
            ("service", "Service", None, Automatic),
            ("tier", "Tier", None, Automatic),
            (
                "threat_score",
                "Threat score",
                None,
                Number { decimal_places: 1 },
            ),
            (
                "hosts",
                "Affected hosts",
                None,
                Number { decimal_places: 0 },
            ),
            (
                "requests_per_second",
                "Traffic",
                Some("requests/s"),
                Number { decimal_places: 0 },
            ),
        ],
        _ => vec![
            ("source", "Source", None, Automatic),
            (
                "requests",
                "Suspicious requests",
                None,
                Number { decimal_places: 0 },
            ),
            ("latest_target", "Latest target", None, Automatic),
            ("latest_category", "Threat category", None, Automatic),
        ],
    };
    ChartTooltipSpec {
        fields: fields
            .into_iter()
            .map(|(column, label, unit, format)| ChartTooltipField {
                column: column.into(),
                label: label.into(),
                unit: unit.map(Into::into),
                format,
            })
            .collect(),
        include_data_table_hint: false,
    }
}

pub fn create_views(p: &Projection) -> Result<Vec<ChartView>, ChartError> {
    p.specs
        .iter()
        .zip(&p.data)
        .enumerate()
        .map(|(panel, (spec, data))| {
            let mut theme = ChartTheme::dark();
            if panel == 3 {
                theme.typography.label = 10.0;
            }
            ChartView::new(
                ChartModel::new(spec.clone(), data.clone(), limits())?
                    .with_theme(theme)?
                    .with_tooltip(panel_tooltip(panel))?,
            )
        })
        .collect()
}
pub fn prepare(
    views: &mut [ChartView],
    p: &Projection,
    revision: u64,
) -> Result<[u64; 4], ChartError> {
    let mut times = [0; 4];
    for (i, view) in views.iter_mut().enumerate() {
        let previous = view.snapshot().input;
        let start = Instant::now();
        view.replace_and_update(
            p.specs[i].clone(),
            p.data[i].clone(),
            input(
                revision,
                previous.map_or(PANEL_BOUNDS, |v| v.bounds),
                previous.map_or(1.0, |v| v.scale_factor),
            ),
        )?;
        times[i] = start.elapsed().as_micros() as u64;
    }
    // The caller publishes its revision only after this whole function succeeds.
    // If any panel rejects, fail-stop and do not publish the partial generation.
    Ok(times)
}

pub fn metrics(
    view: &ChartView,
    panel: usize,
    preset: crate::simulation::Preset,
) -> serde_json::Value {
    let d = view.diagnostics();
    let paint = &view.snapshot().output.paint;
    let plot = view.plot_bounds();
    use twinkle_ui::backend::PaintCommand;
    serde_json::json!({"input_rows":d.input_rows,"transformed_rows":d.transformed_rows,"displayed_rows":d.displayed_rows,"geometry_vertices":d.geometry_vertices,"paint_commands":d.paint_commands,"paths":paint.iter().filter(|p|matches!(p,PaintCommand::Path{..})).count(),"rectangles_including_background":paint.iter().filter(|p|matches!(p,PaintCommand::Fill{..})).count(),"rounded_fills_including_controls":paint.iter().filter(|p|matches!(p,PaintCommand::RoundedFill{..})).count(),"plot_logical_size":plot.map(|p|[p.size.width,p.size.height]),"heat_cell_logical_size":if panel==1{plot.map(|p|[p.size.width/100.0,p.size.height/preset.groups() as f32])}else{None},"steady_bytes":d.steady_bytes,"peak_bytes":d.peak_bytes,"notices":d.notices.iter().map(|n|format!("{n:?}")).collect::<Vec<_>>()})
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulation::Preset;
    #[test]
    fn all_four_visible_datasets_change_each_tick_in_quiet_and_attack_phases() {
        use crate::simulation::Preset;
        let values = |p: &Projection| {
            let names = [
                "requests_per_second",
                "requests_per_second",
                "requests_per_second",
                "requests",
            ];
            (0..4)
                .map(|i| {
                    p.data[i]
                        .datasets()
                        .iter()
                        .flat_map(|d| d.column(names[i]).unwrap().values.iter().cloned())
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>()
        };
        let mut sim = Simulation::new(42, Preset::Standard);
        for phase in [0, 1500] {
            while sim.tick < phase {
                sim.step();
            }
            let mut old = values(&Projection::build(&sim, 1, HeatOptions::default()).unwrap());
            for revision in 2..52 {
                sim.step();
                let next =
                    values(&Projection::build(&sim, revision, HeatOptions::default()).unwrap());
                for panel in 0..4 {
                    assert_ne!(
                        old[panel], next[panel],
                        "unchanged panel {panel} at tick {}",
                        sim.tick
                    );
                }
                old = next;
            }
        }
    }
    #[test]
    fn every_preset_admits_four_changing_panels_and_preserves_keys() {
        for preset in [Preset::Standard, Preset::Dense, Preset::Stress] {
            let mut sim = Simulation::new(42, preset);
            let checksum = sim.checksum();
            let p = Projection::build(&sim, 1, HeatOptions::default()).unwrap();
            assert_eq!(p.rows, [1800, 100 * preset.groups(), preset.services(), 20]);
            assert_eq!(sim.checksum(), checksum);
            let mut views = create_views(&p).unwrap();
            prepare(&mut views, &p, 1).unwrap();
            for v in &views {
                assert!(!v.snapshot().output.paint.is_empty());
                assert!(!v.hits().is_empty());
            }
            let old_source_keys = p.data[3].dataset("sources").unwrap().keys().to_vec();
            sim.step();
            let mut q = Projection::build(&sim, 2, HeatOptions::default()).unwrap();
            q.compare_previous(&p);
            prepare(&mut views, &q, 2).unwrap();
            assert!(q.changed_heat_cells.unwrap() > 0);
            for v in &views {
                assert_eq!(v.snapshot().input.unwrap().revisions.content, 2);
            }
            assert!(old_source_keys.iter().all(|k| k.0.starts_with("source:")));
            for v in &views[2..3] {
                assert!(v.hits().iter().all(|h| h.row.0.starts_with("service:")));
            }
        }
    }
}
