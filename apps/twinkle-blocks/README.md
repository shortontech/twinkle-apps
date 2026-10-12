# Twinkle Blocks (native Rust view)

A deliberately straightforward, Rust-only version of the falling-block test
app. Rust owns the game rules and constructs the complete 10×20 declarative
cell grid on every view pass. It does not link or initialize the JSX runtime.

This exists beside `twinkle-blocks-jsx` so their CPU usage can be compared with
the same game logic, controls, presentation shape, and update cadence.

Run from the repository root:

```sh
cargo run -p twinkle-blocks
```

Run its deterministic validation path with:

```sh
cargo run -p twinkle-blocks -- --validate
```
