use crate::accumulate_held_note_history;
use crate::detect;
use crate::key_detection::{detect_key, DetectedKey, ScaleMode};
use crate::update_displayed_key;
use crate::ChordHistoryEntry;
use crate::ChordLens;
use crate::KeyCandidate;
use crate::KeyDisplayState;
use crate::KeyHistory;
use crate::KeyResponsiveness;
use nih_plug::context::process::{ProcessContext, Transport};
use nih_plug::context::PluginApi;
use nih_plug::prelude::{AuxiliaryBuffers, Buffer, NoteEvent, Plugin, ProcessStatus};
use std::collections::VecDeque;
use std::sync::atomic::Ordering;

const TEST_SAMPLE_RATE: f32 = 48_000.0;
const TEST_BUFFER_SAMPLES: usize = 512;

struct TestProcessContext {
    transport: Transport,
    input_events: VecDeque<NoteEvent<()>>,
    output_events: Vec<NoteEvent<()>>,
}

impl TestProcessContext {
    fn new(events: Vec<NoteEvent<()>>) -> Self {
        Self {
            transport: test_transport(TEST_SAMPLE_RATE),
            input_events: events.into(),
            output_events: Vec::new(),
        }
    }
}

impl ProcessContext<ChordLens> for TestProcessContext {
    fn plugin_api(&self) -> PluginApi {
        PluginApi::Standalone
    }

    fn execute_background(&self, _task: <ChordLens as Plugin>::BackgroundTask) {}

    fn execute_gui(&self, _task: <ChordLens as Plugin>::BackgroundTask) {}

    fn transport(&self) -> &Transport {
        &self.transport
    }

    fn next_event(&mut self) -> Option<NoteEvent<()>> {
        self.input_events.pop_front()
    }

    fn send_event(&mut self, event: NoteEvent<()>) {
        self.output_events.push(event);
    }

    fn set_latency_samples(&self, _samples: u32) {}

    fn set_current_voice_capacity(&self, _capacity: u32) {}
}

fn test_transport(sample_rate: f32) -> Transport {
    let mut transport = unsafe { std::mem::MaybeUninit::<Transport>::zeroed().assume_init() };
    transport.sample_rate = sample_rate;
    transport
}

fn empty_test_buffer() -> Buffer<'static> {
    let mut buffer = Buffer::default();
    unsafe {
        buffer.set_slices(TEST_BUFFER_SAMPLES, |output_slices| output_slices.clear());
    }
    buffer
}

fn run_process(plugin: &mut ChordLens, events: Vec<NoteEvent<()>>) -> Vec<NoteEvent<()>> {
    let mut buffer = empty_test_buffer();
    let mut aux_inputs = [];
    let mut aux_outputs = [];
    let mut aux = AuxiliaryBuffers {
        inputs: &mut aux_inputs,
        outputs: &mut aux_outputs,
    };
    let mut context = TestProcessContext::new(events);
    let status = plugin.process(&mut buffer, &mut aux, &mut context);
    assert_eq!(status, ProcessStatus::Normal);
    context.output_events
}

fn note_on(note: u8) -> NoteEvent<()> {
    NoteEvent::NoteOn {
        timing: 0,
        voice_id: None,
        channel: 0,
        note,
        velocity: 1.0,
    }
}

fn note_off(note: u8) -> NoteEvent<()> {
    NoteEvent::NoteOff {
        timing: 0,
        voice_id: None,
        channel: 0,
        note,
        velocity: 0.0,
    }
}

fn process_until_settled(
    plugin: &mut ChordLens,
    leading_events: Vec<NoteEvent<()>>,
    extra_buffers: usize,
) {
    run_process(plugin, leading_events);
    for _ in 0..extra_buffers {
        run_process(plugin, Vec::new());
    }
}

fn chord_str(notes: &[u8], scale_root: u8) -> String {
    let info = detect(notes, scale_root, false);
    format!(
        "{}{}{}{}",
        info.root, info.quality, info.omitted, info.slash
    )
}

