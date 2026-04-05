//! # ChordLens — Real-time MIDI Chord Detector  (VST3 + CLAP)
//!
//! ## Architecture
//!
//! ```text
//!  ┌──────────────────────────────────────────────┐
//!  │              Host DAW / plugin runner         │
//!                                                  │
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

mod chord;
mod editor;
#[cfg(test)]
mod tests;

use chord::{detect, ChordInfo};
use editor::{EDITOR_HEIGHT, EDITOR_WIDTH};

use nih_plug::prelude::*;
use nih_plug_egui::EguiState;
use parking_lot::RwLock;
use std::collections::{HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Debug, PartialEq, Eq, Enum, Clone, Copy)]
pub enum KeyRoot {
    #[name = "Auto"]
    Auto,
    #[name = "C"]
    C,
    #[name = "C#"]
    CSharp,
    #[name = "D"]
    D,
    #[name = "D#"]
    DSharp,
    #[name = "E"]
    E,
    #[name = "F"]
    F,
    #[name = "F#"]
    FSharp,
    #[name = "G"]
    G,
    #[name = "G#"]
    GSharp,
    #[name = "A"]
    A,
    #[name = "A#"]
    ASharp,
    #[name = "B"]
    B,
}

impl KeyRoot {
    pub fn as_str(&self) -> &'static str {
        match self {
            KeyRoot::Auto => "Auto",
            KeyRoot::C => "C",
            KeyRoot::CSharp => "C#",
            KeyRoot::D => "D",
            KeyRoot::DSharp => "D#",
            KeyRoot::E => "E",
            KeyRoot::F => "F",
            KeyRoot::FSharp => "F#",
            KeyRoot::G => "G",
            KeyRoot::GSharp => "G#",
            KeyRoot::A => "A",
            KeyRoot::ASharp => "A#",
            KeyRoot::B => "B",
        }
    }
    pub fn pc_val(&self) -> u8 {
        match self {
            KeyRoot::Auto => 0,
            KeyRoot::C => 0,
            KeyRoot::CSharp => 1,
            KeyRoot::D => 2,
            KeyRoot::DSharp => 3,
            KeyRoot::E => 4,
            KeyRoot::F => 5,
            KeyRoot::FSharp => 6,
            KeyRoot::G => 7,
            KeyRoot::GSharp => 8,
            KeyRoot::A => 9,
            KeyRoot::ASharp => 10,
            KeyRoot::B => 11,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Enum, Clone, Copy)]
pub enum KeyMode {
    #[name = "Major"]
    Major,
    #[name = "Minor"]
    Minor,
    #[name = "Dorian"]
    Dorian,
    #[name = "Phrygian"]
    Phrygian,
    #[name = "Lydian"]
    Lydian,
    #[name = "Mixolydian"]
    Mixolydian,
    #[name = "Locrian"]
    Locrian,
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
            KeyMode::Major => "Major",
            KeyMode::Minor => "Minor",
            KeyMode::Dorian => "Dorian",
            KeyMode::Phrygian => "Phrygian",
            KeyMode::Lydian => "Lydian",
            KeyMode::Mixolydian => "Mixolydian",
            KeyMode::Locrian => "Locrian",
        }
    }
}

#[derive(Clone, Default)]
pub struct ChordHistoryEntry {
    pub root: String,
    pub quality: String,
    pub omitted: String,
    pub slash: String,
}

#[derive(Clone, Default)]
pub struct ChordState {
    pub chord_info: ChordInfo,
    pub key_text: String,
    pub scale_root: u8,
    pub scale_intervals: Vec<i32>,
    pub nashville_text: String,
    pub chord_history: Vec<ChordHistoryEntry>,
}

pub struct ChordLens {
    params: Arc<ChordLensParams>,
    chord_state: Arc<RwLock<ChordState>>,
    active_notes: HashSet<u8>,
    history_midi: VecDeque<u8>,
    last_detected_key: String,
    debounce_samples_remaining: u32,
    notes_changed: bool,
    history_chords: VecDeque<ChordHistoryEntry>,
    last_pushed_chord: String,
    current_stable_chord: String,
    stable_samples: u32,
    pub reset_history: Arc<AtomicBool>,
}

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
    #[id = "show_history"]
    pub show_history: BoolParam,
}

