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
//! *reads* once per frame (~60 Hz). `parking_lot::RwLock` does not allocate on
//! the uncontended fast path, but any state formatting still done inside
//! `process()` must be kept minimal because this is a MIDI effect running on a
//! host callback.
//!
//! ## Chord detection
//!
//! See `chord.rs` for the interval-math details.  The short story:
//!  - Active MIDI note counts are stored in a fixed `[ActiveNoteState; 128]`.
//!  - On every note event, `chord::detect()` rebuilds the `ChordInfo`.
//!  - The new `ChordInfo` is written to `ChordState` under the write-lock.

mod chord;
mod editor;
mod key_detection;
#[cfg(test)]
mod tests;

use chord::{detect, ChordInfo};
use editor::{EDITOR_HEIGHT, EDITOR_WIDTH};
use key_detection::{detect_key, DetectedKey, KeyEstimate, ScaleMode};

use nih_plug::prelude::*;
use nih_plug_egui::EguiState;
use parking_lot::RwLock;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

const KEY_HISTORY_LIMIT: usize = 24;
const CHORD_HISTORY_LIMIT: usize = 16;
const HELD_NOTE_HISTORY_STEP_SECS: f32 = 0.080;
const KEY_EVIDENCE_HALFLIFE_SECS: f32 = 1.25;
const NOTE_ON_EVIDENCE_WEIGHT: f32 = 8.0;
const HELD_NOTE_EVIDENCE_WEIGHT: f32 = 3.0;
const AUTO_KEY_REFRESH_SECS: f32 = 0.040;

