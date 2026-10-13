use std::{
    cell::RefCell,
    error::Error,
    rc::Rc,
    time::{Duration, Instant},
};
use twinkle_charts::{
    ChartEvent, ChartInputBindings, ChartState, ChartTable, ChartTableEvent, ChartTableFilter,
    ChartView,
};
use twinkle_security::{
    benchmark::{self, Config, Ledger, PERIOD_US, Report, TickRecord},
    charts::{self, HeatOptions, Projection},
    simulation::{self, CATEGORIES, Simulation, TIERS},
};
use twinkle_ui::{
    AdapterOutcome, Application, Button, Column, Component, Container, Grid, HostAdapter,
    HostServices, NativeFrameCorrelation, NativeFramePresentation, NativeFrameSubmission, Point,
    Rect, Shortcut, ShortcutOutcome, Text, UiHost, VerticalScroll, ViewContext,
};

#[derive(Clone, Debug)]
enum Message {
    Pause,
    Restart,
    Category,
    Detail,
    Table(usize),
    Chart(usize, ChartEvent),
    Pointer(usize, Point),
    TableEvent(ChartTableEvent),
    NextTier,
    Scroll,
}
fn pointer0(p: Point) -> Message {
    Message::Pointer(0, p)
}
fn pointer1(p: Point) -> Message {
    Message::Pointer(1, p)
}
fn pointer2(p: Point) -> Message {
    Message::Pointer(2, p)
}
fn pointer3(p: Point) -> Message {
    Message::Pointer(3, p)
}
struct Dashboard {
    config: Config,
    sim: Simulation,
    projection: Projection,
    views: Vec<Rc<RefCell<ChartView>>>,
    states: Vec<ChartState>,
    revision: u64,
    epoch_id: u64,
    clock: Instant,
    next_interactive: Instant,
    paused: bool,
    heat_options: HeatOptions,
    table_panel: Option<usize>,
    table_offset: usize,
    table_tier: usize,
    status: String,
    benchmark: Option<NativeBenchmark>,
}
struct NativeBenchmark {
    report: Report,
    ledger: Ledger,
    warmup_done: u64,
    warmup_last_ack: u64,
    reset_done: bool,
    measured: u64,
    attempted: u64,
    drain_started: Option<Instant>,
    error: Option<String>,
    finished: bool,
    native_identity: Option<(&'static str, (u32, u32), f32)>,
}
impl Dashboard {
    fn new(config: Config) -> Result<Self, Box<dyn Error>> {
        let mut sim = Simulation::new(config.seed, config.preset);
        if !config.benchmark() {
            for _ in 0..config.start_tick {
                sim.step();
            }
        }
        let projection = Projection::build(&sim, 1, HeatOptions::default())?;
        let mut views = charts::create_views(&projection)?;
        charts::prepare(&mut views, &projection, 1)?;
        let benchmark = if config.benchmark() {
            Some(NativeBenchmark {
                report: Report::new(&config)?,
                ledger: Ledger::new(),
                warmup_done: 0,
                warmup_last_ack: 0,
                reset_done: false,
                measured: 0,
                attempted: 0,
                drain_started: None,
                error: None,
                finished: false,
                native_identity: None,
            })
        } else {
            None
        };
        Ok(Self {
            config,
            sim,
            projection,
            views: views
                .into_iter()
                .map(|v| Rc::new(RefCell::new(v)))
                .collect(),
            states: vec![ChartState::default(); 4],
            revision: 1,
            epoch_id: 0,
            clock: Instant::now(),
            next_interactive: Instant::now() + Duration::from_millis(100),
            paused: false,
            heat_options: HeatOptions::default(),
            table_panel: None,
            table_offset: 0,
            table_tier: 0,
            status:
                "Synthetic data · hover or focus a chart and use arrow keys · fixed severity 0–100%"
                    .into(),
            benchmark,
        })
    }
    fn publish(&mut self) -> Result<(u64, [u64; 4]), Box<dyn Error>> {
        self.revision = self.revision.checked_add(1).ok_or("revision overflow")?;
        let start = Instant::now();
        let mut p = Projection::build(&self.sim, self.revision, self.heat_options)?;
        p.compare_previous(&self.projection);
        let projection_us = start.elapsed().as_micros() as u64;
        let mut times = [0; 4];
        for (i, chart) in self.views.iter().enumerate() {
            let mut view = chart.borrow_mut();
            let old = view.snapshot().input.unwrap();
            let start = Instant::now();
            view.replace_and_update(
                p.specs[i].clone(),
                p.data[i].clone(),
                charts::input(self.revision, old.bounds, old.scale_factor),
            )?;
            times[i] = start.elapsed().as_micros() as u64;
        }
        self.projection = p;
        Ok((projection_us, times))
    }
    fn failed(&mut self, error: impl std::fmt::Display) {
        self.status = format!("FAILED: {error}");
        self.paused = true;
        if let Some(b) = &mut self.benchmark {
            b.error = Some(error.to_string());
            b.drain_started = Some(Instant::now());
        }
    }
    fn advance(&mut self) -> bool {
        if self.epoch_id == 1 {
            if let Some(b) = &mut self.benchmark {
                b.attempted += 1;
            }
        }
        let start = Instant::now();
        self.sim.step();
        let generation_us = start.elapsed().as_micros() as u64;
        let dispatch_us = start.saturating_duration_since(self.clock).as_micros() as u64;
        match self.publish() {
            Ok((projection_us, preparation_us)) => {
                if self.epoch_id == 1 {
                    if let Some(b) = &mut self.benchmark {
                        let record = TickRecord {
                            epoch: self.epoch_id,
                            tick: self.sim.tick,
                            revision: self.revision,
                            deadline_us: b.measured * PERIOD_US,
                            dispatch_us,
                            generation_us,
                            projection_us,
                            preparation_us,
                            accepted_us: self.clock.elapsed().as_micros() as u64,
                            submitted_us: None,
                            cpu_submission_us: None,
                            host_update_us: None,
                            declaration_us: None,
                            layout_us: None,
                            presented_us: None,
                            discarded: false,
                            presentation_observed_us: None,
                            refresh_interval_ns: None,
                            compositor_sequence: None,
                            rows: self.projection.rows,
                            geometry: std::array::from_fn(|i| {
                                self.views[i].borrow().diagnostics().geometry_vertices
                            }),
                            changed_heat_cells: self.projection.changed_heat_cells,
                            rss_kib: benchmark::rss_kib(),
                            panels: std::array::from_fn(|i| {
                                charts::metrics(&self.views[i].borrow(), i, self.config.preset)
                            }),
                        };
                        if let Err(error) = b.ledger.accept(record) {
                            self.failed(error);
                            return false;
                        }
                        b.measured += 1;
                    }
                }
                true
            }
            Err(error) => {
                self.failed(error);
                false
            }
        }
    }
    fn finish(&mut self) -> Result<(), Box<dyn Error>> {
        if let Some(b) = &mut self.benchmark {
            if b.finished {
                return Ok(());
            }
            if b.measured < self.config.ticks && b.error.is_none() {
                b.error = Some("window closed before benchmark completed".into());
            }
            let summary = b.report.finish(
                &self.config,
                &b.ledger,
                &self.sim,
                b.error.as_deref(),
                b.attempted,
            )?;
            eprintln!("{summary}");
            b.finished = true;
        }
        Ok(())
    }
    fn ready_to_exit(&self) -> bool {
        self.benchmark.as_ref().is_some_and(|b| {
            b.drain_started
                .is_some_and(|t| b.ledger.pending() == 0 || t.elapsed() >= Duration::from_secs(5))
        })
    }
    fn input_for(&self, i: usize) -> twinkle_ui::retained_surface::RetainedSurfaceInput {
        let mut input = self.views[i].borrow().snapshot().input.unwrap();
        input.revisions.content = self.revision;
        input
    }
    fn refresh_states(&mut self, i: usize) {
        self.revision = self
            .revision
            .checked_add(1)
            .expect("content revision overflow");
        let input = self.input_for(i);
        let result = self.views[i]
            .borrow_mut()
            .update_state(self.states[i].clone(), input)
            .map(|_| ());
        if let Err(error) = result {
            self.failed(error);
        }
    }
}
impl Application for Dashboard {
    type Message = Message;
    fn update(&mut self, message: Message) {
        if self.benchmark.is_some() {
            return;
        }
        match message {
            Message::Scroll => {}
            Message::Pause => {
                self.paused = !self.paused;
                self.next_interactive = Instant::now() + Duration::from_millis(100);
            }
            Message::Restart => {
                self.sim = Simulation::new(self.config.seed, self.config.preset);
                self.next_interactive = Instant::now() + Duration::from_millis(100);
                self.states = vec![ChartState::default(); 4];
                for i in 0..4 {
                    self.refresh_states(i);
                }
                if let Err(error) = self.publish() {
                    self.failed(error);
                }
            }
            Message::Category => {
                self.heat_options.category = match self.heat_options.category {
                    None => Some(0),
                    Some(5) => None,
                    Some(c) => Some(c + 1),
                };
                if let Err(error) = self.publish() {
                    self.failed(error);
                }
            }
            Message::Detail => {
                self.heat_options.detail = !self.heat_options.detail;
                if let Err(error) = self.publish() {
                    self.failed(error);
                }
            }
            Message::Table(i) => {
                self.table_panel = if self.table_panel == Some(i) {
                    None
                } else {
                    Some(i)
                };
                self.table_offset = 0;
            }
            Message::NextTier => {
                self.table_tier = (self.table_tier + 1) % 3;
                self.table_offset = 0;
            }
            Message::Pointer(i, point) => {
                self.views[i].borrow_mut().inspect_nearest_detail(point);
            }
            Message::Chart(i, event) => match event {
                ChartEvent::Navigate(command) => {
                    let event = self.views[i].borrow_mut().navigate(command);
                    if let Some(event) = event {
                        self.update(Message::Chart(i, event));
                    }
                }
                ChartEvent::Inspect { row, .. } | ChartEvent::Select { row, .. } => {
                    self.states[i].selected_rows.clear();
                    self.states[i].selected_rows.insert(row.clone());
                    self.status = format!("Selected {} — open exact values for all fields", row.0);
                    self.refresh_states(i);
                }
                ChartEvent::ToggleSeries { series, visible } => {
                    let selected = self.states[i].visible_series.get_or_insert_with(|| {
                        simulation::OUTCOMES.iter().map(|s| (*s).into()).collect()
                    });
                    if visible {
                        selected.insert(series);
                    } else {
                        selected.remove(&series);
                    }
                    self.refresh_states(i);
                }
                ChartEvent::TableMode(_) => self.update(Message::Table(i)),
                ChartEvent::ViewportChanged(v) => {
                    self.states[i].viewport = Some(v);
                    self.refresh_states(i);
                }
                ChartEvent::Viewport(_) => {}
            },
            Message::TableEvent(ChartTableEvent::Page { offset, .. }) => self.table_offset = offset,
            Message::TableEvent(ChartTableEvent::Inspect { row, .. }) => {
                self.status = format!("Exact row {}", row.0)
            }
            Message::TableEvent(ChartTableEvent::SetHiddenSeriesIncluded(_)) => {}
        }
    }
    fn poll(&mut self) -> bool {
        if self.paused && self.benchmark.is_none() {
            return false;
        }
        if let Some(b) = &self.benchmark {
            if b.drain_started.is_some() {
                return false;
            }
            if self.epoch_id == 0 {
                if b.warmup_done < self.config.warmup {
                    if !self.advance() {
                        return false;
                    }
                    self.benchmark.as_mut().unwrap().warmup_done += 1;
                    return true;
                }
                if b.warmup_last_ack < self.revision {
                    return false;
                }
                if !b.reset_done {
                    self.sim = Simulation::new(self.config.seed, self.config.preset);
                    for _ in 0..self.config.start_tick {
                        self.sim.step();
                    }
                    if let Err(error) = self.publish() {
                        self.failed(error);
                        return false;
                    }
                    self.benchmark.as_mut().unwrap().reset_done = true;
                    return true;
                }
                let plot = self.views[1]
                    .borrow()
                    .plot_bounds()
                    .expect("accepted heatmap plot");
                if plot.size.width < 599.99 || plot.size.height < 299.99 {
                    if self.clock.elapsed() < Duration::from_secs(5) {
                        return false;
                    }
                    self.failed(format!(
                        "benchmark heatmap plot is {}x{}; requires at least 600x300 logical pixels",
                        plot.size.width, plot.size.height
                    ));
                    return false;
                }
                self.epoch_id = 1;
                self.clock = Instant::now();
                if let Err(error)=self.benchmark.as_mut().unwrap().report.emit(serde_json::json!({"schema":"twinkle-security-v1","event":"warmup_complete","rss_kib":benchmark::rss_kib(),"last_revision":self.revision})){self.failed(error);}
                return self.advance();
            }
            if b.measured >= self.config.ticks {
                self.benchmark.as_mut().unwrap().drain_started = Some(Instant::now());
                return false;
            }
            if self.clock.elapsed().as_micros() < (b.measured * PERIOD_US) as u128 {
                return false;
            }
            return self.advance();
        }
        if Instant::now() < self.next_interactive {
            return false;
        }
        let late = Instant::now().saturating_duration_since(self.next_interactive);
        self.next_interactive += Duration::from_millis(100);
        if late >= Duration::from_millis(100) {
            self.status = format!("OVERLOAD: {} ms behind scheduled tick", late.as_millis());
        }
        self.advance()
    }
    fn poll_interval(&self) -> Option<Duration> {
        if self.benchmark.is_some() {
            None
        } else if self.paused {
            None
        } else {
            Some(
                self.next_interactive
                    .saturating_duration_since(Instant::now())
                    .max(Duration::from_millis(1)),
            )
        }
    }
    fn take_frame_correlation(&mut self) -> Option<NativeFrameCorrelation> {
        Some(NativeFrameCorrelation {
            mount: self.epoch_id,
            generation: self.revision,
        })
    }
    fn native_frame_submitted(&mut self, s: NativeFrameSubmission) {
        let Some(b) = &mut self.benchmark else { return };
        let identity = (s.backend, s.physical_size, s.scale_factor);
        if b.native_identity != Some(identity) {
            if self.epoch_id == 1 && b.native_identity.is_some() {
                b.error = Some("native renderer/size/scale changed during measurement".into());
                b.drain_started = Some(Instant::now());
            }
            let result=b.report.emit(serde_json::json!({"schema":"twinkle-security-v1","event":"native_capabilities","actual_renderer":s.backend,"physical_size":s.physical_size,"device_scale":s.scale_factor,"presentation_supported":b.ledger.presentation_supported}));
            if let Err(error) = result {
                b.error = Some(error.to_string());
                b.drain_started = Some(Instant::now());
            }
            b.native_identity = Some(identity);
        }
        let Some(token) = s.correlation else { return };
        if token.mount == 0 {
            b.warmup_last_ack = b.warmup_last_ack.max(token.generation);
        } else if token.mount == 1 {
            b.ledger.submitted(
                token.generation,
                s.submitted_at
                    .saturating_duration_since(self.clock)
                    .as_micros() as u64,
            );
            if let Some(record) = b
                .ledger
                .records
                .iter_mut()
                .find(|r| r.revision == token.generation)
            {
                record
                    .cpu_submission_us
                    .get_or_insert(s.present_call_duration.as_micros() as u64);
            }
        }
    }
    fn native_presentation_supported(&mut self, supported: bool) {
        if let Some(b) = &mut self.benchmark {
            b.ledger.presentation_supported = supported;
        }
    }
    fn native_frame_presented(&mut self, p: NativeFramePresentation) {
        let Some(b) = &mut self.benchmark else { return };
        let Some(token) = p.correlation else { return };
        if token.mount == 1 {
            b.ledger.presented(
                token.generation,
                p.presented_at
                    .map(|t| t.saturating_duration_since(self.clock).as_micros() as u64),
                p.observed_at
                    .saturating_duration_since(self.clock)
                    .as_micros() as u64,
                p.discarded,
            );
            if let Some(record) = b
                .ledger
                .records
                .iter_mut()
                .find(|r| r.revision == token.generation)
            {
                record.refresh_interval_ns = p.refresh_interval.map(|d| d.as_nanos() as u64);
                record.compositor_sequence = p.sequence;
            }
        }
    }
    fn shortcut_outcome(&mut self, s: Shortcut) -> ShortcutOutcome {
        if self.benchmark.is_some() {
            return ShortcutOutcome::from_changed(false);
        }
        match s {
            Shortcut::Reload => {
                self.update(Message::Restart);
                ShortcutOutcome::handled(true)
            }
            Shortcut::Escape => {
                self.table_panel = None;
                ShortcutOutcome::handled(true)
            }
            _ => ShortcutOutcome::from_changed(false),
        }
    }
    fn title(&self) -> &str {
        "Twinkle Security"
    }
    fn initial_size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }
    fn view(&self, context: ViewContext) -> impl twinkle_ui::View<Message> {
        let (requests, blocked, hosts) = self.sim.totals();
        let phase = simulation::phase(self.sim.tick);
        let mut content = Column::new()
            .fill_width()
            .fill_height()
            .gap(8.0)
            .child(
                Text::new("SECURITY OPERATIONS   ·   SIMULATED DATA")
                    .bold(true)
                    .color(0x75e0d3),
            )
            .child(
                Text::new(format!(
                    "{phase}   |   {} requests/s   |   {:.1}% blocked   |   {hosts} affected hosts",
                    requests * 10,
                    blocked as f64 * 100.0 / requests.max(1) as f64
                ))
                .color(0xe5edf7),
            );
        if self.benchmark.is_none() {
            let category = self
                .heat_options
                .category
                .map_or("All categories", |i| CATEGORIES[i]);
            let mut controls = Grid::fixed(if context.viewport.size.width < 1100.0 {
                2
            } else {
                4
            })
            .gap(6.0)
            .child(Button::new(
                Message::Pause,
                if self.paused { "Resume" } else { "Pause" },
            ))
            .child(Button::new(Message::Restart, "Restart"))
            .child(Button::new(Message::Category, category))
            .child(Button::new(
                Message::Detail,
                if self.heat_options.detail {
                    "Rolling heatmap"
                } else {
                    "Category detail"
                },
            ));
            if self.table_panel == Some(2) {
                controls = controls.child(Button::new(
                    Message::NextTier,
                    format!("{} table", TIERS[self.table_tier]),
                ));
            }
            content = content.child(controls);
        }
        if let Some(b) = &self.benchmark {
            content = content.child(
                Text::new(format!(
                    "Benchmark: {} / {} measured updates",
                    b.measured, self.config.ticks
                ))
                .color(0xb3c3d7),
            );
        } else if self.status.starts_with("FAILED:") {
            content = content.child(Text::new(self.status.clone()).color(0xb3c3d7).wrap(true));
        }
        if let Some(i) = self.table_panel {
            let table = ChartTable::new(self.projection.data[i].clone(), charts::limits());
            let dataset = if i == 2 {
                format!("tier{}", self.table_tier)
            } else {
                self.projection.table_ids[i].to_string()
            };
            match table.page_filtered(&dataset, self.table_offset, &ChartTableFilter::default()) {
                Ok(page) => {
                    content = content.child(table.element(&page, "exact-table", move |event| {
                        Message::TableEvent(event)
                    }))
                }
                Err(error) => content = content.child(Text::new(error.to_string())),
            }
        } else {
            let columns =
                if context.viewport.size.width < 1100.0 || context.viewport.size.height < 900.0 {
                    1
                } else {
                    2
                };
            let height = if columns == 1 {
                410.0
            } else {
                ((context.viewport.size.height - 160.0) / 2.0).max(300.0)
            };
            let grid = Grid::fixed(columns)
                .gap(12.0)
                .children(self.views.iter().enumerate().map(|(i, view)| {
                    let view = Rc::clone(view);
                    Container::new()
                        .fill_width()
                        .height(height)
                        .into_element()
                        .with_layout_child_preferred(charts::PANEL_BOUNDS.size, move |allocation| {
                            let mut view = view.borrow_mut();
                            let mut input = view.snapshot().input.unwrap();
                            input.bounds =
                                Rect::new(0.0, 0.0, allocation.size.width, allocation.size.height);
                            if let Err(error) = view.update(input) {
                                return Text::new(format!("Chart layout failed: {error}"))
                                    .into_element();
                            }
                            view.element_with_input(
                                format!("security-chart-{i}"),
                                move |event| Message::Chart(i, event),
                                ChartInputBindings {
                                    pointer_move: Some([pointer0, pointer1, pointer2, pointer3][i]),
                                    ..Default::default()
                                },
                            )
                            .expect("accepted chart")
                            .into_element()
                        })
                }));
            if columns == 1 {
                content = content.child(
                    VerticalScroll::new(Message::Scroll, 0.0)
                        .height((context.viewport.size.height - 200.0).max(160.0))
                        .child(grid),
                );
            } else {
                content = content.child(grid);
            }
        }
        Container::new()
            .fill_width()
            .fill_height()
            .padding(16.0)
            .background(0x0c1424)
            .child(content)
    }
}
struct BenchmarkAdapter {
    enabled: bool,
}
impl HostAdapter<Dashboard> for BenchmarkAdapter {
    fn window_attributes(
        &self,
        app: &Dashboard,
        attributes: winit::window::WindowAttributes,
    ) -> winit::window::WindowAttributes {
        if app.benchmark.is_some() {
            let size = winit::dpi::LogicalSize::new(app.config.width, app.config.height);
            attributes
                .with_surface_size(size)
                .with_min_surface_size(size)
                .with_max_surface_size(size)
                .with_resizable(false)
        } else {
            attributes
        }
    }

