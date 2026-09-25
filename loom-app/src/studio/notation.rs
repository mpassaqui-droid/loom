//! NOTATION: the roll read as sheet music — a grand staff with one note-head
//! per event, and a way to transpose ONE voice by rewriting `| semi N`.
//! Pure logic (no drawing), so both the GUI and later the web can use it.
//!
//! Staff degrees follow the standard: the five lines sit on EVEN degrees
//! 0..=8 (0 = bottom line, 8 = top line); a space is an odd degree (1..=7).
//! Sharps/flats sit on their lower neighbour (`c♯` on the C line), which is
//! enough to READ the shape — the tooltip shows the real note name.

use crate::studio::roll::Roll;
use crate::vibe::strip_comment;

/// One note-head on the grand staff.
#[derive(Clone, Debug, PartialEq)]
pub struct StaffNote {
    pub midi: i32,
    /// Start and length inside the bar, 0..1.
    pub start: f32,
    pub len: f32,
    pub voice: String,
    /// Colour, 0..360 (like the roll and the grid).
    pub hue: f32,
    /// Which staff: middle C and above = treble (true), below = bass (false).
    pub treble: bool,
    /// Staff degree: 0 = bottom line … 8 = top line.
    pub degree: i32,
    /// Ledger lines to draw above/below the staff.
    pub ledger: i32,
}

/// Degrees spanned by the note-heads, to lay out the staves.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StaffRange {
    pub low_degree: i32,
    pub high_degree: i32,
}

/// The note-heads of `roll` on the grand staff.
pub fn staff(roll: &Roll) -> (Vec<StaffNote>, StaffRange) {
    let mut notes: Vec<StaffNote> = Vec::new();
    for n in &roll.notes {
        let treble = n.midi >= 60;
        let degree = if treble { dia(n.midi) - dia(64) } else { dia(n.midi) - dia(43) };
        // in both staves, the five LINES sit on even 0..=8
        let ledger = if degree < 0 {
            let mut d = -2;
            let mut count = 0;
            while d >= degree {
                count += 1;
                d -= 2;
            }
            count
        } else if degree > 8 {
            let mut d = 10;
            let mut count = 0;
            while d <= degree {
                count += 1;
                d += 2;
            }
            count
        } else {
            0
        };
        notes.push(StaffNote { midi: n.midi, start: n.start, len: n.len, voice: n.voice.clone(), hue: n.hue, treble, degree, ledger });
    }
    let (mut lo, mut hi) = notes.iter().fold((999i32, -999i32), |(lo, hi), n| (lo.min(n.degree), hi.max(n.degree)));
    if notes.is_empty() {
        (lo, hi) = (0, 8);
    }
    (notes, StaffRange { low_degree: lo, high_degree: hi })
}

/// C-major staff position of a MIDI note, from octave −1 = 0 (i.e. `dia(60) == 28`).
/// A black-ish note sits on its white neighbour's position (worth mentioning
/// in the tooltip, not on the staff).
fn dia(midi: i32) -> i32 {
    // C=0 D=1 E=2 F=3 G=4 A=5 B=6, accidentals on the lower neighbour
    const POS: [i32; 12] = [0, 0, 1, 1, 2, 3, 3, 4, 4, 5, 5, 6];
    let (oct, semi) = (midi.div_euclid(12) - 1, midi.rem_euclid(12));
    POS[semi as usize] + 7 * oct
}

/// English note names for the tooltip (C♯, E♭, …).
pub const NOTE_NAMES: [&str; 12] = ["C", "C♯", "D", "E♭", "E", "F", "F♯", "G", "A♭", "A", "B♭", "B"];

/// `C4`-style name of a MIDI note (60 → `C4`).
pub fn midi_name(midi: i32) -> String {
    let oct = midi.div_euclid(12) - 1;
    format!("{}{}", NOTE_NAMES[midi.rem_euclid(12) as usize], oct)
}

/// Transpose ONE voice by `semitones`, rewriting its `| semi N` in place so
/// repeated presses accumulate; a line without one gets `| semi N` appended.
/// The rest of the line (pattern, fx, comment) is untouched and must re-parse.
pub fn transpose_voice(src: &str, voice: &str, semitones: i32) -> Result<String, String> {
    let want = voice.trim().to_lowercase();
    let mut found = false;
    let lines: Vec<String> = src
        .split('\n')
        .map(|line| {
            let code = strip_comment(line);
            let Some((head, _)) = code.split_once(':') else { return line.to_string() };
            // the full head: `bass` or the variant `bass@drop`
            let name = head.trim().to_lowercase();
            if name != want {
                return line.to_string();
            }
            found = true;
            let comment = &line[code.len()..];
            replace_semi(code, semitones, comment)
        })
        .collect();
    if !found {
        return Err(format!("aucune voix « {voice} » — vérifie son nom."));
    }
    Ok(lines.join("\n"))
}