fn detect_key_with_responsiveness(
    history: &VecDeque<u8>,
    bass: Option<u8>,
    cur_key: Option<DetectedKey>,
    chord_history: &VecDeque<ChordHistoryEntry>,
) -> crate::key_detection::KeyEstimate {
    detect_key_with_mode(
        history,
        bass,
        cur_key,
        chord_history,
        KeyResponsiveness::Balanced,
    )
}

fn detect_key_with_mode(
    history: &VecDeque<u8>,
    bass: Option<u8>,
    cur_key: Option<DetectedKey>,
    chord_history: &VecDeque<ChordHistoryEntry>,
    responsiveness: KeyResponsiveness,
) -> crate::key_detection::KeyEstimate {
    let mut evidence = [0.0f32; 12];
    for &note in history {
        evidence[(note % 12) as usize] += 6.0;
    }
    detect_key(
        &evidence,
        history,
        bass,
        cur_key,
        chord_history,
        responsiveness,
    )
}

fn detect_scale_balanced(
    history: &VecDeque<u8>,
    bass: Option<u8>,
    cur_key: Option<DetectedKey>,
    chord_history: &VecDeque<ChordHistoryEntry>,
) -> (String, u8, Vec<i32>) {
    let estimate = detect_key_with_responsiveness(history, bass, cur_key, chord_history);
    let key = estimate.key.expect("expected detected key");
    (key.display_name(), key.root, key.intervals().to_vec())
}

fn make_chord_history(entries: &[(&str, &str)]) -> VecDeque<ChordHistoryEntry> {
    entries
        .iter()
        .map(|(root, quality)| ChordHistoryEntry {
            root: (*root).to_string(),
            quality: (*quality).to_string(),
            omitted: String::new(),
            slash: String::new(),
        })
        .collect()
}

#[test]
fn test_basic_triads() {
    assert_eq!(chord_str(&[60, 64, 67], 0), "C"); // C Maj
    assert_eq!(chord_str(&[57, 60, 64], 0), "Am"); // A Min
    assert_eq!(chord_str(&[59, 62, 65], 0), "Bdim"); // B Dim
    assert_eq!(chord_str(&[60, 65, 67], 0), "Csus4"); // C Sus4
}

#[test]
fn test_sevenths() {
    assert_eq!(chord_str(&[55, 59, 62, 65], 0), "G7"); // G7
    assert_eq!(chord_str(&[60, 64, 67, 71], 0), "Cmaj7"); // Cmaj7
    assert_eq!(chord_str(&[59, 62, 65, 68], 5), "Bdim7"); // Bdim7 in F major
}

#[test]
fn test_ninth_chords() {
    assert_eq!(chord_str(&[48, 52, 55, 58, 62], 0), "C9"); // C9
    assert_eq!(chord_str(&[48, 52, 55, 59, 62], 0), "Cmaj9"); // Cmaj9
    assert_eq!(chord_str(&[48, 51, 55, 58, 62], 0), "Cm9"); // Cm9
    assert_eq!(chord_str(&[48, 52, 55, 58, 61], 0), "C7b9"); // C7b9
}

#[test]
fn test_extended_jazz_chords() {
    // C11
    assert_eq!(chord_str(&[48, 52, 55, 58, 62, 65], 0), "C11");
    // C13
    assert_eq!(chord_str(&[48, 52, 55, 58, 62, 65, 69], 0), "C13");
    // Cmaj7#11 (requires 9th/14)
    assert_eq!(chord_str(&[60, 64, 67, 71, 74, 78], 0), "Cmaj7#11");
}

#[test]
fn test_enharmonic_naming() {
    // In C major, 58 is Bb but typically shown as A#?
    // Wait, C major doesn't have flats by default.

    // F Major key (scale_root = 5)
    // 70 (Bb) should be Bb, not A#
    let info = detect(&[70, 74, 77], 5, false); // Bb Major triad
    assert_eq!(info.root, "Bb");

    // G Major key (scale_root = 7)
    // 66 (F#) should be F#, not Gb
    let info = detect(&[66, 70, 73], 7, false); // F# Major triad
    assert_eq!(info.root, "F#");
}