    fn started(
        &mut self,
        host: &mut UiHost<Dashboard>,
        services: HostServices<'_>,
    ) -> Result<AdapterOutcome, Box<dyn Error>> {
        let c = &host.application().config;
        let _ = services
            .window()
            .request_surface_size(winit::dpi::LogicalSize::new(c.width, c.height).into());
        Ok(AdapterOutcome::default())
    }
    fn poll_interval(&self) -> Option<Duration> {
        self.enabled.then(|| Duration::from_millis(5))
    }
    fn poll(
        &mut self,
        host: &mut UiHost<Dashboard>,
        _: HostServices<'_>,
    ) -> Result<AdapterOutcome, Box<dyn Error>> {
        if host.application().benchmark.is_some() {
            let outcome = host.step(twinkle_ui::HostBatch {
                now: Some(Instant::now()),
                events: vec![twinkle_ui::HostEvent::Poll],
                ..Default::default()
            });
            if let Some(token) = outcome.change_token.native_correlation {
                if token.mount == 1 {
                    let app = host.application_mut();
                    if let Some(b) = &mut app.benchmark {
                        if let Some(r) = b.ledger.records.last_mut().filter(|r| {
                            r.revision == token.generation && r.host_update_us.is_none()
                        }) {
                            r.host_update_us = Some(outcome.telemetry.input_to_frame_us);
                            r.declaration_us = Some(outcome.telemetry.paint_list_us);
                            r.layout_us = Some(outcome.telemetry.layout_us);
                            r.panels = std::array::from_fn(|i| {
                                charts::metrics(&app.views[i].borrow(), i, app.config.preset)
                            });
                        }
                    }
                }
            }
        }
        if host.application().ready_to_exit() {
            host.application_mut().finish()?;
            Ok(AdapterOutcome::exit())
        } else {
            Ok(AdapterOutcome::default())
        }
    }
    fn stopped(
        &mut self,
        host: &mut UiHost<Dashboard>,
        _: HostServices<'_>,
    ) -> Result<(), Box<dyn Error>> {
        host.application_mut().finish()
    }
}
fn main() -> Result<(), Box<dyn Error>> {
    let config =
        Config::parse().map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
    if let Some(path) = &config.preview {
        let path = path.clone();
        let width = config.width;
        let height = config.height;
        let host = UiHost::new(Dashboard::new(config)?, width, height);
        let mut renderer = twinkle_ui::SoftwareRenderer::new(width, height, 1.0);
        renderer.render(host.render_frame().commands);
        let bytes = renderer
            .pixels()
            .iter()
            .flat_map(|p| [p.r, p.g, p.b, p.a])
            .collect::<Vec<_>>();
        image::save_buffer(path, &bytes, width, height, image::ColorType::Rgba8)?;
        return Ok(());
    }
    if config.headless {
        return benchmark::headless(config);
    }
    // Set renderer selection before creating any runtime workers.
    unsafe {
        std::env::set_var("TWINKLE_RENDERER", &config.renderer);
    }
    let enabled = config.benchmark();
    let dashboard = Dashboard::new(config)?;
    twinkle_ui::run_with_adapter(dashboard, BenchmarkAdapter { enabled })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pause_restart_selection_and_exact_tables_work_through_native_layout() {
        let mut app = Dashboard::new(Config::default()).unwrap();
        let initial = app.sim.checksum();
        app.update(Message::Pause);
        for _ in 0..5 {
            assert!(!app.poll());
        }
        assert_eq!(app.sim.checksum(), initial);
        app.update(Message::Chart(
            0,
            ChartEvent::ToggleSeries {
                series: "Allowed".into(),
                visible: false,
            },
        ));
        let paths = app.views[0]
            .borrow()
            .snapshot()
            .output
            .paint
            .iter()
            .filter(|p| matches!(p, twinkle_ui::backend::PaintCommand::Path { .. }))
            .count();
        assert_eq!(paths, 2, "legend toggles must repaint while paused");
        app.update(Message::Chart(
            0,
            ChartEvent::ToggleSeries {
                series: "Allowed".into(),
                visible: true,
            },
        ));
        app.update(Message::Pause);
        app.next_interactive = Instant::now() - Duration::from_millis(100);
        assert!(app.poll());
        assert_eq!(app.sim.tick, 1);
        let hit = app.views[2].borrow().hits()[0].clone();
        app.update(Message::Chart(
            2,
            ChartEvent::Select {
                dataset: hit.dataset,
                row: hit.row.clone(),
                series: hit.series,
                revision: app.revision,
            },
        ));
        app.advance();
        assert!(app.states[2].selected_rows.contains(&hit.row));
        app.update(Message::Chart(
            2,
            ChartEvent::Navigate(twinkle_charts::NavigationCommand::Next),
        ));
        assert!(app.views[2].borrow().active_hit().is_some());
        let plot = app.views[1].borrow().plot_bounds().unwrap();
        app.update(Message::Pointer(
            1,
            Point {
                x: plot.origin.x + plot.size.width / 2.0,
                y: plot.origin.y + plot.size.height / 2.0,
            },
        ));
        assert!(app.views[1].borrow().active_inspection().is_some());
        app.update(Message::Detail);
        assert_eq!(app.projection.rows[1], app.config.preset.groups() * 6);
        app.update(Message::Detail);
        app.update(Message::Category);
        assert_eq!(app.heat_options.category, Some(0));
        app.update(Message::Restart);
        assert_eq!(app.sim.checksum(), initial);
        let mut host = UiHost::new(app, 1640, 1020);
        for i in 0..4 {
            host.application_mut().update(Message::Table(i));
            host.step(twinkle_ui::HostBatch {
                application_changed: true,
                ..Default::default()
            });
            assert!(!host.render_frame().commands.is_empty());
            host.application_mut().update(Message::Table(i));
        }
        host.resize(900, 700);
        assert!(!host.render_frame().commands.is_empty());
    }
}
