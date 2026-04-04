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
#[cfg(test)]
mod tests;

use chord::{ChordInfo, detect};
use editor::{EDITOR_HEIGHT, EDITOR_WIDTH};

use nih_plug::prelude::*;
use nih_plug_egui::EguiState;
use parking_lot::RwLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::collections::{HashSet, VecDeque};

#[derive(Debug, PartialEq, Eq, Enum, Clone, Copy)]
pub enum KeyRoot {
    #[name = "Auto"] Auto,
    #[name = "C"] C, #[name = "C#"] CSharp, #[name = "D"] D, #[name = "D#"] DSharp,
    #[name = "E"] E, #[name = "F"] F, #[name = "F#"] FSharp, #[name = "G"] G,
    #[name = "G#"] GSharp, #[name = "A"] A, #[name = "A#"] ASharp, #[name = "B"] B,
}

impl KeyRoot {
    pub fn as_str(&self) -> &'static str {
        match self {
            KeyRoot::Auto => "Auto", KeyRoot::C => "C", KeyRoot::CSharp => "C#",
            KeyRoot::D => "D", KeyRoot::DSharp => "D#", KeyRoot::E => "E",
            KeyRoot::F => "F", KeyRoot::FSharp => "F#", KeyRoot::G => "G",
            KeyRoot::GSharp => "G#", KeyRoot::A => "A", KeyRoot::ASharp => "A#",
            KeyRoot::B => "B",
        }
    }
    pub fn pc_val(&self) -> u8 {
        match self {
            KeyRoot::Auto => 0, KeyRoot::C => 0, KeyRoot::CSharp => 1, KeyRoot::D => 2,
            KeyRoot::DSharp => 3, KeyRoot::E => 4, KeyRoot::F => 5, KeyRoot::FSharp => 6,
            KeyRoot::G => 7, KeyRoot::GSharp => 8, KeyRoot::A => 9, KeyRoot::ASharp => 10,
            KeyRoot::B => 11,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Enum, Clone, Copy)]
pub enum KeyMode {
    #[name = "Major"] Major,
    #[name = "Minor"] Minor,
    #[name = "Dorian"] Dorian,
    #[name = "Phrygian"] Phrygian,
    #[name = "Lydian"] Lydian,
    #[name = "Mixolydian"] Mixolydian,
    #[name = "Locrian"] Locrian,
}

impl KeyMode {
    pub fn intervals(&self) -> Vec<i32> {
        match self {
            KeyMode::Major => vec![0, 2, 4, 5, 7, 9, 11],
            KeyMode::Minor => vec![0, 2, 3, 5, 7, 8, 10],
            KeyMode::Dorian => vec![0, 2, 3, 5, 7, 9, 10],
            KeyMode::Phrygian => vec![0, 1, 3, 5, 7, 8, 10],
            KeyMode::Lydian => vec![0, 2, 4, 6, 7, 9, 11],
            KeyMode::Mixolydian => vec![0, 2, 4, 5, 7, 9, 10],
            KeyMode::Locrian => vec![0, 1, 3, 5, 6, 8, 10],
        }
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            KeyMode::Major => "Major", KeyMode::Minor => "Minor", KeyMode::Dorian => "Dorian",
            KeyMode::Phrygian => "Phrygian", KeyMode::Lydian => "Lydian",
            KeyMode::Mixolydian => "Mixolydian", KeyMode::Locrian => "Locrian",
        }
    }
}

// ─── Shared state between audio thread and GUI ────────────────────────────────

/// The single piece of data that crosses the audio↔GUI thread boundary.
/// Written by `process()`, read by the egui paint callback.
#[derive(Clone, Default)]
pub struct ChordState {
    pub chord_info: ChordInfo,
    pub key_text: String,
    pub scale_root: u8,
    pub scale_intervals: Vec<i32>,
    pub nashville_text: String,
}

// ─── Plugin struct ────────────────────────────────────────────────────────────

pub struct ChordLens {
    params: Arc<ChordLensParams>,

    /// Chord information written by the audio thread, read by the GUI.
    chord_state: Arc<RwLock<ChordState>>,

    /// MIDI note numbers currently held down.  Lives only on the audio thread,
    /// so no synchronisation needed.
    active_notes: HashSet<u8>,
    history_midi: VecDeque<u8>,
    last_detected_key: String,
    
    /// Timer for debouncing chord detection (in samples)
    debounce_samples_remaining: u32,
    /// Pending changes to active notes that haven't been "detected" yet
    notes_changed: bool,
    
    // Shared reset flag for the UI button
    pub reset_history: Arc<AtomicBool>,
}

