# AGENTS

ChordLens is a minimalist MIDI chord and key detector plugin for producers, songwriters, and musicians. It listens to incoming MIDI, identifies the current chord, estimates the active key, and can show Nashville notation in real time.

- Never commit, push, merge, or publish anything unless explicitly asked.
- Use a tight verify loop after changes: `cargo fmt`, `cargo test`, `cargo check`, `cargo clippy --all-targets --all-features -- -D warnings`.
- Put temp files, scratch notes, local bundles, and one-off artifacts in `./tmp/`.
- This project is a Rust `nih-plug` MIDI plugin. `nih_export_vst3!` and `nih_export_clap!` expose the plugin entry points, and `cargo xtask bundle chord-lens --release` is the correct way to build distributable VST3/CLAP bundles for this repo.
- Do not invent ad-hoc packaging by renaming raw `dll` / `so` / `dylib` outputs. Use the `nih-plug` bundler and validate final host compatibility before release.
- Keep `README.md` and `CHANGELOG.md` user-focused. Put developer workflow in `README.md`'s `Development` section or in `CONTRIBUTING.md`.
- Release work is opt-in only. When explicitly asked, update user-facing docs as needed, review `CHANGELOG.md`, build release artifacts, verify them, create the requested tag/release assets, and stop short of any git push unless that was also explicitly requested.
