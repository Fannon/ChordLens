//! # ChordLens — Real-time MIDI Chord Detector  (VST3 + CLAP)
//!
//! ## Architecture
//!
//! ```
//!  ┌──────────────────────────────────────────────┐
//!  │              Host DAW / plugin runner         │
//!  │                                               │
//!  │  ┌──────────────┐       ┌───────────────────┐ │
//!  │  │  Audio/MIDI  │       │   GUI Thread      │ │
//!  │  │  Thread      │       │  (egui editor)    │ │
//!  │  │              │  Arc  │                   │ │
//!  │  │  process()   │──────▶│  draw_frame()     │ │
//!  │  │  → detect()  │  RwLock│                  │ │
//!  │  │  → write     │       │  → read snapshot  │ │
//!  │  └──────────────┘       └───────────────────┘ │
//!  └──────────────────────────────────────────────┘
//! ```
//!
//! The **only** shared mutable data is `Arc<RwLock<ChordState>>`.
//! The audio thread *writes* once per buffer (low-frequency); the GUI thread
//! *reads* once per frame (~60 Hz).  `parking_lot::RwLock` does not allocate
//! on the uncontended fast path, making this safe from a real-time perspective
//! for a **MIDI-only** plugin (no audio DSP computed here).
//!
//! ## Chord detection
//!
//! See `chord.rs` for the interval-math details.  The short story:
//!  - Active MIDI note numbers are stored in a `HashSet<u8>`.
//!  - On every note event, `chord::detect()` rebuilds the `ChordInfo`.
//!  - The new `ChordInfo` is written to `ChordState` under the write-lock.

#![allow(non_snake_case)] // nih-plug macros use non-snake names

mod chord;
mod editor;

use chord::{ChordInfo, detect};
use editor::{EDITOR_HEIGHT, EDITOR_WIDTH};

use nih_plug::prelude::*;
use nih_plug_egui::EguiState;
use parking_lot::RwLock;
use std::collections::HashSet;
use std::sync::Arc;

// ─── Shared state between audio thread and GUI ────────────────────────────────

/// The single piece of data that crosses the audio↔GUI thread boundary.
/// Written by `process()`, read by the egui paint callback.
#[derive(Clone, Default)]
pub struct ChordState {
    pub chord_info: ChordInfo,
}

// ─── Plugin struct ────────────────────────────────────────────────────────────

pub struct ChordLens {
    params: Arc<ChordLensParams>,

    /// Chord information written by the audio thread, read by the GUI.
    chord_state: Arc<RwLock<ChordState>>,

    /// MIDI note numbers currently held down.  Lives only on the audio thread,
    /// so no synchronisation needed.
    active_notes: HashSet<u8>,
}

// ─── Parameters ──────────────────────────────────────────────────────────────

#[derive(Params)]
pub struct ChordLensParams {
    /// The editor window size is persisted so it survives plugin reloads.
    #[persist = "editor-state"]
    editor_state: Arc<EguiState>,
}

impl Default for ChordLensParams {
    fn default() -> Self {
        Self {
            editor_state: EguiState::from_size(EDITOR_WIDTH, EDITOR_HEIGHT),
        }
    }
}

// ─── Plugin wiring ────────────────────────────────────────────────────────────

impl Default for ChordLens {
    fn default() -> Self {
        Self {
            params: Arc::new(ChordLensParams::default()),
            chord_state: Arc::new(RwLock::new(ChordState::default())),
            active_notes: HashSet::new(),
        }
    }
}

impl Plugin for ChordLens {
    const NAME: &'static str = "ChordLens";
    const VENDOR: &'static str = "ChordLens Authors";
    const URL: &'static str = "https://github.com/your-username/chord-lens";
    const EMAIL: &'static str = "";

    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    // ── MIDI-only: no audio I/O ───────────────────────────────────────────────
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[];

    // We need NoteOn / NoteOff; MidiCCs gives us those plus CC messages.
    // If the host supports MIDI2-style note expressions we still handle them
    // through the NoteOn/NoteOff arms below.
    const MIDI_INPUT: MidiConfig = MidiConfig::MidiCCs;
    // Pass events through transparently so ChordLens is non-destructive.
    const MIDI_OUTPUT: MidiConfig = MidiConfig::MidiCCs;

    const SAMPLE_ACCURATE_AUTOMATION: bool = false;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    // ── Editor ────────────────────────────────────────────────────────────────

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        editor::create(
            self.params.editor_state.clone(),
            self.chord_state.clone(),
        )
    }

    // ── Process (MIDI event loop) ─────────────────────────────────────────────

    fn process(
        &mut self,
        _buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        // Track whether the active note set changed this buffer so we only
        // run chord detection (and acquire the write-lock) when necessary.
        let mut changed = false;

        while let Some(event) = context.next_event() {
            match event {
                // ── Note ON ───────────────────────────────────────────────────
                NoteEvent::NoteOn { note, velocity, .. } if velocity > 0.0 => {
                    self.active_notes.insert(note);
                    changed = true;
                    // Pass event through so the plugin is transparent
                    context.send_event(event);
                }

                // ── Note OFF (and zero-velocity NoteOn which is a NoteOff) ──
                NoteEvent::NoteOff { note, .. } => {
                    self.active_notes.remove(&note);
                    changed = true;
                    context.send_event(event);
                }
                NoteEvent::NoteOn { note, velocity, .. } if velocity == 0.0 => {
                    self.active_notes.remove(&note);
                    changed = true;
                    context.send_event(event);
                }

                // ── Choke (all notes off for a voice) ────────────────────────
                NoteEvent::Choke { note, .. } => {
                    self.active_notes.remove(&note);
                    changed = true;
                    context.send_event(event);
                }

                // ── All other events pass through unchanged ───────────────────
                other => {
                    context.send_event(other);
                }
            }
        }

        // Update shared chord state only when active notes actually changed.
        if changed {
            let notes: Vec<u8> = self.active_notes.iter().copied().collect();

            // Detect chord — pure computation, no allocation of concern
            let chord_info = detect(&notes);

            // Write to the shared state.  The GUI thread reads this under
            // a read-lock; the write-lock is held for a single clone assignment.
            *self.chord_state.write() = ChordState { chord_info };
        }

        ProcessStatus::Normal
    }
}

// ─── Plugin exports ───────────────────────────────────────────────────────────

impl ClapPlugin for ChordLens {
    const CLAP_ID: &'static str = "io.github.chord-lens.chord-lens";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("Real-time MIDI chord detector with egui display");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::NoteEffect,
        ClapFeature::Analyzer,
        ClapFeature::Utility,
    ];
}

impl Vst3Plugin for ChordLens {
    const VST3_CLASS_ID: [u8; 16] = *b"ChordLens_000001";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Tools, Vst3SubCategory::Analyzer];
}

nih_export_clap!(ChordLens);
nih_export_vst3!(ChordLens);
