**Review**

**Findings**
- DONE: runtime note tracking now uses fixed `[ActiveNoteState; 128]` counters, compact chord/history snapshots, and GUI-side note/chord formatting, so `process()` no longer rebuilds per-note label vectors or clones string-heavy history entries on the audio thread. The module docs were also corrected to stop overstating real-time guarantees.
- DONE: active-note tracking no longer collapses overlapping notes of the same pitch. Runtime note state now keeps an instance count per pitch, and regression tests cover duplicate `NoteOn`/`NoteOff` ordering through both `KeyHistory` and the plugin `process()` path.
- DONE: auto key detection now re-runs on a timer while active notes, pending key changes, or decaying evidence remain, so held-note reinforcement and evidence decay can keep updating the visible key state between MIDI events. Process-path regression coverage verifies that key state refresh now happens without new note input.
- DONE: flat-note UI parsing now uses a shared longest-prefix note-name matcher, so `Bb`, `Eb`, `Ab`, and similar labels no longer fall through to natural-note colors in the main chord display, history view, or active-note list. Regression tests cover flat and sharp labels, including octave/suffix forms.
- DONE: `nih_plug` and `nih_plug_egui` are now pinned to the validated git revision in `Cargo.toml`, and `CONTRIBUTING.md` documents the exact upgrade workflow for future dependency bumps.
- DONE: the docs now describe the overlay as Roman numerals, explicitly document `Key Tracking`, `Roman Numerals`, and `Root-less Voicings` as host-only parameters, and remove the stale implication that the egui editor exposes a visible “Nashville mode”.

**Recommended Improvements**
- Move audio-thread state to fixed-size, allocation-free data: note counters, root/mode enums, confidence, and small ring buffers. Format chord strings, history labels, and note names on the GUI thread instead of in `process()`.
- Replace `HashMap<u8, u32>` with either `[u16; 128]` reference counts or a `(channel, note)` keyed structure. Add regression tests for duplicate `NoteOn`/`NoteOff` ordering.
- Re-run key estimation on a timer while evidence is nonzero or active notes are held. That would make the decay/held-note model actually influence the visible result.
- Stop reparsing note names in the UI. Store pitch classes directly in `ChordInfo`, or reuse a single canonical note-name parser shared with key detection.
- Pin `nih_plug` and `nih_plug_egui` revisions, then document the exact upgrade procedure in `CONTRIBUTING.md`.
- Clean up the user docs: rename “Nashville Numbers” to “Roman Numerals” unless you intend to implement real Nashville notation, document which parameters are host-only vs visible in the plugin UI, and remove stale changelog claims like the old `Acc: [ms]` control in [CHANGELOG.md](/C:/Development/chord-lens/CHANGELOG.md:47).
- Expand testing around the gaps the current suite misses: overlapping-note lifetime, sustained-note key re-evaluation, flat-key UI parsing, editor startup, state recall, and at least one non-Windows host/bundle smoke test.
- Expand testing around the remaining host-only gap: automated non-Windows host/bundle smoke coverage is still manual, but the Rust suite now covers overlapping-note lifetime, sustained key refresh, flat-key labeling, editor startup, and persisted editor-state recall.

`cargo fmt --check`, `cargo test`, `cargo check`, and `cargo clippy --all-targets --all-features -- -D warnings` all passed in the current tree, so the main issues are behavioral, real-time, and documentation-related rather than basic build breakage.
