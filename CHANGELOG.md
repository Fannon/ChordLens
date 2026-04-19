# Changelog

All notable changes to the **ChordLens** project will be documented in this file.
This file is for user-visible changes. Contributor workflow belongs in `CONTRIBUTING.md`.

## [Unreleased]

### Fixed
- **Overlapping MIDI Note Lifetime:** Duplicate `NoteOn` events for the same pitch now stay active until the last matching `NoteOff`, preventing stacked or retriggered notes from dropping out of chord detection too early.
- **Flat-Key Note Coloring:** The editor now parses displayed note labels with a shared longest-prefix matcher, so `Bb`, `Eb`, and `Ab` keep the correct scale-aware coloring in the main chord display, chord history, and active-note list.

### Added
- **Regression Coverage for Layered Notes:** Added runtime and process-path tests that verify duplicate note lifetimes survive a single release and keep the detected chord stable.
- **Regression Coverage for Enharmonic UI Labels:** Added parser tests that lock down flat and sharp label handling, including octave and suffix forms such as `Eb3` and `Abmaj7`.

## [0.1.8] - 2026-04-05

### Added
- **Chromatic Key Mode:** Added a `Chromatic` option next to `Auto` in the key selector for free chromatic note viewing without scale-based harmony context.

### Changed
- **Refined Key Tracking Engine:** Auto key detection now uses a more structured scoring pipeline with decaying note evidence, chord-context weighting, and confidence scoring for steadier musical results.
- **Improved Debug Tuning Tools:** Debug builds can now show top key candidates and confidence in the plugin UI when `CHORDLENS_DEBUG_KEYS=1` is set.
- **Chromatic Mode Display Behavior:** Chromatic mode now keeps normal chord detection and chord naming while disabling Nashville notation and scale-position note coloring.
- **Chromatic Root Highlighting:** In chromatic mode, the detected root note still uses the main accent color so the display keeps a clear tonal anchor.

## [0.1.7] - 2026-04-05

### Changed
- **More Reliable Release Packaging:** Release artifacts now use NIH-plug's native bundling flow for VST3 and CLAP packaging instead of simple renamed library files.
- **Footer Spacing Polish:** Chord history now sits closer to the bottom edge with more balanced spacing, and the note list is positioned slightly lower for better visual alignment.
- **Smarter Auto Key Tracking:** Key detection now reacts faster to real tonal changes while staying steadier through brief wrong notes, passing tones, and bluesy color notes.
- **Held Notes Matter More:** Sustained notes now reinforce the detected key instead of letting short accidental notes dominate the result.
- **Less Key Flicker:** Auto-detected keys now confirm before the display changes, so the visible key stays more stable during transitions.
- **More Musical Context:** Repeated tonic, dominant, and common cadential motion now help the detector prefer musically likely keys.
- **Improved Mode Stability:** Auto detection is more conservative about changing mode when the tonal root still appears stable.
- **Host-Controllable Key Tracking:** Added a DAW automatable `Key Tracking` parameter with `Stable`, `Balanced`, and `Reactive` response modes.

## [0.1.6] - 2026-04-04

### Added
- **Seamless Layout:** Redesigned the UI architecture to use a single-panel layout, resolving visual background gaps.
- **Dynamic View Toggle:** Replaced the "H" button with a label that changes between "History" and "Notes" to reflect the current view.
- **Improved Legibility:** Inversion hints (e.g. "1st inv.") are now larger (14px) and use a regular font style.

### Changed
- **Unified Controls:** Standardized the height of all buttons and dropdowns to 22px for consistent alignment.
- **Improved Spacing:** Added padding for Nashville numbers and adjusted header margins to prevent text from clipping at window edges.
- **Simplified Parameters:** Shortened DAW parameter names for cleaner integration (e.g., "Chord History" and "Root-less Voicings").
- **Compact History View:** Adjusted margins in the history panel to display more harmonic data at once.

## [0.1.5] - 2026-04-04

### Added
- **Extended Jazz Palette:** Support for 11th, 13th, and Altered Dominant chords (e.g., `7b9`, `7#9`, `7#11`, `9#11`, `9b13`).
- **Key-Aware Enharmonic Naming:** Intelligent flat/sharp selection based on the detected musical key (e.g., Bb in F Major).
- **Configurable Accumulation:** Added an **Acc: [ms]** control (0ms–100ms) to the UI to stabilize detection during live performance.
- **Experimental Root-less Detection:** Optional toggle ("RL") to enable heuristic root inference (e.g., identifying G13 from F, A, B, E).
- **Comprehensive Test Suite:** Decoupled unit tests into a dedicated `tests.rs` with extensive coverage for musical edge cases.

### Changed
- **Improved Nashville/Roman Numerals:** Corrected case-handling for minor, diminished, and half-diminished degrees (e.g., `ii`, `øvii`).
- **Accurate Inversion Detection:** Refined logic to correctly name 1st, 2nd, and 3rd inversions based on the scale degree in the bass.
- **Refined Dyad Notation:** Two-note structures are now more cleanly represented as `m(no5)` or `(no5)`.
- **Robust Detection Engine:** Redesigned the matching heuristic to effectively handle standard jazz omissions (like the 5th).

## [0.1.4] - 2026-04-04

### Changed
- **Optimized Distribution:** Shifted to OS-specific ZIP bundles (`ChordLens_Windows.zip`, etc.) to provide a cleaner installation experience with native file naming.
- **Project Identity:** Official Vendor and Author set to **Simon Heimler**.

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