#[test]
fn test_nashville_roman_numerals() {
    // I in C Major
    let info = detect(&[60, 64, 67], 0, false);
    assert_eq!(info.degree, "I");

    // ii in C Major (Dm)
    let info = detect(&[62, 65, 69], 0, false);
    assert_eq!(info.degree, "ii");

    // V7 in C Major (G7)
    let info = detect(&[55, 59, 62, 65], 0, false);
    assert_eq!(info.degree, "V"); // Our degree logic doesn't include the 7 suffix yet

    // IV in F Major (Bb)
    let info = detect(&[70, 74, 77], 5, false);
    assert_eq!(info.degree, "IV");
}

#[test]
fn test_rootless_heuristic() {
    // Playing F, A, B, E (intervals relative to G: 7, 11, 13, 21? or relative to C: 5, 9, 11, 16)
    // This is a G13 rootless voicing (F, A, B, E)
    // Notes: 65, 69, 71, 76 (F4, A4, B4, E5)
    // Without rootless: probably won't find G
    let info_no_rl = detect(&[65, 69, 71, 76], 0, false);
    assert_ne!(info_no_rl.root, "G");

    // With rootless and G is the dominant of C major (scale_root=0)
    let info_rl = detect(&[65, 69, 71, 76], 0, true);
    assert_eq!(info_rl.root, "G");
    assert_eq!(info_rl.quality, "13");
}

#[test]
fn test_inversions_and_slashes() {
    // C/G (2nd inversion if G is lowest)
    let info = detect(&[55, 60, 64], 0, false); // G3, C4, E4
    assert_eq!(info.root, "C");
    assert_eq!(info.slash, "/G");
    assert_eq!(info.inversion, "2nd inv.");

    // C/E (1st inversion)
    let info = detect(&[52, 60, 67], 0, false); // E3, C4, G4
    assert_eq!(info.root, "C");
    assert_eq!(info.slash, "/E");
    assert_eq!(info.inversion, "1st inv.");
}

#[test]
fn test_key_root_chromatic_label_and_pc() {
    assert_eq!(crate::KeyRoot::Chromatic.as_str(), "Chromatic");
    assert_eq!(crate::KeyRoot::Chromatic.pc_val(), 0);
}

#[test]
fn test_scale_detection_keeps_current_key_for_single_outlier() {
    let history = VecDeque::from(vec![60, 64, 67, 69, 71, 72, 66]);
    let (key, root, intervals) = detect_scale_balanced(
        &history,
        Some(60),
        Some(DetectedKey {
            root: 0,
            mode: ScaleMode::Major,
        }),
        &VecDeque::new(),
    );

    assert_eq!(key, "C Major");
    assert_eq!(root, 0);
    assert_eq!(intervals, vec![0, 2, 4, 5, 7, 9, 11]);
}

#[test]
fn test_scale_detection_switches_when_recent_evidence_changes_key() {
    let history = VecDeque::from(vec![60, 64, 67, 71, 67, 62, 66, 69, 73, 74, 78, 81]);
    let (key, root, intervals) = detect_scale_balanced(
        &history,
        Some(62),
        Some(DetectedKey {
            root: 0,
            mode: ScaleMode::Major,
        }),
        &VecDeque::new(),
    );

    assert_eq!(key, "D Major");
    assert_eq!(root, 2);
    assert_eq!(intervals, vec![0, 2, 4, 5, 7, 9, 11]);
}

#[test]
fn test_held_notes_add_key_history_over_time() {
    let mut key_history = KeyHistory::default();
    key_history.note_on(60);

    accumulate_held_note_history(&mut key_history, 2000, 48_000.0);
    assert_eq!(
        key_history
            .note_history()
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![60]
    );

    accumulate_held_note_history(&mut key_history, 2000, 48_000.0);
    assert_eq!(
        key_history
            .note_history()
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![60, 60]
    );

    accumulate_held_note_history(&mut key_history, 4000, 48_000.0);
    assert_eq!(
        key_history
            .note_history()
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![60, 60, 60]
    );
}

