use crate::{ChordHistoryEntry, KeyResponsiveness};
use std::collections::VecDeque;

const MAJOR_INTERVALS: [i32; 7] = [0, 2, 4, 5, 7, 9, 11];
const MINOR_INTERVALS: [i32; 7] = [0, 2, 3, 5, 7, 8, 10];
const MAJ_PENT_INTERVALS: [i32; 5] = [0, 2, 4, 7, 9];
const MIN_PENT_INTERVALS: [i32; 5] = [0, 3, 5, 7, 10];
const DORIAN_INTERVALS: [i32; 7] = [0, 2, 3, 5, 7, 9, 10];
const MIXOLYDIAN_INTERVALS: [i32; 7] = [0, 2, 4, 5, 7, 9, 10];
const LYDIAN_INTERVALS: [i32; 7] = [0, 2, 4, 6, 7, 9, 11];
const PHRYGIAN_INTERVALS: [i32; 7] = [0, 1, 3, 5, 7, 8, 10];
const LOCRIAN_INTERVALS: [i32; 7] = [0, 1, 3, 5, 6, 8, 10];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScaleMode {
    Major,
    Minor,
    MajPent,
    MinPent,
    Dorian,
    Mixolydian,
    Lydian,
    Phrygian,
    Locrian,
}

impl ScaleMode {
    pub fn display_name(&self) -> &'static str {
        match self {
            ScaleMode::Major => "Major",
            ScaleMode::Minor => "Minor",
            ScaleMode::MajPent => "Maj Pent.",
            ScaleMode::MinPent => "Min Pent.",
            ScaleMode::Dorian => "Dorian",
            ScaleMode::Mixolydian => "Mixolydian",
            ScaleMode::Lydian => "Lydian",
            ScaleMode::Phrygian => "Phrygian",
            ScaleMode::Locrian => "Locrian",
        }
    }

    pub fn intervals(&self) -> &'static [i32] {
        match self {
            ScaleMode::Major => &MAJOR_INTERVALS,
            ScaleMode::Minor => &MINOR_INTERVALS,
            ScaleMode::MajPent => &MAJ_PENT_INTERVALS,
            ScaleMode::MinPent => &MIN_PENT_INTERVALS,
            ScaleMode::Dorian => &DORIAN_INTERVALS,
            ScaleMode::Mixolydian => &MIXOLYDIAN_INTERVALS,
            ScaleMode::Lydian => &LYDIAN_INTERVALS,
            ScaleMode::Phrygian => &PHRYGIAN_INTERVALS,
            ScaleMode::Locrian => &LOCRIAN_INTERVALS,
        }
    }

    fn base_weight(&self) -> i32 {
        match self {
            ScaleMode::Major => 100,
            ScaleMode::Minor => 90,
            ScaleMode::MajPent | ScaleMode::MinPent => 80,
            ScaleMode::Dorian => 60,
            ScaleMode::Mixolydian => 50,
            ScaleMode::Lydian => 40,
            ScaleMode::Phrygian => 30,
            ScaleMode::Locrian => 20,
        }
    }

    fn is_minor_like(&self) -> bool {
        matches!(
            self,
            ScaleMode::Minor
                | ScaleMode::MinPent
                | ScaleMode::Dorian
                | ScaleMode::Phrygian
                | ScaleMode::Locrian
        )
    }
}

