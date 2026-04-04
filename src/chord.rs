//! # chord.rs — Chord Recognition Engine
//!
//! Pure interval-math chord detector.  No external music–theory crates are
//! used; every available crate either targets audio-signal analysis (not MIDI
//! note lists), focuses on *generation* rather than *detection*, or returns
//! 404.  Keeping it internal gives us full control and zero compile overhead.
//!
//! ## Approach
//! 1. Collect the set of active MIDI note numbers.
//! 2. Find the **lowest** note (bass note).
//! 3. Build a **pitch-class set**: reduce every note modulo 12, deduplicate,
//!    sort.  This removes octave duplicates and makes the math octave-agnostic.
//! 4. For each pitch class as potential root, compute *intervals above the
//!    root* (0–11 semitones) and match against a look-up table of known chord
//!    qualities.
//! 5. Choose the best match (longest that still covers all pitch classes), then
//!    report the actual root, quality string, inversion hint, and slash bass
//!    note if the bass ≠ root.

use std::fmt;

// ─── Public types ────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Default)]
pub enum NoteRole {
    #[default]
    Normal,
    Root,
    Bass,
}

/// Everything the GUI needs to display.
#[derive(Clone, Debug, Default)]
pub struct ChordInfo {
    /// The root note name, e.g. `"C"`, `"F#"`. `"–"` when empty.
    pub root: String,
    /// The quality suffix, e.g. `"maj7"`, `"m"`.
    pub quality: String,
    /// Omitted notes suffix, e.g. `"(no5)"`.
    pub omitted: String,
    /// Optional slash suffix, e.g. `"/G"`.  Empty when bass == root.
    pub slash: String,
    /// Inversion description: `""`, `"1st inv."`, `"2nd inv."`, `"3rd inv."`.
    pub inversion: String,
    /// Names and roles of every currently active note.
    pub active_notes: Vec<(String, NoteRole)>,
}

impl fmt::Display for ChordInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.root == "–" {
            write!(f, "–")
        } else {
            write!(f, "{}{}{}{}", self.root, self.quality, self.omitted, self.slash)
        }
    }
}

// ─── Note naming ─────────────────────────────────────────────────────────────

const NOTE_NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

/// Convert a MIDI note number to a name like `"C4"`, `"F#3"`.
pub fn midi_to_name(note: u8) -> String {
    let pc = (note % 12) as usize;
    let octave = (note / 12) as i32 - 1; // MIDI 60 = C4
    format!("{}{}", NOTE_NAMES[pc], octave)
}

fn pc_name(pc: u8) -> &'static str {
    NOTE_NAMES[(pc % 12) as usize]
}

// ─── Chord quality table ──────────────────────────────────────────────────────
//
// Each entry is (interval set, quality suffix).
// Intervals are semitones above the root, *excluding* 0.
// Ordering: more specific (more intervals) entries come *first* so the matcher
// can greedily prefer the richest match.

struct ChordTemplate {
    /// Semitone intervals above the root (root=0 is implicit, not listed).
    intervals: &'static [u8],
    /// String suffix appended to the root name.
    quality: &'static str,
}

