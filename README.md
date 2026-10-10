# Twinkle test apps

This repository contains small standalone applications built with the Twinkle
UI engine in the sibling `../twinkle` checkout. Their purpose is to exercise
Twinkle as an application developer would and record cases where Twinkle's
documentation does not give enough information to use an API confidently.

## Apps

- [`twinkle-notes`](apps/twinkle-notes/README.md): a simple Notepad-style
  multiline text editor with path-based open and save.

## Run

From this repository root, run an app with `cargo run -p twinkle-notes`.
Twinkle's getting-started guide covers the native platform prerequisites.

Documentation gaps and any source searches needed to resolve them are logged
in [`TWINKLE_DOC_GAPS.md`](TWINKLE_DOC_GAPS.md).
