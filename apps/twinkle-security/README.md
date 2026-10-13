# Twinkle Security

A synthetic security operations dashboard with four coordinated native graphs,
updated every 100 ms: request traffic, a dense rolling threat heatmap, service
risk bubbles, and ranked attack sources. Data is generated locally; it describes
no real organization, host, or incident.

```sh
cargo run --release -p twinkle-security
```

Pause/resume and restart buttons control simulation time. Focus a chart and use
arrow keys to navigate its data, or hover a mark. Open exact-value tables with the
chart's Data table button. Tables are paginated; the service
table has a tier selector. The heatmap can filter a threat category or switch to
service-group/category detail. Narrow windows stack panels in a scroll area.
Reload restarts the deterministic scenario; Escape closes the exact-value table.

## Dataset

Defaults are seed 42 and the standard preset. The generator's five-minute phase
schedule repeats: baseline, legitimate demand surge, credential stuffing,
distributed scanning, then mitigation/recovery. Absolute ticks and row identities
continue across the scenario boundary. Pause advances no ticks; restart returns
to the same canonical initial state.

| Preset | Services | Sources | Heatmap rows × time columns |
| --- | ---: | ---: | ---: |
| standard | 60 | 200 | 60 × 100 = 6,000 cells |
| dense | 240 | 1,000 | 120 × 100 = 12,000 cells |
| stress | 1,000 | 5,000 | 200 × 100 = 20,000 cells |

Heatmap rows group 1/2/5 services, respectively. Columns are the last 100
100-ms buckets. Each new tick shifts all historical geometry, adds a new column,
and removes the oldest. Historical observations are not regenerated. Color is
suspicious requests/sec divided by group capacity, fixed at 0–100%; inspect exact
rates, capacity, member services, and all six threat-category contributions.
Source bars show a rolling ten-second count, with latest target/category fields.
Traffic history retains 600 observations per outcome. Bubble color identifies
service tier, Y is smoothed risk (0–100), and area maps affected hosts against a
fixed 0–200-host domain; a 2-logical-pixel minimum radius preserves zero values.
Magnitude axes use fixed bounds and exact tables retain values beyond those
bounds. Points may overlap; admitted row counts are not claims of visible marks.

Generator v1 uses the SplitMix64 integer finalizer with independent seed/entity/
signed-tick/channel keys. Negative ticks use two's-complement u64 encoding.
It starts immediately before tick -700 with empty histories and zero recursive
state, then replays through tick zero. All counts are integer requests per 100-ms
bucket; chart rates multiply by ten. Legitimate traffic uses 25–35% of capacity
at baseline, rising by another 35% during the surge. Suspicious counts are shared
across service, category, source, and outcome projections. Mitigation blocks a
larger share while incoming attack volume fades. Risk uses floor-rounded integer
smoothing `(9 * previous + target) / 10`, where target is the suspicious fraction
in thousandths of a score point. Host counts are bounded by service inventory.

Canonical SHA-256 checksums serialize scalar fields in explicit u64 little-endian
widths and length-prefixed UTF-8 preset names, in stable entity/history order.
Only the mixer wraps; count arithmetic has bounded inputs and rolling sums use
checked addition/subtraction. Golden vectors cover both seeds, every phase
boundary, scenario wrap, and all presets:

```sh
cargo test -p twinkle-security
cargo run -p twinkle-security --example checksums
cargo run --release -p twinkle-security --example checksums
```

The two checksum commands must match [generator-v1-golden.json](evidence/generator-v1-golden.json).
The simulation lives in `src/simulation.rs`, independent of chart/UI state.

## Benchmarks

Benchmark mode is enabled by `--benchmark-output` or `--headless`. Interactive
controls are disabled during a native benchmark. Use release builds and keep the
window visible at the recorded size. The reference window is 1640 × 1020 logical
pixels so the heatmap can allocate at least a 600 × 300 plot after chart chrome.
Run renderer variants separately, without competing benchmark processes.

