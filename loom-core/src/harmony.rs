//! Roman-numeral harmony the way a theory class writes it.
//!
//! Plain numerals (`I ii V7`) stay diatonic: the chord is stacked from the
//! key's scale, so they work in any mode. On top of that:
//! - case against the scale = mode mixture: `iv` in a major key is the minor
//!   iv (borrowed), `V` in a minor key is the major dominant;
//! - `b` / `#` before the numeral: chromatic root, measured from the major
//!   scale (`bVII`, `bVI`, `♯iv°`), quality from the case;
//! - quality marks: `°` (or `o`) diminished, `ø` half-diminished, `+` augmented,
//!   `maj7`;
//! - figures: `6` `64` (triad inversions), `7` `65` `43` `42` (seventh chords);
//! - secondary chords: `V7/V`, `vii°7/ii` (the numeral read in the key of the target);
//! - special chords: `N` / `N6` (Neapolitan), `It6`, `Fr6`, `Ger6` (augmented sixths).

use crate::Key;

const MAJOR: [i32; 7] = [0, 2, 4, 5, 7, 9, 11];
const NATURAL_MINOR: [f64; 7] = [0.0, 2.0, 3.0, 5.0, 7.0, 8.0, 10.0];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Quality { Major, Minor, Dim, Aug }

#[derive(Debug, PartialEq)]
struct Numeral {
    accidental: i32,
    degree: i32,
    upper: bool,
    quality: Option<Quality>,
    half_dim: bool,
    maj7: bool,
    seventh: bool,
    inversion: usize,
}

fn numeral_degree(s: &str) -> Option<(i32, bool)> {
    let d = match s.to_ascii_lowercase().as_str() {
        "i" => 1, "ii" => 2, "iii" => 3, "iv" => 4, "v" => 5, "vi" => 6, "vii" => 7,
        _ => return None,
    };
    let upper = s.chars().all(|c| c.is_ascii_uppercase());
    let lower = s.chars().all(|c| c.is_ascii_lowercase());
    (upper || lower).then_some((d, upper))
}

fn parse_numeral(tok: &str) -> Option<Numeral> {
    let mut rest = tok;
    // `♯` / `♭` too: on a line, ` #` starts a comment, so `#iv°` must be written `♯iv°`
    let first = rest.chars().next()?;
    let numeral_next = rest.chars().nth(1).is_some_and(|c| matches!(c, 'I' | 'V' | 'i' | 'v'));
    let accidental = match first {
        'b' | '♭' if numeral_next => { rest = &rest[first.len_utf8()..]; -1 }
        '#' | '♯' if numeral_next => { rest = &rest[first.len_utf8()..]; 1 }
        _ => 0,
    };
    let n = rest.chars().take_while(|c| matches!(c, 'I' | 'V' | 'i' | 'v')).count();
    let (degree, upper) = numeral_degree(&rest[..n])?;
    rest = &rest[n..];
    let mut quality = None;
    let mut half_dim = false;
    let mut maj7 = false;
    if let Some(r) = rest.strip_prefix("maj7") { maj7 = true; rest = r; }
    else if let Some(r) = rest.strip_prefix('°').or_else(|| rest.strip_prefix('o')).or_else(|| rest.strip_prefix("dim")) { quality = Some(Quality::Dim); rest = r; }
    else if let Some(r) = rest.strip_prefix('ø') { quality = Some(Quality::Dim); half_dim = true; rest = r; }
    else if let Some(r) = rest.strip_prefix('+').or_else(|| rest.strip_prefix("aug")) { quality = Some(Quality::Aug); rest = r; }
    let (seventh, inversion) = match rest {
        "" => (maj7 || half_dim, 0),
        "7" => (true, 0),
        "6" => (maj7, 1),
        "64" => (false, 2),
        "65" => (true, 1),
        "43" => (true, 2),
        "42" | "2" => (true, 3),
        _ => return None,
    };
    Some(Numeral { accidental, degree, upper, quality, half_dim, maj7, seventh, inversion })
}

/// Is `tok` written in this extended roman grammar (anything beyond plain `I`..`VII` `7`)?
pub fn is_roman(tok: &str) -> bool {
    let base = tok.split('/').next().unwrap_or(tok);
    matches!(base, "N" | "N6" | "It6" | "Fr6" | "Ger6") || parse_numeral(base).is_some()
        && tok.split('/').nth(1).is_none_or(|t| parse_numeral(t).is_some())
}