const ALL_MODES: [ScaleMode; 9] = [
    ScaleMode::Major,
    ScaleMode::Minor,
    ScaleMode::MajPent,
    ScaleMode::MinPent,
    ScaleMode::Dorian,
    ScaleMode::Mixolydian,
    ScaleMode::Lydian,
    ScaleMode::Phrygian,
    ScaleMode::Locrian,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DetectedKey {
    pub root: u8,
    pub mode: ScaleMode,
}

impl DetectedKey {
    pub fn display_name(&self) -> String {
        format!(
            "{} {}",
            crate::chord::pc_name(self.root, self.root),
            self.mode.display_name()
        )
    }

    pub fn intervals(&self) -> &'static [i32] {
        self.mode.intervals()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyEstimate {
    pub key: Option<DetectedKey>,
    pub confidence: u8,
}

#[derive(Clone, Copy)]
struct CandidateScore {
    key: DetectedKey,
    total_score: i32,
    miss_weight: i32,
}

fn parse_note_name(name: &str) -> Option<u8> {
    match name {
        "C" => Some(0),
        "C#" | "Db" => Some(1),
        "D" => Some(2),
        "D#" | "Eb" => Some(3),
        "E" => Some(4),
        "F" => Some(5),
        "F#" | "Gb" => Some(6),
        "G" => Some(7),
        "G#" | "Ab" => Some(8),
        "A" => Some(9),
        "A#" | "Bb" => Some(10),
        "B" => Some(11),
        _ => None,
    }
}

fn is_minor_quality(quality: &str) -> bool {
    (quality.starts_with('m') && !quality.starts_with("maj"))
        || quality.contains('ø')
        || quality.contains("dim")
}

fn is_dominant_quality(quality: &str) -> bool {
    quality.contains('7') && !quality.contains("maj7")
}

fn semitone_distance(a: u8, b: u8) -> u8 {
    let diff = (a + 12 - b) % 12;
    diff.min(12 - diff)
}

fn is_contextual_outlier(rel: i32, mode: ScaleMode) -> bool {
    match mode {
        ScaleMode::Major | ScaleMode::MajPent | ScaleMode::Mixolydian => matches!(rel, 3 | 6 | 10),
        ScaleMode::Minor | ScaleMode::MinPent | ScaleMode::Dorian | ScaleMode::Phrygian => {
            matches!(rel, 4 | 6 | 11)
        }
        ScaleMode::Lydian => matches!(rel, 3 | 10),
        ScaleMode::Locrian => matches!(rel, 6),
    }
}

fn is_passing_tone(history: &VecDeque<u8>, index: usize, key: DetectedKey) -> bool {
    if index == 0 || index + 1 >= history.len() {
        return false;
    }

    let cur_pc = history[index] % 12;
    let prev_pc = history[index - 1] % 12;
    let next_pc = history[index + 1] % 12;
    let prev_rel = (prev_pc as i32 + 12 - key.root as i32) % 12;
    let next_rel = (next_pc as i32 + 12 - key.root as i32) % 12;

    if !key.intervals().contains(&prev_rel) || !key.intervals().contains(&next_rel) {
        return false;
    }

    semitone_distance(cur_pc, prev_pc) <= 1 && semitone_distance(cur_pc, next_pc) <= 1
}

fn score_chord_history(key: DetectedKey, chord_history: &VecDeque<ChordHistoryEntry>) -> i32 {
    if chord_history.is_empty() {
        return 0;
    }

    let mut score = 0;
    let recent: Vec<_> = chord_history.iter().rev().take(6).cloned().collect();
    for (idx, entry) in recent.iter().enumerate() {
        let Some(chord_root) = parse_note_name(&entry.root) else {
            continue;
        };
        let weight = match idx {
            0 => 10,
            1 => 8,
            2 => 6,
            3 => 4,
            _ => 2,
        };
        let rel = (chord_root as i32 + 12 - key.root as i32) % 12;
        let minor_quality = is_minor_quality(&entry.quality);

        match rel {
            0 => {
                score += weight * 4;
                if key.mode.is_minor_like() == minor_quality {
                    score += weight * 3;
                }
            }
            5 => score += weight * 2,
            7 => {
                score += weight * 3;
                if is_dominant_quality(&entry.quality) {
                    score += weight * 2;
                }
            }
            2 if minor_quality || entry.quality.contains('ø') => score += weight * 2,
            _ => {}
        }
    }

    let mut chronological: Vec<_> = chord_history.iter().rev().take(6).cloned().collect();
    chronological.reverse();

    for window in chronological.windows(3) {
        let [a, b, c] = window else {
            continue;
        };
        let (Some(a_root), Some(b_root), Some(c_root)) = (
            parse_note_name(&a.root),
            parse_note_name(&b.root),
            parse_note_name(&c.root),
        ) else {
            continue;
        };
        let rels = [
            (a_root as i32 + 12 - key.root as i32) % 12,
            (b_root as i32 + 12 - key.root as i32) % 12,
            (c_root as i32 + 12 - key.root as i32) % 12,
        ];
        if rels == [2, 7, 0] && is_minor_quality(&a.quality) && is_dominant_quality(&b.quality) {
            score += 48;
        }
    }

    for window in chronological.windows(2) {
        let [a, b] = window else {
            continue;
        };
        let (Some(a_root), Some(b_root)) = (parse_note_name(&a.root), parse_note_name(&b.root))
        else {
            continue;
        };
        let rels = [
            (a_root as i32 + 12 - key.root as i32) % 12,
            (b_root as i32 + 12 - key.root as i32) % 12,
        ];
        if rels == [7, 0] && is_dominant_quality(&a.quality) {
            score += 24;
        }
    }

    score
}

fn weighted_pitch_counts(history: &VecDeque<u8>) -> [i32; 12] {
    let mut counts = [0i32; 12];
    let len = history.len();
    for (i, &note) in history.iter().enumerate() {
        let age = len - i - 1;
        let weight = match age {
            0..=3 => 6,
            4..=7 => 4,
            8..=15 => 2,
            _ => 1,
        };
        counts[(note % 12) as usize] += weight;
    }
    counts
}

fn score_candidate(
    key: DetectedKey,
    counts: &[i32; 12],
    history: &VecDeque<u8>,
    bass: Option<u8>,
    chord_history: &VecDeque<ChordHistoryEntry>,
) -> CandidateScore {
    let mut score = key.mode.base_weight();
    let mut miss_weight = 0i32;

    score += counts[key.root as usize] * 8;
    let dominant = (key.root + 7) % 12;
    let mediant = if matches!(key.mode, ScaleMode::Minor | ScaleMode::MinPent) {
        (key.root + 3) % 12
    } else {
        (key.root + 4) % 12
    };
    score += counts[dominant as usize] * 3;
    score += counts[mediant as usize] * 2;
    score += score_chord_history(key, chord_history);

    if let Some(bass_note) = bass {
        let bass_pc = bass_note % 12;
        if bass_pc == key.root {
            score += 24;
        } else if bass_pc == dominant {
            score += 8;
        }
    }

    for (pc, &weight) in counts.iter().enumerate() {
        if weight == 0 {
            continue;
        }

        let rel = (pc + 12 - key.root as usize) as i32 % 12;
        if key.intervals().contains(&rel) {
            score += weight * 6;
        } else {
            let contextual = is_contextual_outlier(rel, key.mode);
            let penalty = if contextual { 3 } else { 8 };
            miss_weight += if contextual { weight / 2 } else { weight };
            score -= weight * penalty;
        }
    }

    for (idx, note) in history.iter().enumerate() {
        let rel = ((*note % 12) as i32 + 12 - key.root as i32) % 12;
        if key.intervals().contains(&rel) {
            continue;
        }
        if is_passing_tone(history, idx, key) {
            score += 6;
            miss_weight = miss_weight.saturating_sub(2);
        }
    }

    CandidateScore {
        key,
        total_score: score,
        miss_weight,
    }
}

fn score_candidates(
    history: &VecDeque<u8>,
    bass: Option<u8>,
    chord_history: &VecDeque<ChordHistoryEntry>,
) -> Vec<CandidateScore> {
    let counts = weighted_pitch_counts(history);
    let mut scored = Vec::with_capacity(12 * ALL_MODES.len());
    for root in 0..12u8 {
        for mode in ALL_MODES {
            scored.push(score_candidate(
                DetectedKey { root, mode },
                &counts,
                history,
                bass,
                chord_history,
            ));
        }
    }
    scored.sort_by(|a, b| b.total_score.cmp(&a.total_score));
    scored
}

fn choose_key(
    scored: &[CandidateScore],
    current_key: Option<DetectedKey>,
    responsiveness: KeyResponsiveness,
) -> CandidateScore {
    let best = scored[0];
    let Some(current_key) = current_key else {
        return best;
    };
    let Some(current_score) = scored.iter().copied().find(|c| c.key == current_key) else {
        return best;
    };

    if best.key != current_key
        && best.total_score
            < current_score.total_score
                + responsiveness.key_switch_margin(current_score.miss_weight)
    {
        return current_score;
    }

    if best.key.root == current_key.root
        && best.key.mode != current_key.mode
        && best.total_score < current_score.total_score + responsiveness.mode_switch_margin()
    {
        return current_score;
    }

    best
}

fn confidence_from_scores(selected: CandidateScore, scored: &[CandidateScore]) -> u8 {
    let runner_up = scored
        .iter()
        .find(|candidate| candidate.key != selected.key)
        .copied()
        .unwrap_or(selected);
    let gap = (selected.total_score - runner_up.total_score).max(0);
    let miss_penalty = selected.miss_weight.min(24);
    (40 + gap.min(50) - miss_penalty).clamp(0, 100) as u8
}

pub fn detect_key(
    history: &VecDeque<u8>,
    bass: Option<u8>,
    current_key: Option<DetectedKey>,
    chord_history: &VecDeque<ChordHistoryEntry>,
    responsiveness: KeyResponsiveness,
) -> KeyEstimate {
    if history.is_empty() {
        return KeyEstimate {
            key: None,
            confidence: 0,
        };
    }

    let scored = score_candidates(history, bass, chord_history);
    let selected = choose_key(&scored, current_key, responsiveness);
    KeyEstimate {
        key: Some(selected.key),
        confidence: confidence_from_scores(selected, &scored),
    }
}