#[test]
fn test_held_notes_cap_history_bursts_per_buffer() {
    let mut key_history = KeyHistory::default();
    key_history.note_on(67);

    accumulate_held_note_history(&mut key_history, 20_000, 48_000.0);

    assert_eq!(
        key_history
            .note_history()
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![67, 67, 67]
    );
}

#[test]
fn test_continuous_decay_reduces_old_note_evidence() {
    let mut key_history = KeyHistory::default();
    key_history.note_on(60);
    let initial = key_history.note_evidence()[0];

    accumulate_held_note_history(&mut key_history, 48_000, 48_000.0);
    let decayed = key_history.note_evidence()[0];

    assert!(decayed < initial + crate::HELD_NOTE_EVIDENCE_WEIGHT);
    assert!(decayed > 0.0);
}

#[test]
fn test_key_history_clear_resets_notes_evidence_and_chords() {
    let mut key_history = KeyHistory::default();
    key_history.note_on(60);
    key_history.push_chord(ChordHistoryEntry {
        root: "C".to_string(),
        quality: String::new(),
        omitted: String::new(),
        slash: String::new(),
    });

    key_history.clear();

    assert!(key_history.note_history().is_empty());
    assert!(key_history.chord_history().is_empty());
    assert!(key_history
        .note_evidence()
        .iter()
        .all(|&value| value == 0.0));
}

#[test]
fn test_duplicate_note_off_keeps_pitch_active_until_last_release() {
    let mut key_history = KeyHistory::default();
    key_history.note_on(60);
    key_history.note_on(60);

    key_history.note_off(60);

    assert_eq!(key_history.active_note_list(), vec![60]);
}

#[test]
fn test_displayed_key_initializes_immediately() {
    let mut displayed = KeyDisplayState {
        key: None,
        ..Default::default()
    };
    let mut pending = KeyDisplayState::default();
    let mut pending_samples = 0;

    update_displayed_key(
        &mut displayed,
        &mut pending,
        &mut pending_samples,
        KeyCandidate {
            estimate: crate::key_detection::KeyEstimate {
                key: Some(DetectedKey {
                    root: 0,
                    mode: ScaleMode::Major,
                }),
                confidence: 88,
                #[cfg(debug_assertions)]
                diagnostics: String::new(),
            },
        },
        64,
        48_000.0,
        KeyResponsiveness::Balanced.display_switch_secs(),
    );

    assert_eq!(
        displayed.key,
        Some(DetectedKey {
            root: 0,
            mode: ScaleMode::Major
        })
    );
    assert_eq!(displayed.confidence, 88);
    assert!(pending.key.is_none());
}

#[test]
fn test_displayed_key_requires_persistence_before_switching() {
    let mut displayed = KeyDisplayState {
        key: Some(DetectedKey {
            root: 0,
            mode: ScaleMode::Major,
        }),
        confidence: 80,
    };
    let mut pending = KeyDisplayState::default();
    let mut pending_samples = 0;

    update_displayed_key(
        &mut displayed,
        &mut pending,
        &mut pending_samples,
        KeyCandidate {
            estimate: crate::key_detection::KeyEstimate {
                key: Some(DetectedKey {
                    root: 2,
                    mode: ScaleMode::Major,
                }),
                confidence: 72,
                #[cfg(debug_assertions)]
                diagnostics: String::new(),
            },
        },
        2_000,
        48_000.0,
        KeyResponsiveness::Balanced.display_switch_secs(),
    );

    assert_eq!(
        displayed.key,
        Some(DetectedKey {
            root: 0,
            mode: ScaleMode::Major
        })
    );
    assert_eq!(
        pending.key,
        Some(DetectedKey {
            root: 2,
            mode: ScaleMode::Major
        })
    );

    update_displayed_key(
        &mut displayed,
        &mut pending,
        &mut pending_samples,
        KeyCandidate {
            estimate: crate::key_detection::KeyEstimate {
                key: Some(DetectedKey {
                    root: 2,
                    mode: ScaleMode::Major,
                }),
                confidence: 76,
                #[cfg(debug_assertions)]
                diagnostics: String::new(),
            },
        },
        7_000,
        48_000.0,
        KeyResponsiveness::Balanced.display_switch_secs(),
    );

    assert_eq!(
        displayed.key,
        Some(DetectedKey {
            root: 2,
            mode: ScaleMode::Major
        })
    );
    assert_eq!(displayed.confidence, 76);
    assert!(pending.key.is_none());
}

