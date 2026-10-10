# Twinkle Blocks

A small falling-block puzzle written as a test app for Twinkle. The menu and
board presentation are authored in TSX and hosted by Rust through
`twinkle-jsx-runtime`; the Rust app owns the game rules and a 10×20 cell buffer
that it publishes to the JSX tree for rendering.

While gameplay is active, D-pad and left-stick movement, down, rotation, hard
drop, and the Start-button game menu are handled through Twinkle's direct
physical controller hook.
When paused or while the JSX menu is open, physical input falls back to normal
Twinkle controller navigation and activation. Keyboard shortcuts and automatic
piece fall use Twinkle's application hooks.

The app starts falling pieces immediately. Press controller **Start** or activate
the visible **Game menu** button to pause, resume, or begin a new game after
top-out.

Run from the repository root:

```sh
cargo run -p twinkle-blocks
```

This app opts into the V8-backed JSX runtime, so its build is intentionally
heavier than the Rust-only Notes app.
