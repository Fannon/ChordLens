//! # chord.rs — Chord Recognition Engine
//!
//! Pure interval-math chord detector.

use std::fmt;

#[derive(Clone, Debug, PartialEq, Default)]
pub enum NoteRole {
    #[default]
    Normal,
    Root,
    Bass,
}

#[derive(Clone, Debug, Default)]
pub struct ChordInfo {
    pub root: String,
    pub quality: String,
    pub omitted: String,
    pub slash: String,
    pub inversion: String,
    pub degree: String,
    pub active_notes: Vec<(String, NoteRole)>,
    pub active_midi: Vec<(u8, NoteRole)>,
}

impl fmt::Display for ChordInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.root == "–" || self.root.is_empty() {
            write!(f, "–")
        } else {
            write!(f, "{}{}{}{}", self.root, self.quality, self.omitted, self.slash)
        }
    }
}

const NOTE_NAMES_SHARP: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
const NOTE_NAMES_FLAT: [&str; 12] = ["C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B"];
const FLAT_KEYS: [u8; 6] = [5, 10, 3, 8, 1, 6];

pub fn get_note_names(scale_root: u8) -> &'static [&'static str; 12] {
    if FLAT_KEYS.contains(&(scale_root % 12)) { &NOTE_NAMES_FLAT } else { &NOTE_NAMES_SHARP }
}

pub fn midi_to_name(note: u8, scale_root: u8) -> String {
    let names = get_note_names(scale_root);
    let pc = (note % 12) as usize;
    let octave = (note / 12) as i32 - 1; 
    format!("{}{}", names[pc], octave)
}

pub fn pc_name(pc: u8, scale_root: u8) -> &'static str {
    get_note_names(scale_root)[(pc % 12) as usize]
}

struct ChordTemplate {
    /// Semitone intervals above the root (root=0 is implicit, not listed).
    intervals: &'static [u8],
    /// String suffix appended to the root name.
    quality: &'static str,
}

// Strictly ordered by complexity (matching richest chords first)
static TEMPLATES: &[ChordTemplate] = &[
    // ── 13th chords ──────────────────────────────────────────────────────────
    ChordTemplate { intervals: &[4, 7, 10, 14, 17, 21], quality: "13" },
    ChordTemplate { intervals: &[3, 7, 10, 14, 17, 21], quality: "m13" },
    ChordTemplate { intervals: &[4, 7, 11, 14, 17, 21], quality: "maj13" },
    // ── 11th chords ──────────────────────────────────────────────────────────
    ChordTemplate { intervals: &[4, 7, 10, 14, 17], quality: "11" },
    ChordTemplate { intervals: &[3, 7, 10, 14, 17], quality: "m11" },
    ChordTemplate { intervals: &[3, 6, 10, 14, 17], quality: "m11b5" },
    ChordTemplate { intervals: &[4, 7, 11, 14, 18], quality: "maj7#11" },
    ChordTemplate { intervals: &[4, 7, 10, 14, 18], quality: "9#11" },
    // ── 9th chords ───────────────────────────────────────────────────────────
    ChordTemplate { intervals: &[4, 7, 10, 14, 20], quality: "9b13" },
    ChordTemplate { intervals: &[4, 7, 9, 14], quality: "6/9" },
    ChordTemplate { intervals: &[3, 7, 9, 14], quality: "m6/9" },
    ChordTemplate { intervals: &[4, 7, 10, 14], quality: "9" },
    ChordTemplate { intervals: &[3, 7, 10, 14], quality: "m9" },
    ChordTemplate { intervals: &[4, 7, 11, 14], quality: "maj9" },
    ChordTemplate { intervals: &[4, 7, 10, 13], quality: "7b9" },
    ChordTemplate { intervals: &[4, 7, 10, 15], quality: "7#9" },
    // ── 7th chords ───────────────────────────────────────────────────────────
    ChordTemplate { intervals: &[4, 7, 10, 18], quality: "7#11" },
    ChordTemplate { intervals: &[4, 7, 10], quality: "7" },
    ChordTemplate { intervals: &[4, 7, 11], quality: "maj7" },
    ChordTemplate { intervals: &[3, 7, 10], quality: "m7" },
    ChordTemplate { intervals: &[3, 6, 10], quality: "ø7" },
    ChordTemplate { intervals: &[3, 6, 9],  quality: "dim7" },
    ChordTemplate { intervals: &[4, 8, 10], quality: "7aug" },
    // ── Jazz Omissions (Common "no 5th" etc.) ────────────────────────────────
    ChordTemplate { intervals: &[4, 10, 21], quality: "13(no5,no9,no11)" },
    ChordTemplate { intervals: &[4, 10, 14, 21], quality: "13(no5,no11)" },
    ChordTemplate { intervals: &[4, 10, 14], quality: "9(no5)" },
    ChordTemplate { intervals: &[4, 10], quality: "7(no5)" },
    ChordTemplate { intervals: &[3, 10], quality: "m7(no5)" },
    // ── Added tones ──────────────────────────────────────────────────────────
    ChordTemplate { intervals: &[4, 7, 9],  quality: "6" },
    ChordTemplate { intervals: &[3, 7, 9],  quality: "m6" },
    ChordTemplate { intervals: &[3, 7, 14], quality: "madd9" },
    ChordTemplate { intervals: &[4, 7, 14], quality: "add9" },
    // ── Basic triads ─────────────────────────────────────────────────────────
    ChordTemplate { intervals: &[4, 7],     quality: "" },
    ChordTemplate { intervals: &[3, 7],     quality: "m" },
    ChordTemplate { intervals: &[3, 6],     quality: "dim" },
    ChordTemplate { intervals: &[4, 8],     quality: "aug" },
    ChordTemplate { intervals: &[2, 7],     quality: "sus2" },
    ChordTemplate { intervals: &[5, 7],     quality: "sus4" },
    ChordTemplate { intervals: &[7],        quality: "5" },
    ChordTemplate { intervals: &[3],        quality: "m(no5)" },
    ChordTemplate { intervals: &[4],        quality: "(no5)" },
];

