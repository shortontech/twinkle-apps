use crate::{
    charts::{self, HeatOptions, Projection},
    simulation::{GENERATOR_VERSION, Preset, Simulation},
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{BufWriter, Write},
    path::PathBuf,
    time::{Duration, Instant},
};

pub const PERIOD_US: u64 = 100_000;
pub const MAX_PENDING: usize = 128;
#[derive(Clone, Debug)]
pub struct Config {
    pub seed: u64,
    pub preset: Preset,
    pub ticks: u64,
    pub warmup: u64,
    pub headless: bool,
    pub unpaced: bool,
    pub renderer: String,
    pub output: Option<PathBuf>,
    pub preview: Option<PathBuf>,
    pub start_tick: u64,
    pub width: u32,
    pub height: u32,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            seed: 42,
            preset: Preset::Standard,
            ticks: 3000,
            warmup: 100,
            headless: false,
            unpaced: false,
            renderer: "gpu".into(),
            output: None,
            preview: None,
            start_tick: 0,
            width: 1640,
            height: 1020,
        }
    }
}
impl Config {
    pub fn parse() -> Result<Self, String> {
        Self::parse_args(std::env::args().skip(1))
    }
    pub fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut c = Self::default();
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.as_str(){
                "--export-preview"=>c.preview=Some(args.next().ok_or("--export-preview needs a path")?.into()),
                "--headless"=>c.headless=true,"--unpaced"=>c.unpaced=true,
                "--seed"=>c.seed=args.next().ok_or("--seed needs a value")?.parse().map_err(|_|"invalid seed")?,
                "--preset"=>c.preset=Preset::parse(&args.next().ok_or("--preset needs a value")?)?,
                "--ticks"=>c.ticks=args.next().ok_or("--ticks needs a value")?.parse().map_err(|_|"invalid ticks")?,
                "--warmup-ticks"=>c.warmup=args.next().ok_or("--warmup-ticks needs a value")?.parse().map_err(|_|"invalid warmup")?,
                "--start-tick"=>c.start_tick=args.next().ok_or("--start-tick needs a value")?.parse().map_err(|_|"invalid start tick")?,
                "--width"=>c.width=args.next().ok_or("--width needs a value")?.parse().map_err(|_|"invalid width")?,
                "--height"=>c.height=args.next().ok_or("--height needs a value")?.parse().map_err(|_|"invalid height")?,
                "--renderer"=>c.renderer=args.next().ok_or("--renderer needs a value")?,
                "--benchmark-output"=>c.output=Some(args.next().ok_or("--benchmark-output needs a value")?.into()),
                "--help"=>return Err("usage: twinkle-security [--headless] [--unpaced (headless only)] [--seed N] [--preset standard|dense|stress] [--ticks N] [--warmup-ticks N] [--renderer software|gpu] [--benchmark-output PATH] [--start-tick N] [--width N] [--height N]".into()),
                _=>return Err(format!("unknown argument: {arg}")),
            }
        }
        if !(1..=36_000).contains(&c.ticks) || c.warmup > 1000 || c.start_tick > 30_000 {
            return Err("ticks must be 1..36000, warmup <=1000, start tick <=30000".into());
        }
        if !["gpu", "software"].contains(&c.renderer.as_str()) {
            return Err("renderer must be gpu or software".into());
        }
        if c.preview.is_some() && (c.headless || c.output.is_some()) {
            return Err("--export-preview cannot be combined with benchmark modes".into());
        }
        if c.unpaced && !c.headless {
            return Err("--unpaced is headless only".into());
        }
        if c.width < 640 || c.height < 480 {
            return Err("window must be at least 640x480".into());
        }
        Ok(c)
    }
    pub fn benchmark(&self) -> bool {
        self.headless || self.output.is_some()
    }
}
#[derive(Clone, Debug)]
pub struct TickRecord {
    pub epoch: u64,
    pub tick: i64,
    pub revision: u64,
    pub deadline_us: u64,
    pub dispatch_us: u64,
    pub generation_us: u64,
    pub projection_us: u64,
    pub preparation_us: [u64; 4],
    pub accepted_us: u64,
    pub submitted_us: Option<u64>,
    pub cpu_submission_us: Option<u64>,
    pub host_update_us: Option<u64>,
    pub declaration_us: Option<u64>,
    pub layout_us: Option<u64>,
    pub presented_us: Option<u64>,
    pub discarded: bool,
    pub presentation_observed_us: Option<u64>,
    pub refresh_interval_ns: Option<u64>,
    pub compositor_sequence: Option<u64>,
    pub rows: [usize; 4],
    pub geometry: [usize; 4],
    pub changed_heat_cells: Option<usize>,
    pub rss_kib: Option<u64>,
    pub panels: [Value; 4],
}
impl TickRecord {
    pub fn value(&self) -> Value {
        json!({"schema":"twinkle-security-v1","event":"tick","epoch":self.epoch,"tick":self.tick,"revision":self.revision,"scheduled_us":self.deadline_us,"dispatch_us":self.dispatch_us,"generation_us":self.generation_us,"projection_us":self.projection_us,"preparation_us":self.preparation_us,"accepted_us":self.accepted_us,"submitted_us":self.submitted_us,"cpu_submission_us":self.cpu_submission_us,"host_update_us":self.host_update_us,"declaration_us":self.declaration_us,"layout_us":self.layout_us,"presented_us":self.presented_us,"discarded":self.discarded,"presentation_observed_us":self.presentation_observed_us,"refresh_interval_ns":self.refresh_interval_ns,"compositor_sequence":self.compositor_sequence,"admitted_rows":self.rows,"geometry_vertices":self.geometry,"changed_heat_cells":self.changed_heat_cells,"rss_kib":self.rss_kib,"panels":self.panels})
    }
}
/// Inject integer elapsed timestamps, not wall-clock sleeps. Revisions never reset.
pub struct Ledger {
    pub records: Vec<TickRecord>,
    pending: BTreeMap<u64, usize>,
    presentations: BTreeMap<u64, usize>,
    pub presentation_supported: bool,
    pub high_water: usize,
    pub duplicate_acks: u64,
    pub unknown_acks: u64,
}
impl Ledger {
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
            pending: BTreeMap::new(),
            presentations: BTreeMap::new(),
            presentation_supported: false,
            high_water: 0,
            duplicate_acks: 0,
            unknown_acks: 0,
        }
    }
    pub fn accept(&mut self, record: TickRecord) -> Result<(), String> {
        if self.pending() >= MAX_PENDING {
            return Err(format!(
                "submission ledger overload: {MAX_PENDING} outstanding revisions"
            ));
        }
        if self
            .records
            .last()
            .is_some_and(|r| r.revision >= record.revision)
        {
            return Err("content revision must increase".into());
        }
        self.pending.insert(record.revision, self.records.len());
        if self.presentation_supported {
            self.presentations
                .insert(record.revision, self.records.len());
        }
        self.records.push(record);
        self.high_water = self.high_water.max(self.pending());
        Ok(())
    }
    pub fn submitted(&mut self, revision: u64, at: u64) {
        if let Some(index) = self.pending.remove(&revision) {
            self.records[index].submitted_us = Some(at);
        } else if self
            .records
            .iter()
            .any(|r| r.revision == revision && r.submitted_us.is_some())
        {
            self.duplicate_acks += 1;
        } else {
            self.unknown_acks += 1;
        }
    }
    pub fn presented(
        &mut self,
        revision: u64,
        timestamp: Option<u64>,
        observed: u64,
        discarded: bool,
    ) {
        if let Some(index) = self.presentations.remove(&revision) {
            self.records[index].presented_us = timestamp;
            self.records[index].presentation_observed_us = Some(observed);
            self.records[index].discarded = discarded;
        }
    }
    pub fn pending(&self) -> usize {
        self.pending_revisions().len()
    }
    pub fn pending_revisions(&self) -> Vec<u64> {
        self.pending
            .keys()
            .chain(self.presentations.keys())
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect()
    }
}
impl Default for Ledger {
    fn default() -> Self {
        Self::new()
    }
}
pub fn percentiles(values: impl IntoIterator<Item = u64>) -> Value {
    let mut v = values.into_iter().collect::<Vec<_>>();
    v.sort_unstable();
    if v.is_empty() {
        return Value::Null;
    }
    let at = |p: usize| v[(v.len() * p).div_ceil(100).saturating_sub(1)];
    json!({"p50":at(50),"p95":at(95),"p99":at(99),"max":v[v.len()-1],"samples":v.len()})
}
pub fn rss_kib() -> Option<u64> {
    std::fs::read_to_string("/proc/self/status")
        .ok()?
        .lines()
        .find(|l| l.starts_with("VmRSS:"))?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()
}
pub fn source_checksum() -> String {
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    for text in [
        include_str!("lib.rs"),
        include_str!("simulation.rs"),
        include_str!("charts.rs"),
        include_str!("benchmark.rs"),
        include_str!("main.rs"),
        include_str!("../Cargo.toml"),
        include_str!("../../../Cargo.toml"),
        include_str!("../../../Cargo.lock"),
    ] {
        hash.update((text.len() as u64).to_le_bytes());
        hash.update(text.as_bytes());
    }
    format!("{:x}", hash.finalize())
}
pub fn metadata(c: &Config) -> Value {
    let read = |p: &str| std::fs::read_to_string(p).ok();
    let cpu = read("/proc/cpuinfo").and_then(|s| {
        s.lines()
            .find(|l| l.starts_with("model name"))
            .map(str::to_owned)
    });
    let revision = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned());
    json!({"schema":"twinkle-security-v1","event":"metadata","generator_version":GENERATOR_VERSION,"source_checksum":source_checksum(),"seed":c.seed,"preset":c.preset.name(),"ticks":c.ticks,"warmup_ticks":c.warmup,"start_tick":c.start_tick,"mode":if c.headless{"headless chart preparation"}else{"native window"},"paced":!c.unpaced,"requested_renderer":c.renderer,"window_logical":[c.width,c.height],"app_revision":revision,"app_dirty":std::process::Command::new("git").args(["status","--porcelain"]).output().ok().map(|o|!o.stdout.is_empty()),"engine_revision":engine_revision(),"build_profile":if cfg!(debug_assertions){"debug"}else{"release"},"os":std::env::consts::OS,"architecture":std::env::consts::ARCH,"cpu":cpu,"gpu_candidates":std::process::Command::new("lspci").output().ok().map(|o|String::from_utf8_lossy(&o.stdout).lines().filter(|l|l.contains("VGA")||l.contains("3D controller")||l.contains("Display controller")).map(str::to_owned).collect::<Vec<_>>()),"gpu_execution_us":null,"device_scale":null,"display_refresh_hz":null,"limits":format!("{:?}",charts::limits()),"rss_sampling":"Linux /proc/self/status VmRSS, once per measured tick","rss_start_kib":rss_kib(),"allocation_metrics":null,"gpu_execution_us":null,"instrumentation":"Instant CPU durations + per-tick RSS; report buffering outside timed preparation","pipeline_trace":std::env::var("TWINKLE_PIPELINE_TRACE").ok()})
}
pub fn engine_revision() -> &'static str {
    // Build-time manifest text captures the actual pin and changes when repinned.
    include_str!("../../../Cargo.toml")
        .split("twinkle-charts = ")
        .nth(1)
        .and_then(|s| s.split("rev = \"").nth(1))
        .and_then(|s| s.split('"').next())
        .unwrap_or("unknown")
}
pub struct Report {
    pub writer: BufWriter<Box<dyn Write>>,
    pub write_us: u64,
}
impl Report {
    pub fn new(c: &Config) -> Result<Self, Box<dyn std::error::Error>> {
        let writer: Box<dyn Write> = if let Some(path) = &c.output {
            Box::new(File::create(path)?)
        } else {
            Box::new(std::io::stdout())
        };
        let mut r = Self {
            writer: BufWriter::new(writer),
            write_us: 0,
        };
        r.emit(metadata(c))?;
        Ok(r)
    }
    pub fn emit(&mut self, value: Value) -> Result<(), Box<dyn std::error::Error>> {
        let start = Instant::now();
        serde_json::to_writer(&mut self.writer, &value)?;
        self.writer.write_all(b"\n")?;
        self.write_us += start.elapsed().as_micros() as u64;
        Ok(())
    }
    pub fn finish(
        &mut self,
        c: &Config,
        ledger: &Ledger,
        sim: &Simulation,
        error: Option<&str>,
        processed: u64,
    ) -> Result<Value, Box<dyn std::error::Error>> {
        let records = &ledger.records;
        for record in records {
            self.emit(record.value())?;
        }
        let missing = records.iter().filter(|r| r.submitted_us.is_none()).count();
        let missed = records
            .iter()
            .filter(|r| r.submitted_us.unwrap_or(r.accepted_us) > r.deadline_us + PERIOD_US)
            .count();
        let failed = error.is_some() || records.len() != c.ticks as usize;
        let presented = records.iter().filter(|r| r.presented_us.is_some()).count();
        let discarded = records.iter().filter(|r| r.discarded).count();
        let presentation_misses = records
            .iter()
            .filter(|r| r.presented_us.is_none_or(|v| v > r.deadline_us + PERIOD_US))
            .count();
        let presentation_ordered = records
            .iter()
            .filter_map(|r| r.presented_us)
            .collect::<Vec<_>>()
            .windows(2)
            .all(|p| p[0] <= p[1]);
        let rendered_pass = !failed
            && ledger.presentation_supported
            && presented == records.len()
            && presentation_misses == 0
            && presentation_ordered
            && missing == 0
            && missed == 0;
        let summary = json!({"schema":"twinkle-security-v1","event":"summary","status":if failed{"fail"}else if !ledger.presentation_supported{"unsupported"}else if rendered_pass{"pass"}else{"fail"},"reason":error.unwrap_or(if c.headless{"headless mode has no submission or compositor presentation"}else if !ledger.presentation_supported{"compositor presentation feedback unavailable"}else if rendered_pass{"every measured revision presented within its 100 ms deadline"}else{"missing, discarded, out-of-order, or late presentations"}),"update_status":if failed||records.iter().any(|r|r.accepted_us>r.deadline_us+PERIOD_US){"fail"}else{"pass"},"submission_status":if c.headless{"unsupported"}else if failed||missing>0||missed>0{"fail"}else{"pass"},"processed":processed,"last_generated_tick":sim.tick,"last_accepted_tick":records.last().map(|r|r.tick),"accepted":records.len(),"submitted":records.iter().filter(|r|r.submitted_us.is_some()).count(),"presented":presented,"presentation_supported":ledger.presentation_supported,"reported_refresh_interval_ns":percentiles(records.iter().filter_map(|r|r.refresh_interval_ns.filter(|n|*n>0))),"missed_presentation_deadlines":if ledger.presentation_supported{json!(presentation_misses)}else{Value::Null},"presentation_ordered":if ledger.presentation_supported{json!(presentation_ordered)}else{Value::Null},"unacknowledged_revisions":ledger.pending_revisions(),"coalesced_or_unsubmitted":if c.headless{Value::Null}else{json!(missing)},"dropped":if ledger.presentation_supported{json!(discarded)}else{Value::Null},"queue_high_water":ledger.high_water,"duplicate_submission_acks":ledger.duplicate_acks,"unknown_submission_acks":ledger.unknown_acks,"missed_submission_or_update_deadlines":missed,"generation_us":percentiles(records.iter().map(|r|r.generation_us)),"projection_us":percentiles(records.iter().map(|r|r.projection_us)),"preparation_us":(0..4).map(|i|percentiles(records.iter().map(|r|r.preparation_us[i]))).collect::<Vec<_>>(),"dispatch_lateness_us":percentiles(records.iter().map(|r|r.dispatch_us.saturating_sub(r.deadline_us))),"host_update_us":percentiles(records.iter().filter_map(|r|r.host_update_us)),"declaration_us":percentiles(records.iter().filter_map(|r|r.declaration_us)),"layout_us":percentiles(records.iter().filter_map(|r|r.layout_us)),"cpu_submission_us":percentiles(records.iter().filter_map(|r|r.cpu_submission_us)),"submission_latency_us":percentiles(records.iter().filter_map(|r|r.submitted_us.map(|v|v.saturating_sub(r.deadline_us)))),"presentation_latency_us":percentiles(records.iter().filter_map(|r|r.presented_us.map(|v|v.saturating_sub(r.deadline_us)))),"rss_peak_sample_kib":records.iter().filter_map(|r|r.rss_kib).max(),"rss_exit_kib":rss_kib(),"model_checksum":sim.checksum(),"warmup_excluded":true,"report_serialization_us":self.write_us});
        self.emit(summary.clone())?;
        self.writer.flush()?;
        Ok(summary)
    }
}
pub fn headless(c: Config) -> Result<(), Box<dyn std::error::Error>> {
    let mut report = Report::new(&c)?;
    let mut sim = Simulation::new(c.seed, c.preset);
    let mut revision = 1;
    let p = Projection::build(&sim, revision, HeatOptions::default())?;
    let mut views = charts::create_views(&p)?;
    for _ in 0..c.warmup {
        sim.step();
        revision += 1;
        let p = Projection::build(&sim, revision, HeatOptions::default())?;
        charts::prepare(&mut views, &p, revision)?;
    }
    sim = Simulation::new(c.seed, c.preset);
    for _ in 0..c.start_tick {
        sim.step();
    }
    revision += 1;
    let mut previous = Projection::build(&sim, revision, HeatOptions::default())?;
    charts::prepare(&mut views, &previous, revision)?;
    report.emit(json!({"schema":"twinkle-security-v1","event":"warmup_complete","rss_kib":rss_kib(),"last_revision":revision}))?;
    let epoch = Instant::now();
    let mut ledger = Ledger::new();
    let mut processed = 0;
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        for index in 0..c.ticks {
            let deadline_us = index * PERIOD_US;
            if !c.unpaced {
                let target = epoch + Duration::from_micros(deadline_us);
                if let Some(wait) = target.checked_duration_since(Instant::now()) {
                    std::thread::sleep(wait);
                }
            }
            let dispatch_us = epoch.elapsed().as_micros() as u64;
            let start = Instant::now();
            processed += 1;
            sim.step();
            revision += 1;
            let generation_us = start.elapsed().as_micros() as u64;
            let start = Instant::now();
            let mut p = Projection::build(&sim, revision, HeatOptions::default())?;
            p.compare_previous(&previous);
            let projection_us = start.elapsed().as_micros() as u64;
            let preparation_us = charts::prepare(&mut views, &p, revision)?;
            // Headless has no outstanding native frames. Do not insert fictitious submissions.
            ledger.records.push(TickRecord {
                epoch: 1,
                tick: sim.tick,
                revision,
                deadline_us,
                dispatch_us,
                generation_us,
                projection_us,
                preparation_us,
                accepted_us: epoch.elapsed().as_micros() as u64,
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
                rows: p.rows,
                geometry: std::array::from_fn(|i| views[i].diagnostics().geometry_vertices),
                changed_heat_cells: p.changed_heat_cells,
                rss_kib: rss_kib(),
                panels: std::array::from_fn(|i| charts::metrics(&views[i], i, c.preset)),
            });
            previous = p;
        }
        Ok(())
    })();
    let error = result.as_ref().err().map(|e| e.to_string());
    let summary = report.finish(&c, &ledger, &sim, error.as_deref(), processed)?;
    eprintln!("{}", summary);
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    fn record(revision: u64) -> TickRecord {
        TickRecord {
            epoch: 1,
            tick: revision as i64,
            revision,
            deadline_us: (revision - 1) * PERIOD_US,
            dispatch_us: 0,
            generation_us: 0,
            projection_us: 0,
            preparation_us: [0; 4],
            accepted_us: 0,
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
            rows: [0; 4],
            geometry: [0; 4],
            changed_heat_cells: Some(0),
            rss_kib: None,
            panels: std::array::from_fn(|_| Value::Null),
        }
    }
    #[test]
    fn ledger_does_not_coalesce_and_has_bounded_backlog() {
        let mut l = Ledger::new();
        for i in 1..=MAX_PENDING as u64 {
            l.accept(record(i)).unwrap();
        }
        assert!(l.accept(record(129)).is_err());
        l.submitted(1, 100001);
        assert_eq!(l.records[0].submitted_us, Some(100001));
        l.accept(record(129)).unwrap();
        assert_eq!(l.high_water, 128);
        l.submitted(1, 200000);
        l.submitted(999, 2);
        assert_eq!(l.duplicate_acks, 1);
        assert_eq!(l.unknown_acks, 1);
    }
    #[test]
    fn compositor_ledger_distinguishes_late_display_discard_and_observation_delay() {
        let mut l = Ledger::new();
        l.presentation_supported = true;
        l.accept(record(1)).unwrap();
        l.accept(record(2)).unwrap();
        l.submitted(1, 50_000);
        assert_eq!(l.pending(), 2);
        l.presented(1, Some(90_000), 400_000, false);
        assert_eq!(l.records[0].presented_us, Some(90_000));
        assert_eq!(l.pending(), 1);
        l.submitted(2, 120_000);
        l.presented(2, None, 140_000, true);
        assert!(l.records[1].discarded);
        assert_eq!(l.pending(), 0);
        l.accept(record(3)).unwrap();
        l.submitted(3, 210_000);
        l.presented(3, Some(310_000), 320_000, false);
        assert!(l.records[2].presented_us.unwrap() > l.records[2].deadline_us + PERIOD_US);
    }
    #[test]
    fn percentiles_use_nearest_rank_and_null_empty() {
        assert_eq!(percentiles([]), Value::Null);
        let p = percentiles(1..=100);
        assert_eq!(p["p50"], 50);
        assert_eq!(p["p95"], 95);
        assert_eq!(p["p99"], 99);
        assert_eq!(p["max"], 100);
    }
    #[test]
    fn cli_rejects_ambiguous_or_unbounded_workloads() {
        assert!(Config::parse_args(["--ticks".into(), "0".into()]).is_err());
        assert!(Config::parse_args(["--unpaced".into()]).is_err());
        assert!(Config::parse_args(["--renderer".into(), "auto".into()]).is_err());
    }
}