#[derive(Debug, PartialEq, Eq, Enum, Clone, Copy)]
pub enum KeyRoot {
    #[name = "Auto"]
    Auto,
    #[name = "Chromatic"]
    Chromatic,
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
            KeyRoot::Chromatic => "Chromatic",
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
            KeyRoot::Chromatic => 0,
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
    pub fn intervals(&self) -> &'static [i32] {
        match self {
            KeyMode::Major => &[0, 2, 4, 5, 7, 9, 11],
            KeyMode::Minor => &[0, 2, 3, 5, 7, 8, 10],
            KeyMode::Dorian => &[0, 2, 3, 5, 7, 9, 10],
            KeyMode::Phrygian => &[0, 1, 3, 5, 7, 8, 10],
            KeyMode::Lydian => &[0, 2, 4, 6, 7, 9, 11],
            KeyMode::Mixolydian => &[0, 2, 4, 5, 7, 9, 10],
            KeyMode::Locrian => &[0, 1, 3, 5, 6, 8, 10],
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

impl From<KeyMode> for ScaleMode {
    fn from(value: KeyMode) -> Self {
        match value {
            KeyMode::Major => ScaleMode::Major,
            KeyMode::Minor => ScaleMode::Minor,
            KeyMode::Dorian => ScaleMode::Dorian,
            KeyMode::Phrygian => ScaleMode::Phrygian,
            KeyMode::Lydian => ScaleMode::Lydian,
            KeyMode::Mixolydian => ScaleMode::Mixolydian,
            KeyMode::Locrian => ScaleMode::Locrian,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Enum, Clone, Copy)]
pub enum KeyResponsiveness {
    #[name = "Stable"]
    Stable,
    #[name = "Balanced"]
    Balanced,
    #[name = "Reactive"]
    Reactive,
}

impl KeyResponsiveness {
    pub fn display_switch_secs(&self) -> f32 {
        match self {
            KeyResponsiveness::Stable => 0.280,
            KeyResponsiveness::Balanced => 0.180,
            KeyResponsiveness::Reactive => 0.100,
        }
    }

    pub fn key_switch_margin(&self, miss_weight: i32) -> i32 {
        let base = if miss_weight <= 6 {
            64
        } else if miss_weight <= 14 {
            36
        } else {
            18
        };
        match self {
            KeyResponsiveness::Stable => base + 20,
            KeyResponsiveness::Balanced => base,
            KeyResponsiveness::Reactive => (base - 10).max(8),
        }
    }

    pub fn mode_switch_margin(&self) -> i32 {
        match self {
            KeyResponsiveness::Stable => 34,
            KeyResponsiveness::Balanced => 24,
            KeyResponsiveness::Reactive => 14,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChordHistoryEntry {
    pub root_pc: Option<u8>,
    pub quality: &'static str,
    pub omitted: &'static str,
    pub bass_pc: Option<u8>,
}

impl ChordHistoryEntry {
    pub fn slash_label(&self, scale_root: u8) -> String {
        match (self.root_pc, self.bass_pc) {
            (Some(root_pc), Some(bass_pc)) if bass_pc != root_pc => {
                format!("/{}", chord::pc_name(bass_pc, scale_root))
            }
            _ => String::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScaleIntervals {
    values: [i32; 7],
    len: u8,
}

impl ScaleIntervals {
    fn from_slice(intervals: &[i32]) -> Self {
        let mut values = [0; 7];
        let len = intervals.len().min(values.len());
        values[..len].copy_from_slice(&intervals[..len]);
        Self {
            values,
            len: len as u8,
        }
    }

    pub fn as_slice(&self) -> &[i32] {
        &self.values[..self.len as usize]
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChordHistorySnapshot {
    entries: [ChordHistoryEntry; CHORD_HISTORY_LIMIT],
    len: u8,
}

impl ChordHistorySnapshot {
    fn push(&mut self, entry: ChordHistoryEntry) {
        let idx = self.len as usize;
        self.entries[idx] = entry;
        self.len += 1;
    }

    pub fn iter(&self) -> impl Iterator<Item = &ChordHistoryEntry> {
        self.entries[..self.len as usize].iter()
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn len(&self) -> usize {
        self.len as usize
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct StableChordIdentity {
    root_pc: Option<u8>,
    quality: &'static str,
    omitted: &'static str,
    bass_pc: Option<u8>,
}

impl From<ChordInfo> for StableChordIdentity {
    fn from(value: ChordInfo) -> Self {
        Self {
            root_pc: value.root_pc,
            quality: value.quality,
            omitted: value.omitted,
            bass_pc: value.bass_pc,
        }
    }
}

#[derive(Clone, Default)]
pub struct ChordState {
    pub chord_info: ChordInfo,
    pub key_text: String,
    pub scale_root: u8,
    pub scale_intervals: ScaleIntervals,
    pub chromatic_mode: bool,
    pub key_confidence: u8,
    pub nashville_text: String,
    pub chord_history: ChordHistorySnapshot,
    #[cfg(debug_assertions)]
    pub debug_key_diagnostics: String,
}

#[derive(Clone, Default)]
pub(crate) struct KeyDisplayState {
    key: Option<DetectedKey>,
    confidence: u8,
}

#[derive(Clone)]
pub(crate) struct KeyCandidate {
    estimate: KeyEstimate,
}

#[derive(Clone, Copy, Default)]
pub(crate) struct ActiveNoteState {
    instances: u16,
    held_samples: u32,
}

pub(crate) struct KeyHistory {
    active_notes: [ActiveNoteState; 128],
    recent_notes: VecDeque<u8>,
    note_evidence: [f32; 12],
    chords: VecDeque<ChordHistoryEntry>,
}

impl Default for KeyHistory {
    fn default() -> Self {
        Self {
            active_notes: [ActiveNoteState::default(); 128],
            recent_notes: VecDeque::with_capacity(KEY_HISTORY_LIMIT),
            note_evidence: [0.0; 12],
            chords: VecDeque::with_capacity(CHORD_HISTORY_LIMIT),
        }
    }
}

pub struct ChordLens {
    params: Arc<ChordLensParams>,
    chord_state: Arc<RwLock<ChordState>>,
    key_history: KeyHistory,
    internal_detected_key: Option<DetectedKey>,
    displayed_key_state: KeyDisplayState,
    pending_display_key_state: KeyDisplayState,
    pending_display_samples: u32,
    debounce_samples_remaining: u32,
    key_refresh_samples_remaining: u32,
    notes_changed: bool,
    last_pushed_chord: StableChordIdentity,
    current_stable_chord: StableChordIdentity,
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
    #[id = "key_resp"]
    pub key_responsiveness: EnumParam<KeyResponsiveness>,
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
            key_responsiveness: EnumParam::new("Key Tracking", KeyResponsiveness::Balanced),
            show_nashville: BoolParam::new("Roman Numerals", true),
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
            key_history: KeyHistory::default(),
            internal_detected_key: None,
            displayed_key_state: KeyDisplayState::default(),
            pending_display_key_state: KeyDisplayState::default(),
            pending_display_samples: 0,
            debounce_samples_remaining: 0,
            key_refresh_samples_remaining: 0,
            notes_changed: false,
            last_pushed_chord: StableChordIdentity::default(),
            current_stable_chord: StableChordIdentity::default(),
            stable_samples: 0,
            reset_history: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl KeyHistory {
    fn push_note(history: &mut VecDeque<u8>, note: u8) {
        history.push_back(note);
        if history.len() > KEY_HISTORY_LIMIT {
            history.pop_front();
        }
    }

    fn note_on(&mut self, note: u8) {
        let state = &mut self.active_notes[note as usize];
        state.instances = state.instances.saturating_add(1);
        if state.instances == 1 {
            state.held_samples = 0;
        }
        Self::push_note(&mut self.recent_notes, note);
        self.note_evidence[(note % 12) as usize] += NOTE_ON_EVIDENCE_WEIGHT;
    }

    fn note_off(&mut self, note: u8) {
        let state = &mut self.active_notes[note as usize];
        if state.instances > 1 {
            state.instances -= 1;
        } else {
            *state = ActiveNoteState::default();
        }
    }

    fn fill_active_note_list(&self, notes: &mut [u8; 128]) -> usize {
        let mut len = 0;
        for (note, state) in self.active_notes.iter().enumerate() {
            if state.instances > 0 {
                notes[len] = note as u8;
                len += 1;
            }
        }
        len
    }

    fn lowest_note(&self) -> Option<u8> {
        self.active_notes
            .iter()
            .position(|state| state.instances > 0)
            .map(|note| note as u8)
    }

    fn has_active_notes(&self) -> bool {
        self.active_notes.iter().any(|state| state.instances > 0)
    }

    fn has_evidence(&self) -> bool {
        self.note_evidence.iter().any(|&value| value > 0.0)
    }

    fn note_history(&self) -> &VecDeque<u8> {
        &self.recent_notes
    }

    fn note_evidence(&self) -> &[f32; 12] {
        &self.note_evidence
    }

    fn chord_history(&self) -> &VecDeque<ChordHistoryEntry> {
        &self.chords
    }

    fn chord_history_snapshot(&self) -> ChordHistorySnapshot {
        let mut snapshot = ChordHistorySnapshot::default();
        for &entry in &self.chords {
            snapshot.push(entry);
        }
        snapshot
    }

    fn push_chord(&mut self, entry: ChordHistoryEntry) {
        self.chords.push_back(entry);
        if self.chords.len() > CHORD_HISTORY_LIMIT {
            self.chords.pop_front();
        }
    }

    fn clear(&mut self) {
        self.active_notes = [ActiveNoteState::default(); 128];
        self.recent_notes.clear();
        self.note_evidence = [0.0; 12];
        self.chords.clear();
    }

    fn decay_evidence(&mut self, buffer_samples: u32, sample_rate: f32) {
        let halflife_samples = KEY_EVIDENCE_HALFLIFE_SECS * sample_rate;
        if halflife_samples <= 0.0 {
            return;
        }
        let decay = 0.5_f32.powf(buffer_samples as f32 / halflife_samples);
        for value in &mut self.note_evidence {
            *value *= decay;
            if *value < 0.001 {
                *value = 0.0;
            }
        }
    }
}

pub(crate) fn accumulate_held_note_history(
    key_history: &mut KeyHistory,
    buffer_samples: u32,
    sample_rate: f32,
) {
    key_history.decay_evidence(buffer_samples, sample_rate);
    let sustain_step_samples = (HELD_NOTE_HISTORY_STEP_SECS * sample_rate) as u32;
    if sustain_step_samples == 0 {
        return;
    }

    for (note, state) in key_history.active_notes.iter_mut().enumerate() {
        if state.instances == 0 {
            continue;
        }
        let previous_steps = state.held_samples / sustain_step_samples;
        state.held_samples = state.held_samples.saturating_add(buffer_samples);
        let current_steps = state.held_samples / sustain_step_samples;
        let new_steps = current_steps.saturating_sub(previous_steps).min(2);

        for _ in 0..new_steps {
            KeyHistory::push_note(&mut key_history.recent_notes, note as u8);
            key_history.note_evidence[note % 12] += HELD_NOTE_EVIDENCE_WEIGHT;
        }
    }
}

pub(crate) fn update_displayed_key(
    displayed: &mut KeyDisplayState,
    pending: &mut KeyDisplayState,
    pending_samples: &mut u32,
    internal: KeyCandidate,
    buffer_samples: u32,
    sample_rate: f32,
    switch_secs: f32,
) {
    let Some(internal_key) = internal.estimate.key else {
        *displayed = KeyDisplayState::default();
        *pending = KeyDisplayState::default();
        *pending_samples = 0;
        return;
    };

    if displayed.key.is_none() {
        displayed.key = Some(internal_key);
        displayed.confidence = internal.estimate.confidence;
        *pending = KeyDisplayState::default();
        *pending_samples = 0;
        return;
    }

    if displayed.key == Some(internal_key) {
        displayed.confidence = internal.estimate.confidence;
        *pending = KeyDisplayState::default();
        *pending_samples = 0;
        return;
    }

    if pending.key != Some(internal_key) {
        pending.key = Some(internal_key);
        pending.confidence = internal.estimate.confidence;
        *pending_samples = buffer_samples;
    } else {
        pending.confidence = internal.estimate.confidence;
        *pending_samples = pending_samples.saturating_add(buffer_samples);
    }

    let switch_threshold = (switch_secs * sample_rate) as u32;
    if *pending_samples >= switch_threshold {
        *displayed = pending.clone();
        *pending = KeyDisplayState::default();
        *pending_samples = 0;
    }
}

fn resolve_display_state(
    plugin: &mut ChordLens,
    root_param: KeyRoot,
    mode_param: KeyMode,
    responsiveness: KeyResponsiveness,
    key_estimate: &KeyEstimate,
    buffer_samples: u32,
    sample_rate: f32,
) -> (u8, ScaleIntervals, String, u8, bool) {
    if root_param == KeyRoot::Auto {
        plugin.internal_detected_key = key_estimate.key;
        update_displayed_key(
            &mut plugin.displayed_key_state,
            &mut plugin.pending_display_key_state,
            &mut plugin.pending_display_samples,
            KeyCandidate {
                estimate: key_estimate.clone(),
            },
            buffer_samples,
            sample_rate,
            responsiveness.display_switch_secs(),
        );
        let displayed_key = plugin.displayed_key_state.key;
        (
            displayed_key.map_or(0, |key| key.root),
            ScaleIntervals::from_slice(
                displayed_key.map(|key| key.intervals()).unwrap_or_default(),
            ),
            displayed_key
                .map(|key| format!("Detected: {}", key.display_name()))
                .unwrap_or_else(|| "Detected: Unknown".to_string()),
            plugin.displayed_key_state.confidence,
            false,
        )
    } else if root_param == KeyRoot::Chromatic {
        (
            0,
            ScaleIntervals::default(),
            "User: Chromatic".to_string(),
            key_estimate.confidence,
            true,
        )
    } else {
        let forced_key = key_estimate.key.expect("forced key must exist");
        (
            forced_key.root,
            ScaleIntervals::from_slice(forced_key.intervals()),
            format!("User: {} {}", root_param.as_str(), mode_param.as_str()),
            key_estimate.confidence,
            false,
        )
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
                        self.key_history.note_on(note);
                        changed = true;
                    } else {
                        self.key_history.note_off(note);
                        changed = true;
                    }
                    context.send_event(event);
                }
                NoteEvent::NoteOff { note, .. } | NoteEvent::Choke { note, .. } => {
                    self.key_history.note_off(note);
                    changed = true;
                    context.send_event(event);
                }
                other => {
                    context.send_event(other);
                }
            }
        }

        let sample_rate = context.transport().sample_rate;
        let buffer_samples = _buffer.samples() as u32;
        accumulate_held_note_history(&mut self.key_history, buffer_samples, sample_rate);
        let debounce_threshold = (0.025 * sample_rate) as u32; // Fixed 25ms internal debounce

        if changed {
            self.notes_changed = true;
            self.debounce_samples_remaining = debounce_threshold;
        }

        let mut run_detection = false;
        let mut key_only_refresh = false;
        if self.notes_changed {
            if self.debounce_samples_remaining <= buffer_samples {
                self.debounce_samples_remaining = 0;
                self.notes_changed = false;
                run_detection = true;
            } else {
                self.debounce_samples_remaining -= buffer_samples;
            }
        }

        if self.reset_history.swap(false, Ordering::Relaxed) {
            self.key_history.clear();
            self.last_pushed_chord = StableChordIdentity::default();
            self.current_stable_chord = StableChordIdentity::default();
            self.internal_detected_key = None;
            self.displayed_key_state = KeyDisplayState::default();
            self.pending_display_key_state = KeyDisplayState::default();
            self.pending_display_samples = 0;
            self.key_refresh_samples_remaining = 0;
            run_detection = true;
        }

        let root_param = self.params.key_root.value();
        let mode_param = self.params.key_mode.value();
        let responsiveness = self.params.key_responsiveness.value();
        let refresh_threshold = (AUTO_KEY_REFRESH_SECS * sample_rate) as u32;

        if !run_detection
            && root_param == KeyRoot::Auto
            && (self.key_history.has_active_notes()
                || self.key_history.has_evidence()
                || self.pending_display_key_state.key.is_some())
        {
            if self.key_refresh_samples_remaining <= buffer_samples {
                self.key_refresh_samples_remaining = refresh_threshold;
                key_only_refresh = true;
            } else {
                self.key_refresh_samples_remaining -= buffer_samples;
            }
        } else if run_detection {
            self.key_refresh_samples_remaining = refresh_threshold;
        } else {
            self.key_refresh_samples_remaining = 0;
        }

        if run_detection || key_only_refresh {
            let key_estimate = if root_param == KeyRoot::Auto {
                detect_key(
                    self.key_history.note_evidence(),
                    self.key_history.note_history(),
                    self.key_history.lowest_note(),
                    self.internal_detected_key,
                    self.key_history.chord_history(),
                    responsiveness,
                )
            } else if root_param == KeyRoot::Chromatic {
                KeyEstimate {
                    key: None,
                    confidence: 100,
                    #[cfg(debug_assertions)]
                    diagnostics: String::new(),
                }
            } else {
                KeyEstimate {
                    key: Some(DetectedKey {
                        root: root_param.pc_val(),
                        mode: mode_param.into(),
                    }),
                    confidence: 100,
                    #[cfg(debug_assertions)]
                    diagnostics: String::new(),
                }
            };

            let (
                display_scale_root,
                display_scale_intervals,
                key_text,
                key_confidence,
                chromatic_mode,
            ) = resolve_display_state(
                self,
                root_param,
                mode_param,
                responsiveness,
                &key_estimate,
                buffer_samples,
                sample_rate,
            );

            let chord_info = if run_detection {
                let mut notes = [0u8; 128];
                let note_count = self.key_history.fill_active_note_list(&mut notes);
                detect(
                    &notes[..note_count],
                    display_scale_root,
                    self.params.allow_rootless.value(),
                )
            } else {
                self.chord_state.read().chord_info
            };

            let nashville_text = if chromatic_mode {
                String::new()
            } else {
                chord_info.degree_text(display_scale_root)
            };
            let current_chord = StableChordIdentity::from(chord_info);

            if current_chord != self.current_stable_chord {
                self.current_stable_chord = current_chord;
                self.stable_samples = 0;
            } else {
                let threshold = (0.12 * sample_rate) as u32; // 120ms stability threshold
                self.stable_samples += buffer_samples;
                if self.stable_samples >= threshold
                    && self.current_stable_chord != self.last_pushed_chord
                    && self.current_stable_chord.root_pc.is_some()
                {
                    self.key_history.push_chord(ChordHistoryEntry {
                        root_pc: chord_info.root_pc,
                        quality: chord_info.quality,
                        omitted: chord_info.omitted,
                        bass_pc: chord_info.bass_pc,
                    });
                    self.last_pushed_chord = self.current_stable_chord;
                }
            }

            *self.chord_state.write() = ChordState {
                chord_info,
                key_text,
                scale_root: display_scale_root,
                scale_intervals: display_scale_intervals,
                chromatic_mode,
                key_confidence,
                nashville_text,
                chord_history: self.key_history.chord_history_snapshot(),
                #[cfg(debug_assertions)]
                debug_key_diagnostics: if cfg!(debug_assertions)
                    && std::env::var_os("CHORDLENS_DEBUG_KEYS").is_some()
                {
                    key_estimate.diagnostics.clone()
                } else {
                    String::new()
                },
            };
        } else {
            // Even if we didn't recompute chord detection, update stability for history tracking.
            let cur_info = self.chord_state.read().chord_info;
            let current_chord = StableChordIdentity::from(cur_info);
            if current_chord != self.current_stable_chord {
                self.current_stable_chord = current_chord;
                self.stable_samples = 0;
            } else {
                let threshold = (0.12 * sample_rate) as u32; // 120ms
                self.stable_samples += buffer_samples;
                if self.stable_samples >= threshold
                    && self.current_stable_chord != self.last_pushed_chord
                    && self.current_stable_chord.root_pc.is_some()
                {
                    self.key_history.push_chord(ChordHistoryEntry {
                        root_pc: cur_info.root_pc,
                        quality: cur_info.quality,
                        omitted: cur_info.omitted,
                        bass_pc: cur_info.bass_pc,
                    });
                    self.last_pushed_chord = self.current_stable_chord;
                    let mut state = self.chord_state.write();
                    state.chord_history = self.key_history.chord_history_snapshot();
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
