# Iced Blocks

An independent Iced 0.14 implementation of the native Rust `twinkle-blocks`
benchmark scenario. It keeps the same game rules, deterministic seven-piece
bag, 10×20 declarative cell grid, colors, dimensions, update cadence, menu, and
keyboard controls so the two UI runtimes can be compared with equivalent
application work.

Run the optimized app from this repository root:

```sh
cargo run --release -p iced-blocks
```

Run the headless deterministic validation path with:

```sh
cargo run --release -p iced-blocks -- --validate
```

For an apples-to-apples runtime benchmark, build both packages in release mode
and measure them with the same display server, compositor state, window size,
input script, and sampling interval. The Iced port deliberately rebuilds all
200 cell widgets on each view pass, matching the straightforward declarative
shape of the non-JSX Twinkle app.
