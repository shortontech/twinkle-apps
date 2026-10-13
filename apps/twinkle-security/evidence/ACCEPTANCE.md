# Spec 0001 acceptance evidence

Validation date: 2026-10-13. Native reference: Linux x86_64, AMD Ryzen 9 5900X,
NVIDIA GeForce RTX 4080 SUPER / Vulkan, NVIDIA driver 595.99.02. Window: 1640 ×
1020 logical pixels, device scale 1. Actual adapter identity is preserved in
`gpu-standard-summary.json`; native capabilities and reported refresh intervals
are in each JSONL report. The full GPU run used pipeline attribution. GPU device
execution and allocation counters are unavailable, not inferred from CPU timings.

## Requirement audit

| Requirement | Authoritative evidence |
| --- | --- |
| One deterministic model shared by all graphs and KPIs | `src/simulation.rs`; conservation/bounds and campaign-versus-demand tests |
| Versioned seeded streams, negative pre-roll, exact windows, restart and wrap | Mixer vectors, canonical initialization at -700, rolling-history/reference tests and generator golden vectors |
| Cross-profile reproducibility | Debug golden-vector test and release `examples/checksums` output match `generator-v1-golden.json` byte-for-byte |
| Architecture-independent canonical data | All mixer/count operations use defined u64/i64 arithmetic; entity selection reduces u64 before usize conversion; explicit LE serialization and stable iteration. Executed evidence is x86_64; other architectures must run the same golden suite when deployed. |
| Four coordinated graphs changing every 100 ms | Four native retained ChartViews; visible dataset change test in quiet and attack phases; complete native 3,000-tick accounting |
| Traffic 3 × 600 samples; stable time/outcome keys | `charts::Projection::build`, admission test and per-tick admitted rows (1,800) |
| Dense heatmaps with meaningful visible cells | Standard/dense/stress: 6,000/12,000/20,000 exact rows; admission tests and native reports; min 600 × 300 plot guard |
| Honest changing-cell count and retained history | Comparison against the previous immutable projection includes the expired oldest column; 600/100-history bounds are tested; no independent history regeneration |
| Bubbles, host-area mapping and stable service identity | Fixed 0–200 host domain, 2 px floor, separate tier layers, exact tables; engine explicit-domain regression and selection-retention test |
| Ranked top 20, stable source identity and tie breaks | Ranking sorts (descending count, source ID); row keys derive from source ID, not position; source counts/labels and latest target/category are inspectable |
| Stable domains, labels and colors | Explicit chart scales, categorical guide regression, legend-toggle color regression; labels/global severity and tier legends visually inspected |
| Pause/resume/restart, filtering/detail, hover, keyboard and tables | Native-layout/control test checks paused checksum, restart, selection, navigation, hover and all four exact table layouts; narrow-window scrolling preview |
| Reliable generated child painting | Failing-then-passing engine regression verifies fresh paint and bounded hit/message counts over changing content/IDs; subtree invalidation fix is pinned |
| Absolute scheduling; no silent skipped ticks | Recorded deadlines are index × 100,000 us, integer model tick sequence, and increasing scene revisions; report verifier checks every measured tick |
| Bounded queues, overload and correct acknowledgements | 128-revision app and Wayland feedback bounds; injected timestamp ledger tests cover backlog, late display, discarded feedback and delayed observation |
| Warm-up exclusion and attribution | Separate epochs and monotonic revisions; reset/drain precede measurement; all measured records are epoch 1 and start at canonical requested tick |
| Real presentation evidence and honest unsupported mode | Linux Wayland wp_presentation CLOCK_MONOTONIC integration; actual displayed timestamp distinct from observation; explicit pass/fail/unsupported |
| Separate CPU generation, projection, chart preparation, UI and submission | Per-tick and percentile fields include all phases; inclusive host time is documented as overlapping, not additive |
| Metadata, memory and output | Schema v1 metadata, source checksum, engine pin, actual renderer/size/scale, reported refresh, RSS start/warm-up/tick/exit, sampled high-water and serialization cost |
| Documentation and workspace integration | Root/app README, public consumer documentation gaps, workspace package and consistent Git pins/lockfile |

## Timing gate interpretation

A pass requires **every** measured revision to be accepted, submitted and displayed
in order before its next 100-ms deadline. The full standard GPU run processed,
accepted, submitted and displayed all 3,000 revisions with no discarded or missing
frames. Its p95/p99 presentation latency was 31.349/38.632 ms, but six presentations
exceeded 100 ms (maximum 139.129 ms), so its strict result is **fail**. This is a
benchmark finding, not a waived gate or a universal real-time guarantee.

Native CPU, GPU dense and GPU stress smoke runs each displayed all 100 revisions
within deadline. CPU maximum presentation latency was 36.726 ms; dense 37.154 ms;
stress (20,000 heatmap cells) 48.917 ms. These short passes do not substitute for
the full-run result. Headless results are preparation/throughput evidence and
must remain **unsupported** for rendering.

## Resource evidence and boundaries

The corrected standard run sampled RSS from about 199 MiB initially to a maximum
250.6 MiB. Report records are deliberately buffered up to the bounded tick count;
RSS includes those records, driver caches, fonts and allocator retention. This is
not an allocation counter. Live histories remain exactly 600 traffic buckets,
100 heat buckets and 100 source-count buckets; pending revision tracking peaked
at three. Retained UI frame ownership remained approximately 0.6 MB after the
generated-subtree fix, rather than growing to 29 MB before it. Layout p95 was
1.253 ms after the fix. The baseline failed report and CPU profile remain under
ignored `target/security-evidence/` for diagnosis.

## Visual evidence

Software renders of the actual native UiHost declaration were inspected for
baseline, legitimate surge, credential campaign, scanning, mitigation and phase
wrap at reference size, plus a 900 × 700 scrolling layout. The PNGs are layout
proof, not compositor timing proof. Exact row values remain available even when
axes thin labels or marks overlap; rectangle counts include background and path
vertex counts exclude rectangle/circle GPU tessellation. No screen-reader bridge
or GPU execution-time claim is made.

## Final checks

At the final engine pin, `cargo check --workspace --locked`, all 12
`twinkle-security` tests, `cargo fmt --all -- --check`, and `git diff --check`
passed. Engine release validation passed 620 core tests (16 ignored) and 60 chart
tests. All seven stored JSONL reports passed the workload/report verifier.