#[test]
fn test_displayed_key_clears_pending_when_internal_returns() {
    let mut displayed = KeyDisplayState {
        key: Some(DetectedKey {
            root: 0,
            mode: ScaleMode::Major,
        }),
        confidence: 80,
    };
    let mut pending = KeyDisplayState::default();
    let mut pending_samples = 0;

    update_displayed_key(
        &mut displayed,
        &mut pending,
        &mut pending_samples,
        KeyCandidate {
            estimate: crate::key_detection::KeyEstimate {
                key: Some(DetectedKey {
                    root: 2,
                    mode: ScaleMode::Major,
                }),
                confidence: 70,
                #[cfg(debug_assertions)]
                diagnostics: String::new(),
            },
        },
        2_000,
        48_000.0,
        KeyResponsiveness::Balanced.display_switch_secs(),
    );
    update_displayed_key(
        &mut displayed,
        &mut pending,
        &mut pending_samples,
        KeyCandidate {
            estimate: crate::key_detection::KeyEstimate {
                key: Some(DetectedKey {
                    root: 0,
                    mode: ScaleMode::Major,
                }),
                confidence: 91,
                #[cfg(debug_assertions)]
                diagnostics: String::new(),
            },
        },
        2_000,
        48_000.0,
        KeyResponsiveness::Balanced.display_switch_secs(),
    );

    assert_eq!(
        displayed.key,
        Some(DetectedKey {
            root: 0,
            mode: ScaleMode::Major
        })
    );
    assert!(pending.key.is_none());
    assert_eq!(pending_samples, 0);
}

#[test]
fn test_scale_detection_tolerates_chromatic_passing_tone() {
    let history = VecDeque::from(vec![60, 62, 63, 64, 67, 69, 71, 72]);
    let (key, root, _) = detect_scale_balanced(
        &history,
        Some(60),
        Some(DetectedKey {
            root: 0,
            mode: ScaleMode::Major,
        }),
        &VecDeque::new(),
    );

    assert_eq!(key, "C Major");
    assert_eq!(root, 0);
}

#[test]
fn test_scale_detection_tolerates_blues_inflection() {
    let history = VecDeque::from(vec![60, 63, 64, 67, 70, 72, 67, 64]);
    let (key, root, _) = detect_scale_balanced(
        &history,
        Some(60),
        Some(DetectedKey {
            root: 0,
            mode: ScaleMode::Major,
        }),
        &VecDeque::new(),
    );

    assert_eq!(key, "C Major");
    assert_eq!(root, 0);
}

#[test]
fn test_scale_detection_uses_secondary_dominant_context() {
    let history = VecDeque::from(vec![62, 66, 69, 72, 67, 71, 74, 79, 60, 64, 67, 72]);
    let chord_history = make_chord_history(&[("D", "7"), ("G", "7"), ("C", "")]);
    let (key, root, _) = detect_scale_balanced(
        &history,
        Some(62),
        Some(DetectedKey {
            root: 0,
            mode: ScaleMode::Major,
        }),
        &chord_history,
    );

    assert_eq!(key, "C Major");
    assert_eq!(root, 0);
}

#[test]
fn test_scale_detection_prefers_stable_root_over_mode_churn() {
    let history = VecDeque::from(vec![60, 62, 63, 65, 67, 69, 70, 72]);
    let (key, root, _) = detect_scale_balanced(
        &history,
        Some(60),
        Some(DetectedKey {
            root: 0,
            mode: ScaleMode::Minor,
        }),
        &VecDeque::new(),
    );

    assert_eq!(key, "C Minor");
    assert_eq!(root, 0);
}