pub fn detect(active_notes: &[u8], scale_root: u8, allow_rootless: bool) -> ChordInfo {
    let mut sorted_notes: Vec<u8> = active_notes.to_vec();
    sorted_notes.sort_unstable();
    if sorted_notes.is_empty() { return ChordInfo::default(); }
    let mut pcs: Vec<u8> = sorted_notes.iter().map(|n| n % 12).collect();
    pcs.sort_unstable(); pcs.dedup();
    if pcs.len() == 1 {
        let pc = pcs[0]; let root_name = pc_name(pc, scale_root);
        return ChordInfo { root: root_name.to_string(), degree: "I".to_string(), active_notes: sorted_notes.iter().map(|&n| (midi_to_name(n, scale_root), NoteRole::Root)).collect(), active_midi: sorted_notes.iter().map(|&n| (n, NoteRole::Root)).collect(), ..Default::default() };
    }
    let bass_midi = sorted_notes[0]; let bass_pc = bass_midi % 12;
    let mut best: Option<(u8, &'static str, usize)> = None;
    let mut roots_to_check = pcs.clone();
    if allow_rootless {
        let t = scale_root % 12; let d = (scale_root + 7) % 12;
        if !roots_to_check.contains(&t) { roots_to_check.push(t); }
        if !roots_to_check.contains(&d) { roots_to_check.push(d); }
    }
    for &root_pc in &roots_to_check {
        let intervals_pc: Vec<u8> = pcs.iter().filter(|&&pc| pc != root_pc).map(|&pc| (pc + 12 - root_pc) % 12).collect();
        let root_midi = sorted_notes.iter().find(|&&n| n % 12 == root_pc).copied().unwrap_or(root_pc);
        let compound: Vec<u8> = sorted_notes.iter().filter(|&&n| n % 12 != root_pc).filter_map(|&n| {
            let diff = n as i16 - root_midi as i16;
            if diff >= 0 && diff <= 36 { Some(diff as u8) } else { Some(diff.rem_euclid(12) as u8) }
        }).collect();
        'template: for tmpl in TEMPLATES {
            // Strict matching: Every interval in the template MUST be present
            for &ti in tmpl.intervals {
                let r = ti % 12;
                if !compound.contains(&ti) && !intervals_pc.contains(&r) { continue 'template; }
            }
            // Strict match also ensures no OTHER pitch classes are present
            let mut covered = vec![root_pc];
            for &ti in tmpl.intervals { covered.push((root_pc + ti) % 12); }
            covered.sort_unstable(); covered.dedup();
            for &p in &pcs { if !covered.contains(&p) { continue 'template; } }

            let score = tmpl.intervals.len();
            let better = match &best { None => true, Some((br, _, bs)) => { if score > *bs { true } else if score == *bs { root_pc == bass_pc && *br != bass_pc } else { false } } };
            if better { best = Some((root_pc, tmpl.quality, score)); }
        }
    }
    match best {
        None => {
            ChordInfo { active_notes: sorted_notes.iter().map(|&n| (midi_to_name(n, scale_root), if n == bass_midi { NoteRole::Bass } else { NoteRole::Normal })).collect(), active_midi: sorted_notes.iter().map(|&n| (n, NoteRole::Normal)).collect(), ..Default::default() }
        }
        Some((r_pc, quality, _)) => {
            let r_name = pc_name(r_pc, scale_root);
            let mut qual = quality.to_string(); let mut omitted = String::new();
            if let Some(i) = qual.find("(no") { omitted = qual[i..].to_string(); qual.truncate(i); }
            let slash = if bass_pc != r_pc { format!("/{}", pc_name(bass_pc, scale_root)) } else { String::new() };
            let rel = (r_pc as i32 + 12 - scale_root as i32) % 12;
            let mut deg = match rel { 0 => "I", 1 => "bII", 2 => "II", 3 => "bIII", 4 => "III", 5 => "IV", 6 => "#IV", 7 => "V", 8 => "bVI", 9 => "VI", 10 => "bVII", 11 => "VII", _ => "?" }.to_string();
            if (qual.contains('m') || qual.contains("dim") || qual.contains('ø')) && !qual.contains("maj") { deg = deg.to_lowercase(); }
            let first_r_midi = sorted_notes.iter().find(|&&n| n % 12 == r_pc).copied().unwrap_or(bass_midi);
            let inv = if slash.is_empty() { String::new() } else {
                let mut v = vec![r_pc];
                if let Some(t) = TEMPLATES.iter().find(|t| t.quality == quality) { for &ti in t.intervals { v.push((r_pc + ti) % 12); } }
                v.sort_unstable(); v.dedup();
                let bc = sorted_notes.iter().filter(|&&n| n < first_r_midi && v.contains(&(n % 12))).map(|&n| n % 12).collect::<std::collections::HashSet<_>>().len();
                let bass_rel = (bass_pc + 12 - r_pc) % 12;
                if bc > 0 {
                    match bass_rel {
                        3 | 4 => "1st inv.".to_string(),
                        6 | 7 | 8 => "2nd inv.".to_string(),
                        9 | 10 | 11 => "3rd inv.".to_string(),
                        _ => String::new()
                    }
                } else { String::new() }
            };
            ChordInfo { root: r_name.to_string(), quality: qual, omitted, slash, inversion: inv, degree: deg, active_notes: sorted_notes.iter().map(|&n| { let role = if n == bass_midi && bass_pc != r_pc { NoteRole::Bass } else if n % 12 == r_pc { NoteRole::Root } else { NoteRole::Normal }; (midi_to_name(n, scale_root), role) }).collect(), active_midi: sorted_notes.iter().map(|&n| { let role = if n == bass_midi && bass_pc != r_pc { NoteRole::Bass } else if n % 12 == r_pc { NoteRole::Root } else { NoteRole::Normal }; (n, role) }).collect() }
        }
    }
}

pub fn detect_scale(history: &std::collections::VecDeque<u8>, bass: Option<u8>, cur_key: &str) -> (String, u8, Vec<i32>) {
    if history.is_empty() { return ("Unknown".to_string(), 0, vec![]); }
    let mut counts = [0i32; 12]; let l = history.len();
    for (i, &m) in history.iter().enumerate() { counts[(m % 12) as usize] += if i > l / 2 { 2 } else { 1 }; }
    let modes = [("Major", vec![0, 2, 4, 5, 7, 9, 11], 100), ("Minor", vec![0, 2, 3, 5, 7, 8, 10], 90), ("Maj Pent.", vec![0, 2, 4, 7, 9], 80), ("Min Pent.", vec![0, 3, 5, 7, 10], 80), ("Dorian", vec![0, 2, 3, 5, 7, 9, 10], 60), ("Mixolydian", vec![0, 2, 4, 5, 7, 9, 10], 50), ("Lydian", vec![0, 2, 4, 6, 7, 9, 11], 40), ("Phrygian", vec![0, 1, 3, 5, 7, 8, 10], 30), ("Locrian", vec![0, 1, 3, 5, 6, 8, 10], 20)];
    let mut bs = -1000; let mut bn = String::from("Unknown");
    for r in 0..12 {
        for (m, ints, w) in modes.iter() {
            let mut s = *w + counts[r as usize] * 10;
            if let Some(b) = bass { if b % 12 == r as u8 { s += 50; } }
            for pc in 0..12 { let f = counts[pc as usize]; if f == 0 { continue; } let rel = (pc + 12 - r) % 12; if ints.contains(&(rel as i32)) { s += f * 5; } else { s -= f * 50; } }
            let cand = format!("{} {}", pc_name(r as u8, r as u8), m);
            if cand == cur_key { s += 150; }
            if s > bs { bs = s; bn = cand; }
        }
    }
    let mut rp = 0; let mut bi = vec![];
    for r in 0..12 { for (m, ints, _) in modes.iter() { if format!("{} {}", pc_name(r as u8, r as u8), m) == bn { rp = r; bi = ints.clone(); } } }
    (bn, rp as u8, bi)
}
