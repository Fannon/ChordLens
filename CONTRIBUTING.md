# Contributing

## Scope

This file is for developer-facing workflow. Keep `README.md` and `CHANGELOG.md` focused on end users and release-visible changes.

## Verify Loop

Use a short verify loop while working:

1. `cargo fmt`
2. `cargo test`
3. `cargo check`
4. `cargo clippy --all-targets --all-features -- -D warnings`

Run the full loop before handing changes off. If you only changed docs, say so explicitly instead of pretending code was revalidated.

## Temp Files

Put temporary notes, screenshots, local plugin copies, release snapshots, and other disposable artifacts in `./tmp/`. Do not scatter temp files across the repo root.

## Building

### Local dev build

- `cargo build`
- `cargo build --release`
- `cargo xtask bundle chord-lens --release`

Helper scripts:

- Windows: `build.bat`
- Unix-like: `build.sh`

Those scripts are convenience wrappers. They use NIH-plug's bundler, copy bundled artifacts into `./bin/`, and write timestamped snapshots under `./tmp/`.

### Rust / VST3 / CLAP facts for this repo

- The crate uses `nih-plug` and exports both plugin formats from [`src/lib.rs`](/C:/Development/chord-lens/src/lib.rs).
- `crate-type = ["cdylib", "lib"]` is required so Rust emits a loadable plugin library while tests and normal library tooling still work.
- `nih_export_vst3!(ChordLens)` and `nih_export_clap!(ChordLens)` generate the format-specific entry points.
- `cargo build --release` gives you the platform-native dynamic library:
  - Windows: `target/release/chord_lens.dll`
  - macOS: `target/release/libchord_lens.dylib`
  - Linux: `target/release/libchord_lens.so`
- `cargo xtask bundle chord-lens --release` is the repo's correct packaging path. It writes bundled artifacts under `target/bundled/`.
- `bundler.toml` controls the human-facing bundle name used by the NIH-plug bundler.
- Host-facing VST3 / CLAP artifacts are packaging concerns on top of the shared library. Different hosts and platforms are stricter than simple file renaming, so final release artifacts should always be produced through the bundler and then tested in a real host.
- This is a MIDI utility plugin, not an audio DSP processor. Real-time safety still matters because chord detection happens inside `process()`.

## Testing

The current automated coverage is mostly unit-level chord detection logic in [`src/tests.rs`](/C:/Development/chord-lens/src/tests.rs). There is no host-level automated test for plugin loading, editor startup, parameter persistence, or DAW interoperability.

Manual smoke testing matters:

1. Build a release artifact.
2. Load VST3 and CLAP versions in at least one supported host.
3. Verify MIDI input, key detection, history view, manual key lock, and editor rendering.
4. Confirm the host can rescan and reopen the plugin cleanly.

## Releases

Only do release work when explicitly asked.

Current repository flow:

1. Review `README.md` and `CHANGELOG.md` for user-facing accuracy.
2. Run the full verify loop.
3. Build release artifacts with `cargo xtask bundle chord-lens --release`.
4. Validate artifacts in a host if possible.
5. Use the GitHub Actions workflow in [`.github/workflows/release.yml`](/C:/Development/chord-lens/.github/workflows/release.yml) or the requested manual process.
6. Create tags, releases, commits, pushes, or merges only if that exact action was requested.