#[test]
fn test_scale_detection_recognizes_streamed_arpeggio_as_same_key() {
    let history = VecDeque::from(vec![60, 64, 67, 72, 76, 79, 83, 84]);
    let (key, root, _) = detect_scale_balanced(
        &history,
        Some(60),
        Some(DetectedKey {
            root: 0,
            mode: ScaleMode::Major,
        }),
        &VecDeque::new(),
    );

    assert_eq!(key, "C Major");
    assert_eq!(root, 0);
}

#[test]
fn test_scale_detection_distinguishes_tonicization_from_modulation() {
    let history = VecDeque::from(vec![
        60, 64, 67, 60, 64, 67, 62, 66, 69, 67, 71, 74, 60, 64, 67, 60, 64, 67,
    ]);
    let chord_history =
        make_chord_history(&[("C", ""), ("C", ""), ("D", "7"), ("G", ""), ("C", "")]);
    let (key, root, _) = detect_scale_balanced(
        &history,
        Some(60),
        Some(DetectedKey {
            root: 0,
            mode: ScaleMode::Major,
        }),
        &chord_history,
    );

    assert_eq!(key, "C Major");
    assert_eq!(root, 0);
}

#[test]
fn test_key_detection_reports_high_confidence_for_clear_tonal_center() {
    let history = VecDeque::from(vec![60, 64, 67, 72, 67, 64, 60, 67, 72, 76, 79, 84]);
    let estimate = detect_key_with_responsiveness(
        &history,
        Some(60),
        Some(DetectedKey {
            root: 0,
            mode: ScaleMode::Major,
        }),
        &make_chord_history(&[("C", ""), ("G", ""), ("C", "")]),
    );

    assert_eq!(
        estimate.key,
        Some(DetectedKey {
            root: 0,
            mode: ScaleMode::Major
        })
    );
    assert!(estimate.confidence >= 50);
    #[cfg(debug_assertions)]
    assert!(!estimate.diagnostics.is_empty());
}

#[test]
fn test_key_detection_returns_none_for_empty_input() {
    let estimate = detect_key(
        &[0.0; 12],
        &VecDeque::new(),
        None,
        None,
        &VecDeque::new(),
        KeyResponsiveness::Balanced,
    );

    assert!(estimate.key.is_none());
    assert_eq!(estimate.confidence, 0);
    #[cfg(debug_assertions)]
    assert!(estimate.diagnostics.is_empty());
}

#[test]
fn test_responsiveness_changes_switch_behavior() {
    let candidate = KeyCandidate {
        estimate: crate::key_detection::KeyEstimate {
            key: Some(DetectedKey {
                root: 2,
                mode: ScaleMode::Major,
            }),
            confidence: 75,
            #[cfg(debug_assertions)]
            diagnostics: String::new(),
        },
    };

    let mut stable_display = KeyDisplayState {
        key: Some(DetectedKey {
            root: 0,
            mode: ScaleMode::Major,
        }),
        confidence: 80,
    };
    let mut stable_pending = KeyDisplayState::default();
    let mut stable_samples = 0;

    let mut reactive_display = stable_display.clone();
    let mut reactive_pending = KeyDisplayState::default();
    let mut reactive_samples = 0;

    update_displayed_key(
        &mut stable_display,
        &mut stable_pending,
        &mut stable_samples,
        candidate.clone(),
        6_000,
        48_000.0,
        KeyResponsiveness::Stable.display_switch_secs(),
    );
    update_displayed_key(
        &mut reactive_display,
        &mut reactive_pending,
        &mut reactive_samples,
        candidate,
        6_000,
        48_000.0,
        KeyResponsiveness::Reactive.display_switch_secs(),
    );

    assert_eq!(
        stable_display.key,
        Some(DetectedKey {
            root: 0,
            mode: ScaleMode::Major
        })
    );
    assert_eq!(
        reactive_display.key,
        Some(DetectedKey {
            root: 2,
            mode: ScaleMode::Major
        })
    );
}