impl Default for ChordLensParams {
    fn default() -> Self {
        Self {
            editor_state: EguiState::from_size(EDITOR_WIDTH, EDITOR_HEIGHT),
            key_root: EnumParam::new("Force Root", KeyRoot::Auto),
            key_mode: EnumParam::new("Force Mode", KeyMode::Major),
            show_nashville: BoolParam::new("Nashville Numbers", true),
            allow_rootless: BoolParam::new("Root-less Voicings", false),
            show_history: BoolParam::new("Chord History", false),
        }
    }
}

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
            history_chords: VecDeque::new(),
            last_pushed_chord: String::new(),
            current_stable_chord: String::new(),
            stable_samples: 0,
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
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[];
    const MIDI_INPUT: MidiConfig = MidiConfig::MidiCCs;
    const MIDI_OUTPUT: MidiConfig = MidiConfig::MidiCCs;
    const SAMPLE_ACCURATE_AUTOMATION: bool = false;
    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }
    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        editor::create(
            self.params.clone(),
            self.chord_state.clone(),
            self.reset_history.clone(),
        )
    }

    fn process(
        &mut self,
        _buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let mut changed = false;
        while let Some(event) = context.next_event() {
            match event {
                NoteEvent::NoteOn { note, velocity, .. } => {
                    if velocity > 0.0 {
                        self.active_notes.insert(note);
                        changed = true;
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
                NoteEvent::NoteOff { note, .. } | NoteEvent::Choke { note, .. } => {
                    self.active_notes.remove(&note);
                    changed = true;
                    context.send_event(event);
                }
                other => {
                    context.send_event(other);
                }
            }
        }

        let sample_rate = context.transport().sample_rate;
        let debounce_threshold = (0.025 * sample_rate) as u32; // Fixed 25ms internal debounce

        if changed {
            self.notes_changed = true;
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

        if self.reset_history.swap(false, Ordering::Relaxed) {
            self.history_midi.clear();
            self.history_chords.clear();
            self.last_pushed_chord.clear();
            self.current_stable_chord.clear();
            self.last_detected_key = String::from("Unknown");
            run_detection = true;
        }

        if run_detection {
            let notes: Vec<u8> = self.active_notes.iter().copied().collect();
            let root_param = self.params.key_root.value();
            let mode_param = self.params.key_mode.value();

            let (_, scale_root, scale_intervals) = if root_param == KeyRoot::Auto {
                chord::detect_scale(
                    &self.history_midi,
                    notes.iter().copied().min(),
                    &self.last_detected_key,
                )
            } else {
                (String::new(), root_param.pc_val(), mode_param.intervals())
            };

            let chord_info = detect(&notes, scale_root, self.params.allow_rootless.value());
            let key_text = if root_param == KeyRoot::Auto {
                let (auto_key, _, _) = chord::detect_scale(
                    &self.history_midi,
                    notes.iter().copied().min(),
                    &self.last_detected_key,
                );
                self.last_detected_key = auto_key.clone();
                format!("Detected: {}", auto_key)
            } else {
                format!("User: {} {}", root_param.as_str(), mode_param.as_str())
            };

            let nashville_text = chord_info.degree.clone();
            let current_full_name = format!("{}", chord_info);

            if current_full_name != self.current_stable_chord {
                self.current_stable_chord = current_full_name;
                self.stable_samples = 0;
            } else {
                let threshold = (0.12 * sample_rate) as u32; // 120ms stability threshold
                self.stable_samples += _buffer.samples() as u32;
                if self.stable_samples >= threshold
                    && self.current_stable_chord != self.last_pushed_chord
                    && !self.current_stable_chord.is_empty()
                    && self.current_stable_chord != "–"
                {
                    self.history_chords.push_back(ChordHistoryEntry {
                        root: chord_info.root.clone(),
                        quality: chord_info.quality.clone(),
                        omitted: chord_info.omitted.clone(),
                        slash: chord_info.slash.clone(),
                    });
                    if self.history_chords.len() > 16 {
                        self.history_chords.pop_front();
                    }
                    self.last_pushed_chord = self.current_stable_chord.clone();
                }
            }

            *self.chord_state.write() = ChordState {
                chord_info,
                key_text,
                scale_root,
                scale_intervals,
                nashville_text,
                chord_history: self.history_chords.iter().cloned().collect(),
            };
        } else {
            // Even if we didn't run full detection, update stability for history tracking
            let cur_info = self.chord_state.read().chord_info.clone();
            let name = format!("{}", cur_info);
            if name != self.current_stable_chord {
                self.current_stable_chord = name;
                self.stable_samples = 0;
            } else {
                let threshold = (0.12 * sample_rate) as u32; // 120ms
                self.stable_samples += _buffer.samples() as u32;
                if self.stable_samples >= threshold
                    && self.current_stable_chord != self.last_pushed_chord
                    && !self.current_stable_chord.is_empty()
                    && self.current_stable_chord != "–"
                {
                    self.history_chords.push_back(ChordHistoryEntry {
                        root: cur_info.root.clone(),
                        quality: cur_info.quality.clone(),
                        omitted: cur_info.omitted.clone(),
                        slash: cur_info.slash.clone(),
                    });
                    if self.history_chords.len() > 16 {
                        self.history_chords.pop_front();
                    }
                    self.last_pushed_chord = self.current_stable_chord.clone();
                    let mut state = self.chord_state.write();
                    state.chord_history = self.history_chords.iter().cloned().collect();
                }
            }
        }

        ProcessStatus::Normal
    }
}

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
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[
        Vst3SubCategory::Fx,
        Vst3SubCategory::Tools,
        Vst3SubCategory::Analyzer,
    ];
}

nih_export_clap!(ChordLens);
nih_export_vst3!(ChordLens);