```sh
cargo build --release -p twinkle-security
# Five minutes of measured 10 Hz native GPU updates, after 100 warm-up ticks:
TWINKLE_PIPELINE_TRACE=/tmp/security-gpu-pipeline.jsonl \
  target/release/twinkle-security --renderer gpu --ticks 3000 \
  --benchmark-output /tmp/security-gpu.jsonl
# Native CPU renderer:
target/release/twinkle-security --renderer software --ticks 3000 \
  --benchmark-output /tmp/security-cpu.jsonl
# Stress smoke:
target/release/twinkle-security --renderer gpu --preset stress --ticks 100 \
  --benchmark-output /tmp/security-stress.jsonl
# Headless preparation: no window, GPU execution, or compositor presentation:
target/release/twinkle-security --headless --ticks 3000 \
  --benchmark-output /tmp/security-headless.jsonl
# Optional unpaced throughput workload, distinct from the 10 Hz benchmark:
target/release/twinkle-security --headless --unpaced --ticks 100 \
  --benchmark-output /tmp/security-throughput.jsonl
```

Other options: `--seed N`, `--warmup-ticks N`, `--start-tick N` (phase smoke runs),
`--width N`, `--height N`. Tick count is bounded to 1–36,000, warm-up to 1,000, and
start tick to 30,000. `--export-preview PATH.png` renders a static native UiHost
frame through Twinkle's software renderer and exits; it cannot be combined with
benchmark modes. Example: `--start-tick 1500 --export-preview credential.png`.
These previews prove layout, not native compositor timing.

### Measurement boundary

All four retained chart views prepare a tick before its revision is published.
A preparation rejection stops the run rather than displaying a stale chart as a
successful update. Each measured update has an absolute deadline relative to a
monotonic epoch. Lateness never slows the simulation clock or changes tick count.
Warm-up and reset use separate epoch IDs with monotonically increasing content
revisions. A bounded ledger tracks every measured revision; missing submissions
or discarded compositor frames remain visible in results. The final drain has
a five-second timeout and cannot excuse missed original 100-ms deadlines.

On Linux Wayland, the enabled `wayland-presentation` engine feature requests
`wp_presentation` feedback on the native surface before presenting. Supported
CLOCK_MONOTONIC feedback supplies actual presentation time; callback observation
time is separate. CPU present-call duration is not GPU execution time. The
requested renderer may fall back; `native_capabilities` records the actual one.
GPU adapter identity is also recorded in the optional engine pipeline trace.
Other platforms/clocks without feedback return `unsupported`, never a rendered
pass inferred from update/submission speed.

### JSONL schema `twinkle-security-v1`

Output contains metadata, native capabilities (windowed runs), warm-up completion,
per-tick records, then a summary. Records contain epoch/tick/revision IDs,
scheduled and actual dispatch/acceptance/submission/presentation times in elapsed
microseconds, generation/projection/per-panel preparation CPU durations, inclusive host-update time, declaration
and layout times, CPU present-call duration, raw/admitted/displayed rows, paint primitive/path counts,
geometry, plot and cell dimensions, changed heat cells, diagnostics, and sampled
RSS. Rectangle counts include chart backgrounds; path vertex counts do not
include rectangle/bubble GPU tessellation. RSS comes from Linux `/proc/self/status`
VmRSS and is unavailable on other platforms. Allocation/GPU execution timing are
unavailable. Refresh/scale are recorded where available; unknown values stay null.

Summary gives nearest-rank p50/p95/p99/max, tick accounting, queue high-water,
missing/discarded revisions, deadline misses, model checksum, sampled memory peak,
and report serialization cost. Report output is buffered and written outside
preparation timing. Host-update time includes generation, projection, preparation, declaration, and
layout, so these overlapping durations must not be added together.
A source checksum identifies the compiled Rust sources,
manifests, and lockfile even when the checkout has uncommitted changes.

A rendered `pass` requires every measured revision to be accepted, submitted,
and presented in order within its 100-ms deadline. `fail` retains evidence of
late/missing/discarded frames; `unsupported` means no compositor timing boundary.
Update and submission statuses are separate. Headless never returns a rendered
pass. A performance failure is a valid benchmark result, not a universal hardware
claim. See the [release evidence](evidence/README.md) and [acceptance audit](evidence/ACCEPTANCE.md).
The full GPU run displayed all 3,000 updates with six deadline misses; CPU/dense/
stress smoke runs passed. The strict full-run failure is retained in the results.

Verify a completed report against the workload and timestamp contract with:

```sh
python3 apps/twinkle-security/scripts/verify_report.py /tmp/security-gpu.jsonl
```