// ─── Parameters ──────────────────────────────────────────────────────────────

#[derive(Params)]
pub struct ChordLensParams {
    #[persist = "editor-state"]
    pub editor_state: Arc<EguiState>,
    
    #[id = "force_r"]
    pub key_root: EnumParam<KeyRoot>,
    
    #[id = "force_m"]
    pub key_mode: EnumParam<KeyMode>,

    #[id = "show_nashville"]
    pub show_nashville: BoolParam,

    #[id = "allow_rootless"]
    pub allow_rootless: BoolParam,

    #[id = "debounce_ms"]
    pub debounce_ms: FloatParam,
}

impl Default for ChordLensParams {
    fn default() -> Self {
        Self {
            editor_state: EguiState::from_size(EDITOR_WIDTH, EDITOR_HEIGHT),
            key_root: EnumParam::new("Force Root", KeyRoot::Auto),
            key_mode: EnumParam::new("Force Mode", KeyMode::Major),
            show_nashville: BoolParam::new("Nashville Numbers", true),
            allow_rootless: BoolParam::new("Root-less Voicings (Experimental)", false),
            debounce_ms: FloatParam::new("Accumulation (ms)", 25.0, FloatRange::Linear { min: 0.0, max: 100.0 }),
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
            history_midi: VecDeque::new(),
            last_detected_key: String::from("Unknown"),
            debounce_samples_remaining: 0,
            notes_changed: false,
            reset_history: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl Plugin for ChordLens {
    const NAME: &'static str = "ChordLens";
    const VENDOR: &'static str = "Simon Heimler";
    const URL: &'static str = "https://github.com/Fannon/ChordLens";
    const EMAIL: &'static str = "simon@heimler.de";

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
            self.params.clone(),
            self.chord_state.clone(),
            self.reset_history.clone(),
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
                NoteEvent::NoteOn { note, velocity, .. } => {
                    if velocity > 0.0 {
                        self.active_notes.insert(note);
                        changed = true;
                        
                        // Push to history (32 notes)
                        self.history_midi.push_back(note);
                        if self.history_midi.len() > 32 {
                            self.history_midi.pop_front();
                        }
                    } else {
                        self.active_notes.remove(&note);
                        changed = true;
                    }
                    context.send_event(event);
                }

                // ── Note OFF (and zero-velocity NoteOn which is a NoteOff) ──
                NoteEvent::NoteOff { note, .. } => {
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

        // ── Debounce Logic ───────────────────────────────────────────────────
        let sample_rate = context.transport().sample_rate;
        let debounce_threshold = (self.params.debounce_ms.value() / 1000.0 * sample_rate) as u32;

        if changed {
            self.notes_changed = true;
            // Reset/Start debounce timer on every note change
            self.debounce_samples_remaining = debounce_threshold;
        }

        let mut run_detection = false;
        if self.notes_changed {
            if self.debounce_samples_remaining <= _buffer.samples() as u32 {
                self.debounce_samples_remaining = 0;
                self.notes_changed = false;
                run_detection = true;
            } else {
                self.debounce_samples_remaining -= _buffer.samples() as u32;
            }
        }

        // Update shared chord state only when detection is triggered or history reset.
        if self.reset_history.swap(false, Ordering::Relaxed) {
            self.history_midi.clear();
            self.last_detected_key = String::from("Unknown");
            run_detection = true;
        }

        if run_detection {
            let notes: Vec<u8> = self.active_notes.iter().copied().collect();

            let root_param = self.params.key_root.value();
            let mode_param = self.params.key_mode.value();
            
            let (_, scale_root, scale_intervals) = if root_param == KeyRoot::Auto {
                chord::detect_scale(&self.history_midi, notes.iter().copied().min(), &self.last_detected_key)
            } else {
                (
                    String::new(),
                    root_param.pc_val(),
                    mode_param.intervals(),
                )
            };

            // Detect chord with scale context for enharmonics and Roman numeral
            let chord_info = detect(&notes, scale_root, self.params.allow_rootless.value());
            
            let key_text = if root_param == KeyRoot::Auto {
                let (auto_key, _, _) = chord::detect_scale(&self.history_midi, notes.iter().copied().min(), &self.last_detected_key);
                self.last_detected_key = auto_key.clone();
                format!("Detected: {}", auto_key)
            } else {
                format!("User: {} {}", root_param.as_str(), mode_param.as_str())
            };

            // Shared degree/nash text
            let nashville_text = chord_info.degree.clone();

            // Write to the shared state.
            *self.chord_state.write() = ChordState {
                chord_info,
                key_text,
                scale_root,
                scale_intervals,
                nashville_text,
            };
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