fn pc(key: &Key, semis: f64) -> i32 { key.tonic + semis.round() as i32 }

/// The chord of a roman token in `key`, as MIDI notes from the key's octave.
pub fn roman_chord(tok: &str, key: &Key) -> Option<Vec<i32>> {
    let base_midi = 12 * (key.octave + 1);
    // secondary chords: read the numeral in the key of its target
    if let Some((num, target)) = tok.split_once('/') {
        let t = parse_numeral(target)?;
        let root = target_root(&t, key);
        let scale: Vec<f64> = if t.upper { MAJOR.iter().map(|x| *x as f64).collect() } else { NATURAL_MINOR.to_vec() };
        let sub = Key { tonic: root.rem_euclid(12), scale, octave: key.octave };
        // V/ii sounds near the key, not an octave up
        let mut notes = roman_chord(num, &sub)?;
        if notes.first().is_some_and(|n| *n - base_midi >= 12) { for n in &mut notes { *n -= 12; } }
        return Some(notes);
    }
    let tonic = base_midi + key.tonic;
    match tok {
        // Neapolitan: major triad on the lowered 2nd, usually in first inversion
        "N" => return Some(vec![tonic + 1, tonic + 5, tonic + 8]),
        "N6" => return Some(vec![tonic + 5, tonic + 8, tonic + 13]),
        // augmented sixths over the lowered 6th
        "It6" => return Some(vec![tonic - 4, tonic, tonic + 6]),
        "Fr6" => return Some(vec![tonic - 4, tonic, tonic + 2, tonic + 6]),
        "Ger6" => return Some(vec![tonic - 4, tonic, tonic + 3, tonic + 6]),
        _ => {}
    }
    let n = parse_numeral(tok)?;
    let len = key.scale.len().max(1) as i32;
    let scale_at = |idx: i32| -> i32 {
        let w = idx.div_euclid(len);
        12 * (key.octave + w + 1) + pc(key, key.scale[idx.rem_euclid(len) as usize])
    };
    let chromatic = n.accidental != 0 || len != 7;
    let mut notes: Vec<i32> = if chromatic || n.quality.is_some() {
        // root from the major scale (+ accidental), triad from the quality or the case
        let root = tonic + MAJOR[(n.degree - 1) as usize] + n.accidental;
        let q = n.quality.unwrap_or(if n.upper { Quality::Major } else { Quality::Minor });
        let (third, fifth) = match q { Quality::Major => (4, 7), Quality::Minor => (3, 7), Quality::Dim => (3, 6), Quality::Aug => (4, 8) };
        let mut v = vec![root, root + third, root + fifth];
        if n.seventh {
            v.push(root + match (q, n.half_dim, n.maj7) {
                (_, _, true) => 11,
                (Quality::Dim, false, _) => 9,
                (Quality::Dim, true, _) | (Quality::Minor, ..) | (Quality::Major, ..) => 10,
                (Quality::Aug, ..) => 10,
            });
        }
        v
    } else {
        // diatonic: stacked thirds of the scale, then mode mixture if the case disagrees
        let i0 = n.degree - 1;
        let mut v = vec![scale_at(i0), scale_at(i0 + 2), scale_at(i0 + 4)];
        let third = v[1] - v[0];
        if n.upper && third == 3 { v[1] += 1; if v[2] - v[0] == 6 { v[2] += 1; } }
        if !n.upper && third == 4 { v[1] -= 1; if v[2] - v[0] == 8 { v[2] -= 1; } }
        if n.seventh { v.push(if n.maj7 { v[0] + 11 } else { scale_at(i0 + 6) }); }
        v
    };
    for _ in 0..n.inversion.min(notes.len() - 1) {
        let low = notes.remove(0);
        notes.push(low + 12);
    }
    Some(notes)
}

/// Root pitch class of a numeral (for secondary targets).
fn target_root(t: &Numeral, key: &Key) -> i32 {
    if t.accidental != 0 || key.scale.len() != 7 {
        key.tonic + MAJOR[(t.degree - 1) as usize] + t.accidental
    } else {
        pc(key, key.scale[(t.degree - 1) as usize])
    }
}