/// Append `| semi N` or grow the existing `semi`/`transpose` verb (last one
/// wins in the parser, so this mirrors what you HEAR).
fn replace_semi(code: &str, delta: i32, comment: &str) -> String {
    let code = code.trim_end();
    let mut last: Option<(usize, usize)> = None;
    let mut pos = 0;
    while let Some(rel) = code[pos..].find('|') {
        let seg_start = pos + rel + 1;
        let seg_end = code[seg_start..].find('|').map(|e| seg_start + e).unwrap_or(code.len());
        if let Some(v) = code[seg_start..seg_end].trim_start().strip_prefix("semi ").or_else(|| code[seg_start..seg_end].trim_start().strip_prefix("transpose ")) {
            if v.trim().parse::<i32>().is_ok() {
                last = Some((seg_start, seg_end));
            }
        }
        pos = seg_end;
        if seg_end == code.len() {
            break;
        }
    }
    let mut out = match last {
        // grow the existing value: `semi N` -> `semi N+delta` (last one wins when heard)
        Some((s, e)) => {
            let cur = code[s..e].trim_start();
            let n = cur.strip_prefix("semi ").or_else(|| cur.strip_prefix("transpose ")).and_then(|v| v.trim().parse::<i32>().ok()).unwrap_or(0);
            format!("{} semi {}", &code[..s], n + delta) + &code[e..]
        }
        None => format!("{code} | semi {delta}"),
    };
    if !comment.is_empty() {
        out.push_str("  ");
        out.push_str(comment);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use loom_core::Program;

    /// The roll mape par le lib -- on re-parse un source simple.
    fn roll_of(src: &str) -> Roll {
        let mut r = crate::studio::roll::roll(&Program::parse(src), 0);
        r.notes.sort_by_key(|n| n.midi);
        r
    }

    #[test]
    fn c_major_lands_on_the_standard_staff() {
        let r = roll_of("lead: c4 e4 g4 a4");
        debug_assert_eq!(r.notes.iter().map(|n| n.midi).collect::<Vec<_>>(), [60, 64, 67, 69]);
        let (notes, range) = staff(&r);
        let deg: Vec<i32> = notes.iter().map(|n| n.degree).collect();
        assert_eq!(deg, [-2, 0, 2, 3], "C4 ledger below · E4 bottom line · G4 line · A4 space");
        assert!(notes.iter().all(|n| n.treble));
        assert_eq!(notes[0].ledger, 1, "middle C in treble = one ledger below");
        assert_eq!(notes[1].ledger, 0);
        assert_eq!(notes[2].ledger, 0);
        assert_eq!(notes[3].ledger, 0);
        assert!(range.low_degree <= -2 && range.high_degree >= 3);
        assert_eq!(midi_name(60), "C4");
        assert_eq!(midi_name(61), "C♯4");
        assert_eq!(midi_name(69), "A4");
        assert_eq!(midi_name(43), "G2");
    }

    #[test]
    fn bass_clef_holds_the_low_notes() {
        let r = roll_of("bass: g2 b2 d3");
        let (notes, _) = staff(&r);
        assert!(notes.iter().all(|n| !n.treble));
        assert_eq!(notes.iter().map(|n| n.degree).collect::<Vec<_>>(), [0, 2, 4]);
        assert!(notes.iter().all(|n| n.ledger == 0));
    }

    #[test]
    fn ledger_lines_count_like_a_book() {
        // above the treble staff: B5 (degree 11) needs 1 ledger, D6 (13) needs 2
        let r = roll_of("lead: g5 b5 d6");
        let (notes, _) = staff(&r);
        assert_eq!(notes.iter().map(|n| n.degree).collect::<Vec<_>>(), [9, 11, 13]);
        assert_eq!(notes.iter().map(|n| n.ledger).collect::<Vec<_>>(), [0, 1, 2]);
        // deep bass: C2 = 2 ledger lines below the staff
        let r = roll_of("bass: c2");
        let (notes, _) = staff(&r);
        assert_eq!(notes[0].degree, -4);
        assert_eq!(notes[0].ledger, 2);
    }

    #[test]
    fn transpose_grows_the_existing_semi_and_keeps_the_comment() {
        let src = "kick: x . x .\nlead: 1 3 5 1' | pulse | semi 2\nbass: 1 . 5, . # la basse\n";
        let t = transpose_voice(src, "lead", 3).unwrap();
        assert!(t.contains("lead: 1 3 5 1' | pulse | semi 5"), "{t}");
        let t2 = transpose_voice(&t, "bass", -2).unwrap();
        assert!(t2.contains("bass: 1 . 5, . | semi -2  # la basse"), "{t2}");
        // the song still plays after the rewrite
        assert_eq!(Program::parse(&t2).voices.len(), 3);
        // a `transpose` alias is grown too
        let alias = transpose_voice("mel: 1 2 3 5 | transpose 1", "mel", 2).unwrap();
        assert!(alias.contains("| semi 3"), "{alias}");
        // unknown voice: polite refusal
        assert!(transpose_voice(src, "nope", 1).unwrap_err().contains("aucune voix"));
        // a `@section` variant is transposed on its own line
        let var = "bass: 1 . 5, .\nbass@drop: 1 1 1 1 | semi 4\n";
        let v = transpose_voice(var, "bass@drop", -3).unwrap();
        assert!(v.contains("bass@drop: 1 1 1 1 | semi 1"), "{v}");
        assert!(v.contains("bass: 1 . 5, ."), "{v}");
    }

    #[test]
    fn empty_and_comment_only_lines_are_left_alone() {
        let src = "# titre\nbass: 1 5, 3 .\n# une remarque\n";
        let t = transpose_voice(src, "bass", 1).unwrap();
        let lines: Vec<&str> = t.lines().collect();
        assert_eq!(lines[0], "# titre");
        assert_eq!(lines[2], "# une remarque");
    }
}