/// Build the table once as a `const`-able slice.
static TEMPLATES: &[ChordTemplate] = &[
    // ── 13th chords ──────────────────────────────────────────────────────────
    // 13th with 11th (theoretically complete)
    ChordTemplate { intervals: &[4, 7, 10, 14, 17, 21], quality: "13" },
    ChordTemplate { intervals: &[3, 7, 10, 14, 17, 21], quality: "m13" },
    ChordTemplate { intervals: &[4, 7, 11, 14, 17, 21], quality: "maj13" },
    // Standard 13th (11th omitted)
    ChordTemplate { intervals: &[4, 7, 10, 14, 21], quality: "13(no11)" },
    ChordTemplate { intervals: &[3, 7, 10, 14, 21], quality: "m13(no11)" },
    ChordTemplate { intervals: &[4, 7, 11, 14, 21], quality: "maj13(no11)" },
    // 13th omitting 5th and 11th
    ChordTemplate { intervals: &[4, 10, 14, 21], quality: "13(no5,no11)" },
    ChordTemplate { intervals: &[3, 10, 14, 21], quality: "m13(no5,no11)" },
    ChordTemplate { intervals: &[4, 11, 14, 21], quality: "maj13(no5,no11)" },
    // 13th omitting 5th, 9th AND 11th (guitar shell voicing)
    ChordTemplate { intervals: &[4, 10, 21], quality: "13(no5,no9,no11)" },
    ChordTemplate { intervals: &[3, 10, 21], quality: "m13(no5,no9,no11)" },
    ChordTemplate { intervals: &[4, 11, 21], quality: "maj13(no5,no9,no11)" },
    // ── 11th chords ──────────────────────────────────────────────────────────
    ChordTemplate { intervals: &[4, 7, 10, 14, 17], quality: "11" },
    ChordTemplate { intervals: &[3, 7, 10, 14, 17], quality: "m11" },
    ChordTemplate { intervals: &[4, 7, 11, 14, 17], quality: "maj11" },
    ChordTemplate { intervals: &[4, 7, 10, 17], quality: "7add11" },
    // ── 9th chords ───────────────────────────────────────────────────────────
    ChordTemplate { intervals: &[4, 7, 10, 14], quality: "9" },
    ChordTemplate { intervals: &[3, 7, 10, 14], quality: "m9" },
    ChordTemplate { intervals: &[4, 7, 11, 14], quality: "maj9" },
    ChordTemplate { intervals: &[4, 10, 14], quality: "9(no5)" },
    ChordTemplate { intervals: &[3, 10, 14], quality: "m9(no5)" },
    ChordTemplate { intervals: &[4, 11, 14], quality: "maj9(no5)" },
    ChordTemplate { intervals: &[4, 7, 10, 13], quality: "7b9" },
    ChordTemplate { intervals: &[4, 7, 10, 15], quality: "7#9" },
    ChordTemplate { intervals: &[4, 7, 14], quality: "add9" },
    ChordTemplate { intervals: &[3, 7, 14], quality: "madd9" },
    // ── 7th chords ───────────────────────────────────────────────────────────
    ChordTemplate { intervals: &[4, 7, 10], quality: "7" },       // dominant 7
    ChordTemplate { intervals: &[4, 7, 11], quality: "maj7" },
    ChordTemplate { intervals: &[3, 7, 10], quality: "m7" },
    ChordTemplate { intervals: &[3, 7, 11], quality: "mMaj7" },
    ChordTemplate { intervals: &[4, 10], quality: "7(no5)" },
    ChordTemplate { intervals: &[4, 11], quality: "maj7(no5)" },
    ChordTemplate { intervals: &[3, 10], quality: "m7(no5)" },
    ChordTemplate { intervals: &[3, 6, 10], quality: "ø7" },      // half-diminished / m7b5
    ChordTemplate { intervals: &[3, 6, 9],  quality: "dim7" },    // fully diminished
    ChordTemplate { intervals: &[4, 8, 10], quality: "aug7" },
    ChordTemplate { intervals: &[4, 8, 11], quality: "augMaj7" },
    // ── Suspended + dominant ─────────────────────────────────────────────────
    ChordTemplate { intervals: &[2, 7],     quality: "sus2" },
    ChordTemplate { intervals: &[5, 7],     quality: "sus4" },
    ChordTemplate { intervals: &[5, 7, 10], quality: "7sus4" },
    ChordTemplate { intervals: &[5, 7, 11], quality: "maj7sus4" },
    // ── Basic triads ─────────────────────────────────────────────────────────
    ChordTemplate { intervals: &[4, 7],     quality: "" },         // major
    ChordTemplate { intervals: &[3, 7],     quality: "m" },
    ChordTemplate { intervals: &[3, 6],     quality: "dim" },
    ChordTemplate { intervals: &[4, 8],     quality: "aug" },
    // ── Added tones (triads + colour) ────────────────────────────────────────
    ChordTemplate { intervals: &[4, 7, 9],  quality: "6" },
    ChordTemplate { intervals: &[3, 7, 9],  quality: "m6" },
    ChordTemplate { intervals: &[4, 7, 6],  quality: "add#11" },   // Lydian colour
    // ── Power chord / dyads ──────────────────────────────────────────────────
    ChordTemplate { intervals: &[7],        quality: "5" },
    ChordTemplate { intervals: &[3],        quality: "(min 3rd)" },
    ChordTemplate { intervals: &[4],        quality: "(maj 3rd)" },
];

