#[cfg(test)]
mod tests {
    use crate::chord::*;

    fn chord_str(notes: &[u8], scale_root: u8) -> String {
        let info = detect(notes, scale_root, false);
        format!("{}{}{}{}", info.root, info.quality, info.omitted, info.slash)
    }

    #[test]
    fn test_basic_triads() {
        assert_eq!(chord_str(&[60, 64, 67], 0), "C");     // C Maj
        assert_eq!(chord_str(&[57, 60, 64], 0), "Am");    // A Min
        assert_eq!(chord_str(&[59, 62, 65], 0), "Bdim");  // B Dim
        assert_eq!(chord_str(&[60, 65, 67], 0), "Csus4"); // C Sus4
    }

    #[test]
    fn test_sevenths() {
        assert_eq!(chord_str(&[55, 59, 62, 65], 0), "G7");     // G7
        assert_eq!(chord_str(&[60, 64, 67, 71], 0), "Cmaj7");  // Cmaj7
        assert_eq!(chord_str(&[59, 62, 65, 68], 5), "Bdim7");  // Bdim7 in F major
    }

    #[test]
    fn test_ninth_chords() {
        assert_eq!(chord_str(&[48, 52, 55, 58, 62], 0), "C9");      // C9
        assert_eq!(chord_str(&[48, 52, 55, 59, 62], 0), "Cmaj9");   // Cmaj9
        assert_eq!(chord_str(&[48, 51, 55, 58, 62], 0), "Cm9");     // Cm9
        assert_eq!(chord_str(&[48, 52, 55, 58, 61], 0), "C7b9");    // C7b9
    }

    #[test]
    fn test_extended_jazz_chords() {
        // C11
        assert_eq!(chord_str(&[48, 52, 55, 58, 62, 65], 0), "C11");
        // C13
        assert_eq!(chord_str(&[48, 52, 55, 58, 62, 65, 69], 0), "C13");
        // Cmaj7#11
        assert_eq!(chord_str(&[60, 64, 67, 71, 78], 0), "Cmaj7#11");
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
}
