# Twinkle test apps

This repository contains small standalone applications built with the Twinkle
UI engine. Their purpose is to exercise Twinkle as an application developer
would and record cases where Twinkle's documentation does not give enough
information to use an API confidently.

## Apps

- [`twinkle-security`](apps/twinkle-security/README.md): a deterministic synthetic
  security dashboard with four 10 Hz graphs and heatmaps up to 20,000 cells, plus
  native submission and Wayland presentation benchmarks.

- [`twinkle-notes`](apps/twinkle-notes/README.md): a simple Notepad-style
  multiline text editor with path-based open and save.
- [`twinkle-blocks`](apps/twinkle-blocks/README.md): a Rust-only falling-block
  puzzle with an application-owned render buffer and direct keyboard and
  controller input.
- [`twinkle-blocks-jsx`](apps/twinkle-blocks-jsx/README.md): the equivalent
  falling-block puzzle with its presentation authored in TSX.

## Benchmarks

- [`iced-blocks`](benchmarks/iced-blocks/README.md): an independent Rust/Iced
  falling-block implementation for framework comparisons.

## Run

From this repository root, run an app with `cargo run --release -p twinkle-security`,
`cargo run -p twinkle-notes`,
`cargo run -p twinkle-blocks`, `cargo run -p twinkle-blocks-jsx`, or
`cargo run -p iced-blocks`.
Twinkle's getting-started guide covers the native platform prerequisites.

Documentation gaps and any source searches needed to resolve them are logged
in [`TWINKLE_DOC_GAPS.md`](TWINKLE_DOC_GAPS.md).
