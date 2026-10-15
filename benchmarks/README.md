# Twinkle benchmarks

Standalone workloads, comparison apps and performance tooling live here. Engine
correctness tests and fixtures that require private implementation access remain
in the `twinkle` repository. Packaging smoke tests also remain with the engine.

- `twinkle/`: public-API Rust workloads (pipeline replay, chart dashboard/admission,
  compound controls, Redux core/JSX timings, and Blocks/rich-form integration tests).
- `0012/react/`: frozen React comparison app, with its original provenance.
- `0012/traces/`: deterministic behavior and lifecycle replay inputs.
- `scripts/`: serial comparison runners, process samplers and report summarizers.
- `iced-blocks/`: the existing independent Iced comparison app.

Run Rust commands from the **twinkle-apps repository root**:

```sh
cargo build --release -p twinkle-benchmarks --features jsx --bin twinkle-pipeline-bench
cargo run --release -p twinkle-benchmarks --features jsx --bin twinkle-compound-admission -- --family slider --mode cold --iterations 200
cargo run --release -p twinkle-benchmarks --features charts --bin twinkle-chart-measure
cargo run --release -p twinkle-benchmarks --features charts --bin twinkle-chart-dashboard
cargo run --release -p twinkle-benchmarks --bin twinkle-redux-measure
cargo run --release -p twinkle-benchmarks --features jsx --bin twinkle-redux-jsx-measure
cargo test --release -p twinkle-benchmarks --features jsx --test blocks_admission -- --ignored --nocapture --test-threads=1
```

Add `pipeline-attribution,pipeline-allocator` only for separate diagnostic builds.
Chart allocation admission uses `--features charts,pipeline-allocator --bin
twinkle-chart-admission`. Preserve normal release executables before building
instrumented variants; do not combine their timing series or run measurements
concurrently with builds.

The engine Git revisions are pinned in the workspace manifest and Cargo.lock.
The migration uses engine merge `870f7a991c3f98264e94841fe91b7af2d3250190`.
Change the pins (or use Cargo source patches for a local checkout) to compare
other engine versions; freeze both binaries before running paired trials.

For example, from the repository root:

```sh
python3 benchmarks/scripts/run_compound_measurement_pairs.py --before /path/to/baseline --after /path/to/candidate --engine-source ../twinkle --output benchmark-artifacts/compound-pairs
python3 -m unittest discover -s benchmarks/scripts -p 'test_*.py'
```

The measurement-boundary runner also accepts `--engine-source ../twinkle` because
its private measurement fixture remains in the engine. Its frozen integration
`--guards` executables now come from this package's `blocks_admission` test.
Runner paths are resolved from the scripts' location, independent of the caller's
working directory. Existing report and trace schemas remain unchanged.

Build React assets separately:

```sh
cd benchmarks/0012/react
npm ci
npm run build
```

Then run the live comparison tools in `benchmarks/scripts/`; they require their
explicit binary/output arguments and, for isolated Wayland trials, a private
compositor socket. `run_react_replay.mjs` uses a dedicated Chrome process. Sampling
and GPU/display evidence require a supported host and are not part of a source
relocation smoke test.

## Migration record

Sources moved from the engine workbench/examples/scripts without changing the
workloads, trace inputs, sample counts or admission thresholds. Imports now name
the public `twinkle_ui` crate; the chart measurement module is local to this
package. Original frozen-source provenance files describe their historical
inputs and are intentionally preserved. Generated benchmark artifacts, installed
node_modules and old executables were not migrated.

Relocation checks also exposed two compatibility issues: the existing JSX Blocks
app now ignores non-action messages enabled by the chart consumer's shared Cargo
feature, and the model-only replay comparator excludes native-only publication
reuse counters. Gameplay/controller state still compares exactly at every step.