/// Name the function of a chord (root pitch class + quality) in `key`:
/// diatonic numeral, borrowed chord, secondary dominant, Neapolitan.
/// `seventh` is the interval of the chord's seventh above the root, if any.
pub fn analyze_chord(root: i32, minor: bool, dim: bool, seventh: Option<i32>, key: &Key) -> String {
    let rel = (root - key.tonic).rem_euclid(12);
    const RN: [&str; 7] = ["I", "II", "III", "IV", "V", "VI", "VII"];
    let case = |s: &str| if minor || dim { s.to_lowercase() } else { s.to_string() };
    let suffix = match (dim, seventh) {
        (true, Some(9)) => "°7".to_string(),
        (true, Some(10)) => "ø7".to_string(),
        (true, _) => "°".to_string(),
        (false, Some(11)) => "maj7".to_string(),
        (false, Some(_)) => "7".to_string(),
        _ => String::new(),
    };
    // diatonic in the key's own scale, with the matching quality?
    let scale: Vec<i32> = key.scale.iter().map(|x| x.round() as i32).collect();
    if scale.len() == 7 {
        if let Some(d) = scale.iter().position(|s| *s == rel) {
            let third = (scale[(d + 2) % 7] - scale[d]).rem_euclid(12);
            let fifth = (scale[(d + 4) % 7] - scale[d]).rem_euclid(12);
            let q_minor = third == 3;
            let q_dim = third == 3 && fifth == 6;
            if q_minor == (minor || dim) && q_dim == dim {
                return format!("{}{suffix}", case(RN[d]));
            }
        }
    }
    // secondary dominant: a major (dominant 7th) chord a fifth above a diatonic degree
    if !minor && !dim && seventh != Some(11) {
        let target = (rel + 5).rem_euclid(12);
        if target != 0 {
            if let Some(d) = scale.iter().position(|s| *s == target) {
                let third = (scale[(d + 2) % 7] - scale[d]).rem_euclid(12);
                let t = if third == 3 { RN[d].to_lowercase() } else { RN[d].to_string() };
                return format!("V{suffix}/{t}");
            }
        }
    }
    // secondary leading-tone chord: diminished, a half step under a diatonic degree
    if dim {
        let target = (rel + 1).rem_euclid(12);
        if target != 0 {
            if let Some(d) = scale.iter().position(|s| *s == target) {
                let third = (scale[(d + 2) % 7] - scale[d]).rem_euclid(12);
                let t = if third == 3 { RN[d].to_lowercase() } else { RN[d].to_string() };
                return format!("vii{suffix}/{t}");
            }
        }
    }
    if rel == 1 && !minor && !dim { return "N".into(); }
    // borrowed / chromatic: measured from the major scale
    let (num, acc) = match MAJOR.iter().position(|m| *m == rel) {
        Some(d) => (RN[d], ""),
        None => match MAJOR.iter().position(|m| *m == rel + 1) { Some(d) => (RN[d], "b"), None => ("?", "") },
    };
    format!("{acc}{}{suffix}", case(num))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(src: &str) -> Key { crate::Program::parse(&format!("key: {src}\nlead: c4")).key.unwrap() }
    fn pcs(v: &[i32]) -> Vec<i32> { v.iter().map(|n| n.rem_euclid(12)).collect() }

    #[test]
    fn diatonique_inchange() {
        let c = key("c major");
        assert_eq!(pcs(&roman_chord("I", &c).unwrap()), vec![0, 4, 7]);
        assert_eq!(pcs(&roman_chord("ii", &c).unwrap()), vec![2, 5, 9]);
        assert_eq!(pcs(&roman_chord("V7", &c).unwrap()), vec![7, 11, 2, 5]);
        assert_eq!(pcs(&roman_chord("vii", &c).unwrap()), vec![11, 2, 5], "vii dim stays");
        let a = key("a minor");
        assert_eq!(pcs(&roman_chord("i", &a).unwrap()), vec![9, 0, 4]);
        assert_eq!(pcs(&roman_chord("VI", &a).unwrap()), vec![5, 9, 0]);
    }

    #[test]
    fn sensible_secondaire() {
        let c = key("c major");
        assert_eq!(analyze_chord(1, false, true, Some(9), &c), "vii°7/ii");
        assert_eq!(analyze_chord(6, false, true, Some(10), &c), "viiø7/V");
    }

    #[test]
    fn diese_unicode_car_diese_ascii_ouvre_un_commentaire() {
        let c = key("c major");
        assert_eq!(pcs(&roman_chord("♯iv°", &c).unwrap()), vec![6, 9, 0]);
        assert_eq!(roman_chord("♯iv°", &c), roman_chord("#iv°", &c));
        assert_eq!(pcs(&roman_chord("♭VI", &c).unwrap()), vec![8, 0, 3]);
        // sur une ligne, `♯iv°` joue ; ` #iv°` serait un commentaire
        let p = crate::Program::parse("key: c major\npad: I ♯iv° V");
        assert_eq!(p.voices[0].tokens, vec!["I", "♯iv°", "V"]);
    }

    #[test]
    fn emprunts_et_mixture() {
        let c = key("c major");
        assert_eq!(pcs(&roman_chord("iv", &c).unwrap()), vec![5, 8, 0], "iv borrowed from minor");
        assert_eq!(pcs(&roman_chord("bVII", &c).unwrap()), vec![10, 2, 5]);
        assert_eq!(pcs(&roman_chord("bVI", &c).unwrap()), vec![8, 0, 3]);
        let a = key("a minor");
        assert_eq!(pcs(&roman_chord("V", &a).unwrap()), vec![4, 8, 11], "major dominant in minor");
        assert_eq!(pcs(&roman_chord("V7", &a).unwrap()), vec![4, 8, 11, 2]);
    }

    #[test]
    fn dominantes_secondaires_et_qualites() {
        let c = key("c major");
        assert_eq!(pcs(&roman_chord("V7/V", &c).unwrap()), vec![2, 6, 9, 0]);
        assert_eq!(pcs(&roman_chord("V/ii", &c).unwrap()), vec![9, 1, 4]);
        assert_eq!(pcs(&roman_chord("vii°7/V", &c).unwrap()), vec![6, 9, 0, 3]);
        assert_eq!(pcs(&roman_chord("viiø7", &c).unwrap()), vec![11, 2, 5, 9]);
        assert_eq!(pcs(&roman_chord("IVmaj7", &c).unwrap()), vec![5, 9, 0, 4]);
        assert_eq!(pcs(&roman_chord("III+", &c).unwrap()), vec![4, 8, 0]);
    }

    #[test]
    fn renversements_et_accords_speciaux() {
        let c = key("c major");
        let i6 = roman_chord("I6", &c).unwrap();
        assert_eq!(pcs(&i6), vec![4, 7, 0]);
        assert!(i6[2] > i6[0]);
        assert_eq!(pcs(&roman_chord("I64", &c).unwrap()), vec![7, 0, 4]);
        assert_eq!(pcs(&roman_chord("V65", &c).unwrap()), vec![11, 2, 5, 7]);
        assert_eq!(pcs(&roman_chord("V42", &c).unwrap()), vec![5, 7, 11, 2]);
        assert_eq!(pcs(&roman_chord("N6", &c).unwrap()), vec![5, 8, 1]);
        assert_eq!(pcs(&roman_chord("Ger6", &c).unwrap()), vec![8, 0, 3, 6]);
        assert_eq!(pcs(&roman_chord("It6", &c).unwrap()), vec![8, 0, 6]);
        assert!(roman_chord("Zz", &c).is_none() && roman_chord("b", &c).is_none() && roman_chord("bass", &c).is_none());
        assert!(is_roman("V7/V") && is_roman("bVII") && !is_roman("Am7") && !is_roman("bd"));
    }

    #[test]
    fn analyse_des_fonctions() {
        let c = key("c major");
        assert_eq!(analyze_chord(2, true, false, Some(10), &c), "ii7");
        assert_eq!(analyze_chord(9, false, false, Some(10), &c), "V7/ii");
        assert_eq!(analyze_chord(2, false, false, Some(10), &c), "V7/V");
        assert_eq!(analyze_chord(10, false, false, None, &c), "bVII");
        assert_eq!(analyze_chord(5, true, false, None, &c), "iv");
        assert_eq!(analyze_chord(1, false, false, None, &c), "N");
        assert_eq!(analyze_chord(7, false, false, Some(10), &c), "V7");
    }
}
