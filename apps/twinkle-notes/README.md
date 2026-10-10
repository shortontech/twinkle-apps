# Twinkle Notes

A standalone Twinkle app for testing multiline text editing. It provides a
single editor, a path field, New/Open/Save actions, a line and character count,
and visible file-operation status.

Set a path such as `notes.txt`, then choose Open to load it or Save to write
the current draft. Paths are interpreted relative to the process working
directory unless absolute. New clears the editor and path.

Run from the repository root:

```sh
cargo run -p twinkle-notes
```

See the root [`TWINKLE_DOC_GAPS.md`](../../TWINKLE_DOC_GAPS.md) for Twinkle API
documentation questions surfaced while building this app.
