# Changelog

All notable changes to the **ChordLens** project will be documented in this file.

## [0.1.3] - 2026-04-04

### Added
- **Maximized Hero Display:** Huge **128px** font for the chord root, making harmonic information the focal point of the interface.
- **High-Contrast Nashville:** White/Near-white coloring for Nashville numerals at **48px** for instant readability.
- **High-Visibility Note List:** Increased active note font to **28px** with optimized, compact horizontal spacing.
- **Togglable Octave Coloring:** Architecture implemented for split-color pitch/octave display (disabled by default in code).
### Changed
- **Subtle Layout Compaction:** Bottom bar height reduced to **70.0** to frame the main typography more effectively.
- **Improved Alignment:** Manual centering offsets recalibrated for the enlarged typography in the 480x300 canvas.

## [0.1.2] - 2026-04-04

### Added
- **Simplified Octave Display:** Multiple octaves of the same pitch class (e.g., C2 + C3) now correctly display as a single unified note name (e.g., "C").
### Changed
- **Clean UI State:** Removed the `-` placeholder from the root name when no MIDI notes are active; the display is now cleanly empty until input is received.
- **Improved README:** Musician-oriented documentation with feature highlights and screenshot.

## [0.1.1] - 2026-04-04

### Changed
- **Maximized Typography:** Increased initial font sizes (96px for Root, 48px for Nashville) for better visibility.
- **Asset Optimization:** Removed dozens of unused font variants from the `assets/` folder to reduce plugin footprint.
### Fixed
- **Linux Build Environment:** Added missing `libx11-xcb-dev` system dependency for GitHub Actions.

## [0.1.0] - 2026-04-04

### Added
- **Initial Alpha Release:** Core harmonic detection engine for chords and scales.
- **Nashville Numbering System:** Real-time degree calculation relative to the detected key.
- **Cross-Platform Release Automation:** Automated builds for Windows, macOS, and Linux via GitHub Actions.
- **Stable 480x300 Layout:** Fixed-size VST3/CLAP GUI designed for minimal hosting glitches.
- **Color-Coded Note Roles:** Visual distinction between scale-tonic, scale-step, and chromatic notes.
