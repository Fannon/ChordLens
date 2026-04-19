//! # chord.rs — Chord Recognition Engine
//!
//! Pure interval-math chord detector.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum NoteRole {
    #[default]
    Normal,
    Root,
    Bass,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ActiveMidiNote {
    pub midi: u8,
    pub role: NoteRole,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActiveMidiNotes {
    notes: [ActiveMidiNote; 128],
    len: u8,
}

impl Default for ActiveMidiNotes {
    fn default() -> Self {
        Self {
            notes: [ActiveMidiNote::default(); 128],
            len: 0,
        }
    }
}

impl ActiveMidiNotes {
    fn push(&mut self, midi: u8, role: NoteRole) {
        let idx = self.len as usize;
        self.notes[idx] = ActiveMidiNote { midi, role };
        self.len += 1;
    }

    pub fn iter(&self) -> impl Iterator<Item = &ActiveMidiNote> {
        self.notes[..self.len as usize].iter()
    }

    pub fn len(&self) -> usize {
        self.len as usize
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ChordInfo {
    pub root_pc: Option<u8>,
    pub quality: &'static str,
    pub omitted: &'static str,
    pub bass_pc: Option<u8>,
    pub inversion: &'static str,
    pub active_midi: ActiveMidiNotes,
}

impl ChordInfo {
    pub fn root_label(&self, scale_root: u8) -> String {
        self.root_pc
            .map(|pc| pc_name(pc, scale_root).to_string())
            .unwrap_or_default()
    }

    pub fn slash_label(&self, scale_root: u8) -> String {
        match (self.root_pc, self.bass_pc) {
            (Some(root_pc), Some(bass_pc)) if bass_pc != root_pc => {
                format!("/{}", pc_name(bass_pc, scale_root))
            }
            _ => String::new(),
        }
    }

    pub fn degree_text(&self, scale_root: u8) -> String {
        let Some(root_pc) = self.root_pc else {
            return String::new();
        };

        let rel = (root_pc as i32 + 12 - scale_root as i32) % 12;
        let mut degree = match rel {
            0 => "I",
            1 => "bII",
            2 => "II",
            3 => "bIII",
            4 => "III",
            5 => "IV",
            6 => "#IV",
            7 => "V",
            8 => "bVI",
            9 => "VI",
            10 => "bVII",
            11 => "VII",
            _ => "?",
        }
        .to_string();

        if (self.quality.contains('m')
            || self.quality.contains("dim")
            || self.quality.contains('ø'))
            && !self.quality.contains("maj")
        {
            degree = degree.to_lowercase();
        }

        degree
    }

    pub fn display_name(&self, scale_root: u8) -> String {
        let root = self.root_label(scale_root);
        if root.is_empty() {
            "–".to_string()
        } else {
            format!(
                "{}{}{}{}",
                root,
                self.quality,
                self.omitted,
                self.slash_label(scale_root)
            )
        }
    }
}

const NOTE_NAMES_SHARP: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];
const NOTE_NAMES_FLAT: [&str; 12] = [
    "C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B",
];
const FLAT_KEYS: [u8; 6] = [5, 10, 3, 8, 1, 6];

pub fn get_note_names(scale_root: u8) -> &'static [&'static str; 12] {
    if FLAT_KEYS.contains(&(scale_root % 12)) {
        &NOTE_NAMES_FLAT
    } else {
        &NOTE_NAMES_SHARP
    }
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

#[cfg(test)]
pub(crate) fn parse_pitch_class_prefix(label: &str, scale_root: u8) -> Option<u8> {
    let mut best_match = None;
    let mut best_len = 0;

    for pc in 0..12 {
        let note_name = pc_name(pc, scale_root);
        if label.starts_with(note_name) && note_name.len() > best_len {
            best_match = Some(pc);
            best_len = note_name.len();
        }
    }

    best_match
}

struct ChordTemplate {
    /// Semitone intervals above the root (root=0 is implicit, not listed).
    intervals: &'static [u8],
    /// String suffix appended to the root name.
    quality: &'static str,
}

fn split_quality_and_omitted(quality: &'static str) -> (&'static str, &'static str) {
    if let Some(idx) = quality.find("(no") {
        (&quality[..idx], &quality[idx..])
    } else {
        (quality, "")
    }
}

// Strictly ordered by complexity (matching richest chords first)
static TEMPLATES: &[ChordTemplate] = &[
    // ── 13th chords ──────────────────────────────────────────────────────────
    ChordTemplate {
        intervals: &[4, 7, 10, 14, 17, 21],
        quality: "13",
    },
    ChordTemplate {
        intervals: &[3, 7, 10, 14, 17, 21],
        quality: "m13",
    },
    ChordTemplate {
        intervals: &[4, 7, 11, 14, 17, 21],
        quality: "maj13",
    },
    // ── 11th chords ──────────────────────────────────────────────────────────
    ChordTemplate {
        intervals: &[4, 7, 10, 14, 17],
        quality: "11",
    },
    ChordTemplate {
        intervals: &[3, 7, 10, 14, 17],
        quality: "m11",
    },
    ChordTemplate {
        intervals: &[3, 6, 10, 14, 17],
        quality: "m11b5",
    },
    ChordTemplate {
        intervals: &[4, 7, 11, 14, 18],
        quality: "maj7#11",
    },
    ChordTemplate {
        intervals: &[4, 7, 10, 14, 18],
        quality: "9#11",
    },
    // ── 9th chords ───────────────────────────────────────────────────────────
    ChordTemplate {
        intervals: &[4, 7, 10, 14, 20],
        quality: "9b13",
    },
    ChordTemplate {
        intervals: &[4, 7, 9, 14],
        quality: "6/9",
    },
    ChordTemplate {
        intervals: &[3, 7, 9, 14],
        quality: "m6/9",
    },
    ChordTemplate {
        intervals: &[4, 7, 10, 14],
        quality: "9",
    },
    ChordTemplate {
        intervals: &[3, 7, 10, 14],
        quality: "m9",
    },
    ChordTemplate {
        intervals: &[4, 7, 11, 14],
        quality: "maj9",
    },
    ChordTemplate {
        intervals: &[4, 7, 10, 13],
        quality: "7b9",
    },
    ChordTemplate {
        intervals: &[4, 7, 10, 15],
        quality: "7#9",
    },
    // ── 7th chords ───────────────────────────────────────────────────────────
    ChordTemplate {
        intervals: &[4, 7, 10, 18],
        quality: "7#11",
    },
    ChordTemplate {
        intervals: &[4, 7, 10],
        quality: "7",
    },
    ChordTemplate {
        intervals: &[4, 7, 11],
        quality: "maj7",
    },
    ChordTemplate {
        intervals: &[3, 7, 10],
        quality: "m7",
    },
    ChordTemplate {
        intervals: &[3, 6, 10],
        quality: "ø7",
    },
    ChordTemplate {
        intervals: &[3, 6, 9],
        quality: "dim7",
    },
    ChordTemplate {
        intervals: &[4, 8, 10],
        quality: "7aug",
    },
    // ── Jazz Omissions (Common "no 5th" etc.) ────────────────────────────────
    ChordTemplate {
        intervals: &[4, 10, 21],
        quality: "13(no5,no9,no11)",
    },
    ChordTemplate {
        intervals: &[4, 10, 14, 21],
        quality: "13(no5,no11)",
    },
    ChordTemplate {
        intervals: &[4, 10, 14],
        quality: "9(no5)",
    },
    ChordTemplate {
        intervals: &[4, 10],
        quality: "7(no5)",
    },
    ChordTemplate {
        intervals: &[3, 10],
        quality: "m7(no5)",
    },
    // ── Added tones ──────────────────────────────────────────────────────────
    ChordTemplate {
        intervals: &[4, 7, 9],
        quality: "6",
    },
    ChordTemplate {
        intervals: &[3, 7, 9],
        quality: "m6",
    },
    ChordTemplate {
        intervals: &[3, 7, 14],
        quality: "madd9",
    },
    ChordTemplate {
        intervals: &[4, 7, 14],
        quality: "add9",
    },
    // ── Basic triads ─────────────────────────────────────────────────────────
    ChordTemplate {
        intervals: &[4, 7],
        quality: "",
    },
    ChordTemplate {
        intervals: &[3, 7],
        quality: "m",
    },
    ChordTemplate {
        intervals: &[3, 6],
        quality: "dim",
    },
    ChordTemplate {
        intervals: &[4, 8],
        quality: "aug",
    },
    ChordTemplate {
        intervals: &[2, 7],
        quality: "sus2",
    },
    ChordTemplate {
        intervals: &[5, 7],
        quality: "sus4",
    },
    ChordTemplate {
        intervals: &[7],
        quality: "5",
    },
    ChordTemplate {
        intervals: &[3],
        quality: "m(no5)",
    },
    ChordTemplate {
        intervals: &[4],
        quality: "(no5)",
    },
];

pub fn detect(active_notes: &[u8], scale_root: u8, allow_rootless: bool) -> ChordInfo {
    let note_count = active_notes.len();
    if note_count == 0 {
        return ChordInfo::default();
    }

    let mut sorted_notes = [0u8; 128];
    sorted_notes[..note_count].copy_from_slice(active_notes);
    sorted_notes[..note_count].sort_unstable();

    let mut pcs = [0u8; 12];
    let mut pcs_len = 0usize;
    let mut last_pc = None;
    for &note in &sorted_notes[..note_count] {
        let pc = note % 12;
        if last_pc != Some(pc) {
            pcs[pcs_len] = pc;
            pcs_len += 1;
            last_pc = Some(pc);
        }
    }

    if pcs_len == 1 {
        let pc = pcs[0];
        let mut active_midi = ActiveMidiNotes::default();
        for &note in &sorted_notes[..note_count] {
            active_midi.push(note, NoteRole::Root);
        }
        return ChordInfo {
            root_pc: Some(pc),
            bass_pc: Some(pc),
            active_midi,
            ..Default::default()
        };
    }
    let bass_midi = sorted_notes[0];
    let bass_pc = bass_midi % 12;
    let mut best: Option<(u8, &'static str, usize)> = None;
    let mut roots_to_check = [0u8; 14];
    let mut roots_len = pcs_len;
    roots_to_check[..pcs_len].copy_from_slice(&pcs[..pcs_len]);
    if allow_rootless {
        let t = scale_root % 12;
        let d = (scale_root + 7) % 12;
        if !roots_to_check[..roots_len].contains(&t) {
            roots_to_check[roots_len] = t;
            roots_len += 1;
        }
        if !roots_to_check[..roots_len].contains(&d) {
            roots_to_check[roots_len] = d;
            roots_len += 1;
        }
    }
    for &root_pc in &roots_to_check[..roots_len] {
        let root_midi = sorted_notes[..note_count]
            .iter()
            .find(|&&n| n % 12 == root_pc)
            .copied()
            .unwrap_or(root_pc);

        let mut intervals_pc = [false; 12];
        let mut compound = [false; 37];
        for &note in &sorted_notes[..note_count] {
            if note % 12 == root_pc {
                continue;
            }
            intervals_pc[((note % 12 + 12 - root_pc) % 12) as usize] = true;
            let interval = {
                let diff = note as i16 - root_midi as i16;
                if (0..=36).contains(&diff) {
                    diff as usize
                } else {
                    diff.rem_euclid(12) as usize
                }
            };
            compound[interval] = true;
        }
        'template: for tmpl in TEMPLATES {
            // Strict matching: Every interval in the template MUST be present
            for &ti in tmpl.intervals {
                let r = (ti % 12) as usize;
                if !compound[ti as usize] && !intervals_pc[r] {
                    continue 'template;
                }
            }
            // Strict match also ensures no OTHER pitch classes are present
            let mut covered = [false; 12];
            covered[root_pc as usize] = true;
            for &ti in tmpl.intervals {
                covered[((root_pc + ti) % 12) as usize] = true;
            }
            for &pc in &pcs[..pcs_len] {
                if !covered[pc as usize] {
                    continue 'template;
                }
            }

            let score = tmpl.intervals.len();
            let better = match &best {
                None => true,
                Some((br, _, bs)) => {
                    if score > *bs {
                        true
                    } else if score == *bs {
                        root_pc == bass_pc && *br != bass_pc
                    } else {
                        false
                    }
                }
            };
            if better {
                best = Some((root_pc, tmpl.quality, score));
            }
        }
    }
    match best {
        None => ChordInfo {
            bass_pc: Some(bass_pc),
            active_midi: {
                let mut active_midi = ActiveMidiNotes::default();
                for &note in &sorted_notes[..note_count] {
                    let role = if note == bass_midi {
                        NoteRole::Bass
                    } else {
                        NoteRole::Normal
                    };
                    active_midi.push(note, role);
                }
                active_midi
            },
            ..Default::default()
        },
        Some((r_pc, full_quality, _)) => {
            let (quality, omitted) = split_quality_and_omitted(full_quality);
            let first_r_midi = sorted_notes[..note_count]
                .iter()
                .find(|&&n| n % 12 == r_pc)
                .copied()
                .unwrap_or(bass_midi);
            let inversion = if bass_pc == r_pc {
                ""
            } else {
                let mut chord_tones = [false; 12];
                chord_tones[r_pc as usize] = true;
                if let Some(t) = TEMPLATES.iter().find(|t| t.quality == full_quality) {
                    for &ti in t.intervals {
                        chord_tones[((r_pc + ti) % 12) as usize] = true;
                    }
                }
                let mut covered_below_root = [false; 12];
                for &note in &sorted_notes[..note_count] {
                    if note < first_r_midi && chord_tones[(note % 12) as usize] {
                        covered_below_root[(note % 12) as usize] = true;
                    }
                }
                let bc = covered_below_root
                    .iter()
                    .filter(|&&covered| covered)
                    .count();
                let bass_rel = (bass_pc + 12 - r_pc) % 12;
                if bc > 0 {
                    match bass_rel {
                        3 | 4 => "1st inv.",
                        6..=8 => "2nd inv.",
                        9..=11 => "3rd inv.",
                        _ => "",
                    }
                } else {
                    ""
                }
            };
            ChordInfo {
                root_pc: Some(r_pc),
                quality,
                omitted,
                bass_pc: Some(bass_pc),
                inversion,
                active_midi: {
                    let mut active_midi = ActiveMidiNotes::default();
                    for &note in &sorted_notes[..note_count] {
                        let role = if note == bass_midi && bass_pc != r_pc {
                            NoteRole::Bass
                        } else if note % 12 == r_pc {
                            NoteRole::Root
                        } else {
                            NoteRole::Normal
                        };
                        active_midi.push(note, role);
                    }
                    active_midi
                },
            }
        }
    }
}
