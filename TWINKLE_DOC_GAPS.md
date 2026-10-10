# Twinkle documentation gaps

This file records places where Twinkle's in-repository documentation was
insufficient for building the test apps. Source searches are included only
when the documentation did not resolve the question.

## 2026-10-10 — Multiline text editing

- Documentation reviewed: `docs/getting-started.md`, `docs/task-guides.md`,
  and `crates/twinkle/README.md` describe `TextField` for ordinary forms, but
  do not say whether a field can edit multiple lines, how Enter is handled, or
  how line breaks affect layout and caret hit testing.
- Source searched: `crates/twinkle/src/ui/components.rs`,
  `crates/twinkle/src/text_editor.rs`, and the multiline cases in
  `crates/twinkle/src/ui/tree/tests.rs`; also the `VerticalScroll` component
  implementation and exports in `crates/twinkle/src/lib.rs`.
- Finding: `TextField::on_change` stores the supplied value in the shared text
  editor; embedded newlines are supported. The UI tree tests cover multiline
  caret and hit testing. The API defaults to wrapping text and exposes
  `.wrap(bool)`.
- Documentation gap: a short text-editor recipe should explain multiline
  `TextField` setup, wrapping, sizing/scrolling, and keyboard behavior. It
  should also show `VerticalScroll` construction: its initial message is
  required even when the app only needs the later `on_scroll` message mapper,
  and it is exported from the crate root rather than the prelude.
- Follow-up source search for Enter handling: `crates/twinkle/src/input.rs`
  and `crates/twinkle/src/runtime.rs` show that focused text fields receive a
  newline from Shift+Enter, while plain Enter becomes the application
  `Submit` shortcut and then falls back to keyboard activation. Existing
  multiline tests cover text input, caret placement, and selection, but do not
  cover pressing Enter. The app does not discard newline characters.
- Likely behavior bug: for a multiline editor such as Twinkle Notes, plain
  Enter should ordinarily insert a newline. Twinkle currently requires
  Shift+Enter, and the docs do not state this keyboard contract.

## 2026-10-10 — Opening and saving ordinary files

- Documentation reviewed: `docs/task-guides.md` advises moving heavy file work
  off the synchronous `Application::update` loop, but does not provide an
  example of simple local document loading and saving or a file-dialog API.
- Source searched: public `twinkle-ui` exports and dependencies in
  `crates/twinkle/src/lib.rs` and `crates/twinkle/Cargo.toml`; searched those
  sources for `FileDialog`, file picker, open-file, and save-file APIs.
- Finding: the core UI crate provides no file picker or document-storage API;
  file access belongs to the app. This test app uses standard Rust file I/O
  from explicit path input and keeps the example dependency-free.
- Documentation gap: an app recipe could show the boundary between Twinkle's
  synchronous UI update loop and small local file operations, and point to a
  supported native file-dialog integration for full desktop workflows.

## 2026-10-10 — Local development against a Twinkle checkout

- Documentation reviewed: `docs/getting-started.md` documents adding a pinned
  Git dependency while Twinkle is pre-1.0.
- Finding: this test-app repository uses a Cargo workspace path dependency on
  the sibling `../twinkle` checkout so app changes can be compiled against the
  local engine immediately.
- Documentation gap: distinguish the recommended pinned Git dependency for
  external consumers from a path dependency for local Twinkle development.

## 2026-10-10 — Standalone app surface colors

- Documentation reviewed: the semantic visual system section of
  `crates/twinkle/README.md` describes semantic theme tokens, but the getting
  started example does not explain the default foreground/background of a
  standalone application or how to set a complete light surface.
- Source searched: `Color`, `Background`, and `foreground` / `background`
  methods on the root layout and `TextField` components in
  `crates/twinkle/src/ui.rs` and `crates/twinkle/src/ui/components.rs`; focused
  background resolution in `crates/twinkle/src/ui/tree/layout.rs`.
- Finding: explicitly styling the root layout and editor fields makes this
  app's paper-like light appearance stable across the runtime's default theme.
  When typing follows a pointer click, Twinkle changes the current input
  modality to Keyboard; its automatic focus tint then blends the fallback
  keyboard-focus cue into the field's white background, making the editing
  area appear gray. A pointer-focused editor should not acquire the keyboard
  focus color just because the user types into it. `TextField::focus_background`
  can provide an exact focused background, but it does not distinguish how
  focus was acquired.
- Documentation gap: show a small light/dark application surface example or
  explain how semantic theme resolution reaches standalone app roots. Also
  document the default keyboard focus tint and when it applies.
- Likely behavior bug: focus tint is selected from the latest input modality,
  not the modality that established focus. Source searched:
  `crates/twinkle/src/ui/tree.rs` (input-source modality updates) and
  `crates/twinkle/src/ui/tree/layout.rs` (focused tint selection). This means
  typing in a text field that was clicked changes it to the keyboard focus
  treatment. The docs do not explain this distinction or whether pointer focus
  should remain visually neutral during text entry.