// ─── Detection ───────────────────────────────────────────────────────────────

/// Detect the chord from a set of active MIDI note numbers.
///
/// `active_notes` can be in any order; the function sorts internally.
pub fn detect(active_notes: &[u8]) -> ChordInfo {
    // Build the display list first (we need raw note data before deduplication)
    let mut sorted_notes: Vec<u8> = active_notes.to_vec();
    sorted_notes.sort_unstable();
    sorted_notes.dedup();

    if sorted_notes.len() < 2 {
        // Single note or empty – just show the note name or silence marker
        let active_notes = sorted_notes
            .iter()
            .map(|&n| (midi_to_name(n), NoteRole::Root))
            .collect();
            
        return ChordInfo {
            root: if sorted_notes.is_empty() {
                "–".to_string()
            } else {
                midi_to_name(sorted_notes[0])
            },
            quality: String::new(),
            omitted: String::new(),
            slash: String::new(),
            inversion: String::new(),
            active_notes,
        };
    }

    // Bass note = lowest sounding MIDI number
    let bass_midi = sorted_notes[0];
    let bass_pc = bass_midi % 12; // pitch class 0–11

    // Pitch-class set (mod-12, deduplicated, sorted)
    let mut pcs: Vec<u8> = sorted_notes.iter().map(|n| n % 12).collect();
    pcs.sort_unstable();
    pcs.dedup();

    // Try every pitch class as a potential root candidate.
    // We pick the candidate that:
    //   (a) matches the most intervals without *missing* any active note, AND
    //   (b) as a tie-breaker, is a more common root ordering.
    let mut best: Option<(u8, &'static str, usize)> = None; // (root_pc, quality, interval_count)

    for &root_pc in &pcs {
        // Build an interval set relative to `root_pc`, wrapping at 12
        // (octave-reduced), then look for templates that are a subset of what
        // we have AND account for every pitch class present.
        let intervals: Vec<u8> = pcs
            .iter()
            .filter(|&&pc| pc != root_pc)
            .map(|&pc| (pc + 12 - root_pc) % 12)
            .collect();

        // Extend intervals to handle 9ths/11ths/13ths by also considering the
        // +12/-12 compound forms when the same pitch class appears an octave up
        // in the sounding notes (i.e., we do have the note somewhere).
        // Strategy: for every interval i in [0..12], if (i+12) or (i+24) makes
        // sense as a chord extension, include it.  We compute compound intervals
        // from the raw sorted_notes compared to the first occurrence of root_pc.
        let root_midi = sorted_notes
            .iter()
            .copied()
            .find(|&n| n % 12 == root_pc)
            .unwrap_or(root_pc);

        let compound_intervals: Vec<u8> = sorted_notes
            .iter()
            .filter(|&&n| n % 12 != root_pc) // exclude root note(s)
            .filter_map(|&n| {
                let diff = n as i16 - root_midi as i16;
                if diff >= 0 && diff <= 24 {
                    Some(diff as u8)
                } else if diff < 0 {
                    // note is below root in MIDI space – use octave-reduced form
                    Some((diff.rem_euclid(12)) as u8)
                } else {
                    None
                }
            })
            .collect();

        // Merge simple + compound intervals – a template matches if ALL its
        // listed intervals appear in either set, AND the union of template
        // intervals (mod 12) covers all pitch classes.
        'template: for tmpl in TEMPLATES {
            // Check that every template interval is present (in either set)
            for &ti in tmpl.intervals {
                let ti_reduced = ti % 12;
                let found = compound_intervals.contains(&ti)
                    || intervals.contains(&ti_reduced)
                    || (ti > 12 && intervals.contains(&(ti - 12)));
                if !found {
                    continue 'template;
                }
            }

            // Check that every active pitch class is accounted for by the
            // template (root + template intervals mod 12).
            let mut covered: Vec<u8> = vec![root_pc];
            for &ti in tmpl.intervals {
                covered.push((root_pc + ti) % 12);
            }
            covered.sort_unstable();
            covered.dedup();
            for &pc in &pcs {
                if !covered.contains(&pc) {
                    continue 'template;
                }
            }

            // Valid match – prefer the longest template (richest chord)
            let score = tmpl.intervals.len();
            let better = match &best {
                None => true,
                Some((best_root, _, best_score)) => {
                    if score > *best_score {
                        true
                    } else if score == *best_score {
                        // Tie-breaker: if scores are equal (e.g. symmetrical dim7), 
                        // prefer the one where the bass is the root.
                        root_pc == bass_pc && *best_root != bass_pc
                    } else {
                        false
                    }
                }
            };
            if better {
                best = Some((root_pc, tmpl.quality, score));
            }
            // We keep searching other roots but stop testing more templates for
            // *this* root once the first (richest preferable) one matched.
            break;
        }
    }

    // Build the final ChordInfo
    match best {
        None => {
            // Unrecognised combination
            let active_notes = sorted_notes
                .iter()
                .map(|&n| {
                    (
                        midi_to_name(n),
                        if n == bass_midi { NoteRole::Bass } else { NoteRole::Normal }
                    )
                })
                .collect();
                
            ChordInfo {
                root: format!("? ({})", pcs.iter().map(|&p| pc_name(p)).collect::<Vec<_>>().join(" ")),
                quality: String::new(),
                omitted: String::new(),
                slash: String::new(),
                inversion: String::new(),
                active_notes,
            }
        }
        Some((root_pc, quality, _)) => {
            // Root name
            let root_name = pc_name(root_pc);
            
            let mut qual_str = quality.to_string();
            let mut omitted = String::new();
            if let Some(idx) = qual_str.find("(no") {
                omitted = qual_str[idx..].to_string();
                qual_str.truncate(idx);
            }

            // Slash chord: bass note differs from root
            let slash = if bass_pc != root_pc {
                format!("/{}", pc_name(bass_pc))
            } else {
                String::new()
            };

            // Inversion calculation
            let inversion = if slash.is_empty() {
                String::new()
            } else {
                let first_root_midi = sorted_notes
                    .iter()
                    .find(|&&n| n % 12 == root_pc)
                    .copied()
                    .unwrap_or(bass_midi);

                let template_pcs: Vec<u8> = {
                    let tmpl_intervals = TEMPLATES
                        .iter()
                        .find(|t| t.quality == quality)
                        .map(|t| t.intervals)
                        .unwrap_or(&[]);
                    let mut v: Vec<u8> = vec![root_pc];
                    for &ti in tmpl_intervals {
                        v.push((root_pc + ti) % 12);
                    }
                    v.sort_unstable();
                    v.dedup();
                    v
                };

                let below_count = sorted_notes
                    .iter()
                    .filter(|&&n| n < first_root_midi && template_pcs.contains(&(n % 12)))
                    .map(|&n| n % 12)
                    .collect::<std::collections::HashSet<_>>()
                    .len();

                match below_count {
                    1 => "1st inv.".to_string(),
                    2 => "2nd inv.".to_string(),
                    3 => "3rd inv.".to_string(),
                    _ => String::new(),
                }
            };

            let active_notes = sorted_notes
                .iter()
                .map(|&n| {
                    let role = if n == bass_midi && bass_pc != root_pc {
                        NoteRole::Bass
                    } else if n % 12 == root_pc {
                        NoteRole::Root
                    } else {
                        NoteRole::Normal
                    };
                    (midi_to_name(n), role)
                })
                .collect();

            ChordInfo { root: root_name.to_string(), quality: qual_str, omitted, slash, inversion, active_notes }
        }
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn chord_str(notes: &[u8]) -> String {
        let info = detect(notes);
        format!("{}{}{}{}", info.root, info.quality, info.omitted, info.slash)
    }

    #[test]
    fn test_cmajor() {
        // C4=60  E4=64  G4=67
        assert_eq!(chord_str(&[60, 64, 67]), "C");
    }

    #[test]
    fn test_aminor() {
        // A3=57  C4=60  E4=64
        assert_eq!(chord_str(&[57, 60, 64]), "Am");
    }

    #[test]
    fn test_g7() {
        // G3=55  B3=59  D4=62  F4=65
        assert_eq!(chord_str(&[55, 59, 62, 65]), "G7");
    }

    #[test]
    fn test_cmaj7() {
        // C4=60  E4=64  G4=67  B4=71
        assert_eq!(chord_str(&[60, 64, 67, 71]), "Cmaj7");
    }

    #[test]
    fn test_slash_chord() {
        // C/G: G2=43  C4=60  E4=64  G4=67
        let info = detect(&[43, 60, 64, 67]);
        assert_eq!(info.root, "C");
        assert_eq!(info.quality, "");
        assert_eq!(info.slash, "/G");
    }

    #[test]
    fn test_inversion() {
        // C/E (1st inversion): E3=52  C4=60  G4=67
        let info = detect(&[52, 60, 67]);
        assert_eq!(info.root, "C");
        assert_eq!(info.quality, "");
        assert_eq!(info.slash, "/E");
        assert_eq!(info.inversion, "1st inv.");
    }

    #[test]
    fn test_dim7() {
        // Bdim7: B3=59  D4=62  F4=65  Ab4=68
        assert_eq!(chord_str(&[59, 62, 65, 68]), "Bdim7");
    }

    #[test]
    fn test_sus4() {
        // Csus4: C4=60  F4=65  G4=67
        assert_eq!(chord_str(&[60, 65, 67]), "Csus4");
    }

    #[test]
    fn test_ninth() {
        // C9: C3=48  E3=52  G3=55  Bb3=58  D4=62
        assert_eq!(chord_str(&[48, 52, 55, 58, 62]), "C9");
        
        // Cmaj9: C3=48  E3=52  G3=55  B3=59  D4=62
        assert_eq!(chord_str(&[48, 52, 55, 59, 62]), "Cmaj9");
    }

    #[test]
    fn test_eleventh() {
        // C11: C3=48 E3=52 G3=55 Bb3=58 D4=62 F4=65
        assert_eq!(chord_str(&[48, 52, 55, 58, 62, 65]), "C11");
        
        // Cm11: C3=48 Eb3=51 G3=55 Bb3=58 D4=62 F4=65
        assert_eq!(chord_str(&[48, 51, 55, 58, 62, 65]), "Cm11");
    }

    #[test]
    fn test_thirteenth() {
        // C13 (with 11): C3=48 E3=52 G3=55 Bb3=58 D4=62 F4=65 A4=69
        assert_eq!(chord_str(&[48, 52, 55, 58, 62, 65, 69]), "C13");

        // C13 no 11
        assert_eq!(chord_str(&[48, 52, 55, 58, 62, 69]), "C13(no11)");
    }

    #[test]
    fn test_added_tones() {
        // C6: C4=60 E4=64 G4=67 A4=69
        assert_eq!(chord_str(&[60, 64, 67, 69]), "C6");
        
        // Cadd9: C4=60 E4=64 G4=67 D5=74
        assert_eq!(chord_str(&[60, 64, 67, 74]), "Cadd9");
    }
}