#[test]
fn test_regression_fixture_pop_phrase_stays_in_c_major() {
    let history = VecDeque::from(vec![
        60, 64, 67, 72, 71, 69, 67, 64, 62, 60, 62, 64, 67, 69, 67, 64, 60,
    ]);
    let estimate = detect_key_with_responsiveness(
        &history,
        Some(60),
        Some(DetectedKey {
            root: 0,
            mode: ScaleMode::Major,
        }),
        &make_chord_history(&[("C", ""), ("F", ""), ("G", "7"), ("C", "")]),
    );

    assert_eq!(
        estimate.key,
        Some(DetectedKey {
            root: 0,
            mode: ScaleMode::Major
        })
    );
}

#[test]
fn test_regression_fixture_minor_phrase_stays_in_a_minor() {
    let history = VecDeque::from(vec![
        57, 60, 64, 69, 67, 64, 60, 57, 59, 60, 64, 67, 69, 72, 69, 64, 57,
    ]);
    let estimate = detect_key_with_responsiveness(
        &history,
        Some(57),
        Some(DetectedKey {
            root: 9,
            mode: ScaleMode::Minor,
        }),
        &make_chord_history(&[("Am", ""), ("Dm", ""), ("E", "7"), ("Am", "")]),
    );

    assert_eq!(
        estimate.key,
        Some(DetectedKey {
            root: 9,
            mode: ScaleMode::Minor
        })
    );
}

#[test]
fn test_process_path_detects_chord_and_updates_state() {
    let mut plugin = ChordLens::default();

    process_until_settled(&mut plugin, vec![note_on(60), note_on(64), note_on(67)], 3);

    let state = plugin.chord_state.read().clone();
    assert_eq!(state.chord_info.root, "C");
    assert_eq!(state.chord_info.quality, "");
    assert_eq!(state.nashville_text, "I");
    assert!(!state.chromatic_mode);
    assert!(state.key_text.starts_with("Detected:"));
}

#[test]
fn test_process_path_chromatic_mode_keeps_chord_detection_and_disables_nashville() {
    let mut plugin = ChordLens {
        params: std::sync::Arc::new(crate::ChordLensParams {
            key_root: nih_plug::prelude::EnumParam::new("Force Root", crate::KeyRoot::Chromatic),
            ..crate::ChordLensParams::default()
        }),
        ..ChordLens::default()
    };

    process_until_settled(&mut plugin, vec![note_on(60), note_on(64), note_on(67)], 3);

    let state = plugin.chord_state.read().clone();
    assert_eq!(state.key_text, "User: Chromatic");
    assert_eq!(state.chord_info.root, "C");
    assert_eq!(state.chord_info.quality, "");
    assert_eq!(state.nashville_text, "");
    assert!(state.chromatic_mode);
    assert!(state.scale_intervals.is_empty());
}

#[test]
fn test_process_path_reset_history_clears_runtime_state() {
    let mut plugin = ChordLens::default();

    process_until_settled(&mut plugin, vec![note_on(60), note_on(64), note_on(67)], 14);
    assert!(!plugin.key_history.note_history().is_empty());
    assert!(!plugin.key_history.chord_history().is_empty());
    assert!(!plugin.chord_state.read().chord_history.is_empty());

    plugin.reset_history.store(true, Ordering::Relaxed);
    run_process(&mut plugin, Vec::new());

    let state = plugin.chord_state.read().clone();
    assert!(plugin.key_history.active_note_list().is_empty());
    assert!(plugin.key_history.note_history().is_empty());
    assert!(plugin.key_history.chord_history().is_empty());
    assert!(plugin.internal_detected_key.is_none());
    assert!(plugin.displayed_key_state.key.is_none());
    assert!(plugin.pending_display_key_state.key.is_none());
    assert!(state.chord_history.is_empty());
}

#[test]
fn test_process_path_keeps_duplicate_note_active_after_single_release() {
    let mut plugin = ChordLens::default();

    process_until_settled(
        &mut plugin,
        vec![
            note_on(60),
            note_on(60),
            note_on(64),
            note_on(67),
            note_off(60),
        ],
        3,
    );

    let state = plugin.chord_state.read().clone();
    assert_eq!(state.chord_info.root, "C");
    assert_eq!(state.chord_info.quality, "");
}
