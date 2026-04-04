# ChordLens

**ChordLens** is an elegant, real-time MIDI chord detector audio plugin. It intercepts incoming MIDI note events, calculates the musical chord being held, and displays it in a minimalistic, typography-focused UI without any audio DSP overhead.

## Features

- **Interval-Math Detection**: Uses pure pitch-class sets and interval lookups to avoid external dependency bloat.
- **Advanced Chord Support**: Detects major, minor, augmented, diminished, 7ths, 9ths, 11ths, and 13ths.
- **Pragmatic Voicings**: Automatically detects and informs the user of omitted functional notes logically (e.g. `13(no11)`, `m9(no5)`) suitable for realistic guitar shell voicings or keyboard reductions.
- **Inversion & Slash Tracking**: Correctly calculates inversions and tracks alternate bass notes seamlessly (`C/E`, `1st inv.`).
- **Thread-Safe**: Strictly decoupled audio and GUI loops utilizing lock-free snapshots so audio threads are never preempted.

---

## Developer Instructions

It is built with Rust, `nih-plug`, and `egui`.

### Prerequisites
1. Installed **Rust** toolchain (via [rustup](https://rustup.rs/)). 
2. A host DAW that supports **CLAP** or **VST3** plugins.

### Building
The easiest way to compile the source code is natively through Cargo.

```bash
# Build the optimized release binary
cargo build --release
```

Thanks to the aggressive compiler configurations in `Cargo.toml`, ChordLens will compile stripped of symbols and optimized for file size, yielding a lightweight ~3.4MB plugin.

### Installation
Upon a successful build, the target library will be generated inside the `/target/release/` directory.

- **Windows:** You will find `chord_lens.dll`. Rename the extension to `.vst3` or `.clap` respectively, and drop it into your plugin folder.
- **macOS:** You will find `libchord_lens.dylib`. Rename it to `.vst3` or `.clap`.
- **Linux:** You will find `libchord_lens.so`. Rename it to `.vst3` or `.clap`.

For example, to quickly generate a ready-to-test `.clap` on Windows:
```bash
mkdir -p tmp
cp target/release/chord_lens.dll tmp/chord_lens.clap
```
*(You can then point your DAW to this dummy directory or copy it into `%COMMONPROGRAMFILES%/CLAP`)*.

### Testing
Because the complex interval detection algorithms are strictly standalone functions decoupled from the audio engine, you can exhaustively unit test chord detection scenarios effortlessly:

```bash
cargo test --lib
```
