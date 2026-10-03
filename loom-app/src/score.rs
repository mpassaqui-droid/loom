//! La partition imprimable : le morceau en MusicXML (MuseScore, Sibelius,
//! Finale, Dorico l'ouvrent) et en LilyPond. On part des notes « cuites »
//! (`loom_audio::bake`), on les cale sur une grille, on coupe aux barres de
//! mesure avec des liaisons, et on écrit chaque voix comme une portée.

use loom_audio::bake::{Baked, BakedTrack};

/// Divisions par noire : 24 couvre la triple croche (3) et les triolets de
/// double croche (4).
pub const DIV: u32 = 24;

/// Une case de mesure : des hauteurs (vide = silence), une durée en divisions.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub pitches: Vec<u8>,
    pub dur: u32,
    /// Liée à la suivante / à la précédente.
    pub tie_start: bool,
    pub tie_stop: bool,
}

/// Une portée : nom, batterie ou non, clé de fa ou non, ses mesures.
#[derive(Clone, Debug)]
pub struct Staff {
    pub name: String,
    pub drum: bool,
    pub bass_clef: bool,
    pub measures: Vec<Vec<Item>>,
}

/// La chiffrage : `(numérateur, dénominateur)` à partir des temps par mesure.
pub fn time_signature(meter_beats: f64) -> (u32, u32) {
    if (meter_beats - meter_beats.round()).abs() < 1e-6 { (meter_beats.round().max(1.0) as u32, 4) }
    else if ((meter_beats * 2.0) - (meter_beats * 2.0).round()).abs() < 1e-6 { ((meter_beats * 2.0).round() as u32, 8) }
    else { ((meter_beats * 4.0).round().max(1.0) as u32, 16) }
}

/// Les valeurs qu'on sait écrire, de la plus longue à la plus courte :
/// (durée, type, pointée, triolet).
const VALUES: [(u32, &str, bool, bool); 14] = [
    (96, "whole", false, false), (72, "half", true, false), (48, "half", false, false),
    (36, "quarter", true, false), (32, "half", false, true), (24, "quarter", false, false),
    (18, "eighth", true, false), (16, "quarter", false, true), (12, "eighth", false, false),
    (9, "16th", true, false), (8, "eighth", false, true), (6, "16th", false, false),
    (4, "16th", false, true), (3, "32nd", false, false),
];

/// Coupe une durée en valeurs écrivables (la plus longue d'abord).
pub fn split_duration(mut d: u32) -> Vec<u32> {
    let mut out = Vec::new();
    while d >= 3 {
        let v = VALUES.iter().find(|(v, ..)| *v <= d).map_or(3, |(v, ..)| *v);
        out.push(v);
        d -= v;
    }
    // un reste d'une ou deux divisions s'ajoute à la dernière valeur
    if d > 0 { if let Some(last) = out.last_mut() { *last += d; } else { out.push(d); } }
    out
}

fn value_of(d: u32) -> (&'static str, bool, bool) {
    VALUES.iter().find(|(v, ..)| *v == d).map_or(("16th", false, false), |(_, t, dot, trip)| (*t, *dot, *trip))
}

/// Les notes d'une voix calées sur la grille et coupées en mesures.
pub fn staff(track: &BakedTrack, bars: u32, meter_beats: f64) -> Staff {
    let bars = bars.clamp(1, MAX_BARS);
    let meter_beats = if meter_beats.is_finite() { meter_beats.clamp(0.25, 64.0) } else { 4.0 };
    let bar = (meter_beats * DIV as f64).round().max(1.0) as u32;
    let total = bar * bars;
    // chaque temps choisit sa grille : double croche ou triolet de croches.
    // Le triolet doit être nettement meilleur (le swing et l'humanize restent droits).
    let beats = total.div_ceil(DIV) as usize;
    let mut err = vec![(0.0f64, 0.0f64); beats];
    for n in &track.notes {
        for t in [n.beat, n.beat + n.dur] {
            let x = t * DIV as f64;
            let b = (x / DIV as f64).floor().max(0.0) as usize;
            if b < beats {
                err[b].0 += (x - (x / 6.0).round() * 6.0).abs();
                err[b].1 += (x - (x / 8.0).round() * 8.0).abs();
            }
        }
    }
    let triplet: Vec<bool> = err.iter().map(|(straight, trip)| *trip + 0.5 < *straight * 0.5).collect();
    let q = |beats_at: f64| -> u32 {
        let x = (beats_at * DIV as f64).max(0.0);
        let b = (x / DIV as f64).floor() as usize;
        let g = if triplet.get(b).copied().unwrap_or(false) { 8.0 } else { 6.0 };
        ((x / g).round() * g) as u32
    };
    // accords : notes qui commencent ensemble
    let mut events: Vec<(u32, u32, Vec<u8>)> = Vec::new();
    let mut notes: Vec<_> = track.notes.iter().filter(|n| n.beat >= 0.0).collect();
    notes.sort_by(|a, b| a.beat.total_cmp(&b.beat));
    for n in notes {
        let s = q(n.beat);
        if s >= total { continue; }
        let e = q(n.beat + n.dur.max(0.01)).max(s + 3).min(total);
        match events.last_mut() {
            Some(last) if last.0 == s => { if !last.2.contains(&n.pitch) { last.2.push(n.pitch); } last.1 = last.1.min(e).max(s + 3); }
            _ => events.push((s, e, vec![n.pitch])),
        }
    }
    const PERC: [&str; 16] = ["ride", "brush", "hat", "hh", "snare", "sd", "clap", "cp", "perc", "shaker", "tom", "crash", "cym", "rim", "kick", "bd"];
    let one_pitch = track.notes.windows(2).all(|w| w[0].pitch == w[1].pitch);
    let drum = track.drum || (one_pitch && PERC.iter().any(|p| track.name.to_lowercase().contains(p)));
    // une note s'arrête où la suivante commence (une voix par portée) ; un petit
    // trou après une note courte se lit mieux prolongé (comme on l'écrit à la main) :
    // jusqu'à un temps pour la batterie, une demi-croche… une croche pour les notes
    for i in 0..events.len() {
        let next = events.get(i + 1).map_or(total, |e| e.0);
        if drum {
            // un coup de batterie s'écrit jusqu'au suivant, au plus une noire
            events[i].1 = next.min(events[i].0 + DIV);
        } else if next > events[i].1 && next - events[i].1 <= DIV / 2 {
            events[i].1 = next;
        }
        events[i].1 = events[i].1.min(next);
        events[i].2.sort_unstable();
    }
    // la ligne continue : notes et silences
    let mut line: Vec<(u32, u32, Vec<u8>)> = Vec::new();
    let mut t = 0;
    for (s, e, p) in events {
        if s > t { line.push((t, s, Vec::new())); }
        if e > s { line.push((s, e, p)); }
        t = e.max(t);
    }
    if t < total { line.push((t, total, Vec::new())); }
    // coupe aux barres de mesure puis en valeurs écrivables
    let mut measures: Vec<Vec<Item>> = vec![Vec::new(); bars as usize];
    let is_trip = |t: u32| triplet.get((t / DIV) as usize).copied().unwrap_or(false);
    for (s, e, p) in line {
        let mut a = s;
        while a < e {
            let m = a / bar;
            let mut b = e.min((m + 1) * bar);
            // un triolet ne déborde pas de son temps (ni un temps droit dans un triolet)
            let next_beat = (a / DIV + 1) * DIV;
            if next_beat < b && (is_trip(a) || is_trip(next_beat)) { b = next_beat; }
            let parts = split_duration(b - a);
            let n = parts.len();
            for (k, d) in parts.into_iter().enumerate() {
                let rest = p.is_empty();
                measures[m as usize].push(Item {
                    pitches: p.clone(),
                    dur: d,
                    tie_start: !rest && (k + 1 < n || b < e),
                    tie_stop: !rest && (k > 0 || a > s),
                });
            }
            a = b;
        }
    }
    // une mesure entièrement vide = une pause entière
    for m in &mut measures {
        if m.iter().all(|i| i.pitches.is_empty()) { *m = vec![Item { pitches: Vec::new(), dur: bar, tie_start: false, tie_stop: false }]; }
    }
    let pitched: Vec<u32> = track.notes.iter().map(|n| n.pitch as u32).collect();
    let median = if pitched.is_empty() { 60 } else { let mut v = pitched.clone(); v.sort_unstable(); v[v.len() / 2] };
    // une voix de samples à une seule hauteur au nom de percussion va sur la portée de batterie
    Staff { name: track.name.clone(), drum, bass_clef: !drum && median < 55, measures }
}

/// L'armure en quintes (+ dièses, − bémols), depuis la tonique et la tierce.
pub fn key_fifths(key: Option<&loom_core::Key>) -> i32 {
    let Some(k) = key else { return 0 };
    let minor = k.scale.iter().any(|x| (*x - 3.0).abs() < 1e-6) && !k.scale.iter().any(|x| (*x - 4.0).abs() < 1e-6);
    let major_tonic = if minor { (k.tonic + 3).rem_euclid(12) } else { k.tonic.rem_euclid(12) };
    // C G D A E B F# / Db Ab Eb Bb F
    const FIFTHS: [i32; 12] = [0, -5, 2, -3, 4, -1, 6, 1, -4, 3, -2, 5];
    FIFTHS[major_tonic as usize]
}

/// (lettre, altération, octave) d'une note MIDI, en dièses ou en bémols selon l'armure.
pub fn spell(midi: u8, flats: bool) -> (char, i32, i32) {
    const SHARP: [(char, i32); 12] = [('C', 0), ('C', 1), ('D', 0), ('D', 1), ('E', 0), ('F', 0), ('F', 1), ('G', 0), ('G', 1), ('A', 0), ('A', 1), ('B', 0)];
    const FLAT: [(char, i32); 12] = [('C', 0), ('D', -1), ('D', 0), ('E', -1), ('E', 0), ('F', 0), ('G', -1), ('G', 0), ('A', -1), ('A', 0), ('B', -1), ('B', 0)];
    let (step, alter) = if flats { FLAT[midi as usize % 12] } else { SHARP[midi as usize % 12] };
    (step, alter, midi as i32 / 12 - 1)
}

/// Où poser une note de batterie GM sur la portée de percussion, et sa tête.
fn drum_display(pitch: u8) -> (char, i32, &'static str) {
    match pitch {
        35 | 36 => ('F', 4, "normal"),
        37..=40 => ('C', 5, if pitch == 39 { "x" } else { "normal" }),
        41 | 43 => ('A', 4, "normal"),
        45 | 47 => ('D', 5, "normal"),
        48 | 50 => ('E', 5, "normal"),
        42 | 44 | 46 => ('G', 5, "x"),
        49 | 52 | 55 | 57 => ('A', 5, "x"),
        51 | 53 | 59 => ('F', 5, "x"),
        _ => ('E', 4, "x"),
    }
}

/// La note GM d'une voix de percussion : devinée par son nom (`ride`, `kick`…),
/// sinon sa propre hauteur (une piste de batterie GM).
fn perc_pitch(name: &str, pitch: u8) -> u8 {
    let n = name.to_lowercase();
    let by_name = [("kick", 36), ("bd", 36), ("snare", 38), ("sd", 38), ("clap", 39), ("cp", 39), ("rim", 37), ("hat", 42), ("hh", 42),
        ("ride", 51), ("crash", 49), ("cym", 49), ("tom", 45), ("brush", 38), ("shaker", 82), ("perc", 39)];
    by_name.iter().find(|(k, _)| n.contains(k)).map_or(pitch, |(_, p)| *p)
}

fn esc(s: &str) -> String { s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;") }

/// Le morceau en MusicXML 4.0 (partwise).
pub fn musicxml(baked: &Baked, title: &str, key: Option<&loom_core::Key>) -> String {
    let (num, den) = time_signature(baked.meter_beats);
    let fifths = key_fifths(key);
    let staves: Vec<Staff> = baked.tracks.iter().filter(|t| !t.notes.is_empty()).map(|t| staff(t, baked.bars, baked.meter_beats)).collect();
    let mut x = String::new();
    x.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"no\"?>\n");
    x.push_str("<!DOCTYPE score-partwise PUBLIC \"-//Recordare//DTD MusicXML 4.0 Partwise//EN\" \"http://www.musicxml.org/dtds/partwise.dtd\">\n");
    x.push_str("<score-partwise version=\"4.0\">\n");
    x.push_str(&format!("  <work><work-title>{}</work-title></work>\n", esc(title)));
    x.push_str("  <identification><encoding><software>LOOM</software></encoding></identification>\n  <part-list>\n");
    for (i, s) in staves.iter().enumerate() {
        x.push_str(&format!("    <score-part id=\"P{}\"><part-name>{}</part-name></score-part>\n", i + 1, esc(&s.name)));
    }
    x.push_str("  </part-list>\n");
    for (i, s) in staves.iter().enumerate() {
        x.push_str(&format!("  <part id=\"P{}\">\n", i + 1));
        let mut last_bpm = -1.0;
        for (m, items) in s.measures.iter().enumerate() {
            x.push_str(&format!("    <measure number=\"{}\">\n", m + 1));
            if m == 0 {
                x.push_str(&format!("      <attributes><divisions>{DIV}</divisions><key><fifths>{}</fifths></key><time><beats>{num}</beats><beat-type>{den}</beat-type></time>", if s.drum { 0 } else { fifths }));
                x.push_str(if s.drum { "<clef><sign>percussion</sign></clef>" } else if s.bass_clef { "<clef><sign>F</sign><line>4</line></clef>" } else { "<clef><sign>G</sign><line>2</line></clef>" });
                x.push_str("</attributes>\n");
            }
            if i == 0 {
                if let Some((name, ..)) = baked.sections.iter().find(|(_, start, _)| *start as usize == m) {
                    x.push_str(&format!("      <direction placement=\"above\"><direction-type><rehearsal>{}</rehearsal></direction-type></direction>\n", esc(name)));
                }
                let bpm = baked.bar_bpm.get(m).copied().unwrap_or(120.0);
                if (bpm - last_bpm).abs() > 0.01 {
                    x.push_str(&format!("      <direction placement=\"above\"><direction-type><metronome><beat-unit>quarter</beat-unit><per-minute>{}</per-minute></metronome></direction-type><sound tempo=\"{bpm:.2}\"/></direction>\n", bpm.round()));
                }
            }
            last_bpm = baked.bar_bpm.get(m).copied().unwrap_or(120.0);
            let whole_rest = items.len() == 1 && items[0].pitches.is_empty();
            for it in items {
                let (kind, dot, trip) = value_of(it.dur);
                let pitches: Vec<Option<u8>> = if it.pitches.is_empty() { vec![None] } else { it.pitches.iter().map(|p| Some(*p)).collect() };
                for (k, p) in pitches.iter().enumerate() {
                    x.push_str("      <note>");
                    if k > 0 { x.push_str("<chord/>"); }
                    match p {
                        None => x.push_str(if whole_rest { "<rest measure=\"yes\"/>" } else { "<rest/>" }),
                        Some(p) if s.drum => {
                            let (st, oc, _) = drum_display(perc_pitch(&s.name, *p));
                            x.push_str(&format!("<unpitched><display-step>{st}</display-step><display-octave>{oc}</display-octave></unpitched>"));
                        }
                        Some(p) => {
                            let (st, alter, oc) = spell(*p, fifths < 0);
                            let alter = if alter != 0 { format!("<alter>{alter}</alter>") } else { String::new() };
                            x.push_str(&format!("<pitch><step>{st}</step>{alter}<octave>{oc}</octave></pitch>"));
                        }
                    }
                    x.push_str(&format!("<duration>{}</duration>", it.dur));
                    if it.tie_stop && p.is_some() { x.push_str("<tie type=\"stop\"/>"); }
                    if it.tie_start && p.is_some() { x.push_str("<tie type=\"start\"/>"); }
                    if !whole_rest { x.push_str(&format!("<type>{kind}</type>")); }
                    if dot { x.push_str("<dot/>"); }
                    if trip { x.push_str("<time-modification><actual-notes>3</actual-notes><normal-notes>2</normal-notes></time-modification>"); }
                    if let (Some(p), true) = (p, s.drum) { x.push_str(&format!("<notehead>{}</notehead>", drum_display(perc_pitch(&s.name, *p)).2)); }
                    if (it.tie_start || it.tie_stop) && p.is_some() {
                        x.push_str("<notations>");
                        if it.tie_stop { x.push_str("<tied type=\"stop\"/>"); }
                        if it.tie_start { x.push_str("<tied type=\"start\"/>"); }
                        x.push_str("</notations>");
                    }
                    x.push_str("</note>\n");
                }
            }
            x.push_str("    </measure>\n");
        }
        x.push_str("  </part>\n");
    }
    x.push_str("</score-partwise>\n");
    x
}

/// Une chaîne LilyPond sûre : guillemets et barres obliques échappés, pas de
/// caractère de contrôle (un `\` final fermait la chaîne et laissait passer du Scheme).
fn ly_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars().filter(|c| !c.is_control()) {
        match c { '\\' => out.push_str("\\\\"), '"' => out.push_str("\\\""), _ => out.push(c) }
    }
    out.push('"');
    out
}

/// Au plus tant de mesures dans une partition (une heure à 120 bpm en 4/4).
pub const MAX_BARS: u32 = 2048;

fn ly_duration(d: u32) -> String {
    let (kind, dot, _) = value_of(d);
    let n = match kind { "whole" => "1", "half" => "2", "quarter" => "4", "eighth" => "8", "16th" => "16", _ => "32" };
    format!("{n}{}", if dot { "." } else { "" })
}

fn ly_pitch(midi: u8, flats: bool) -> String {
    let (step, alter, oct) = spell(midi, flats);
    let mut s = step.to_ascii_lowercase().to_string();
    s.push_str(match alter { 1 => "is", -1 => "es", _ => "" });
    // LilyPond : c = do3 (MIDI 48), c' = do4
    let marks = oct - 3;
    if marks > 0 { s.push_str(&"'".repeat(marks as usize)); } else if marks < 0 { s.push_str(&",".repeat((-marks) as usize)); }
    s
}

fn ly_drum(pitch: u8) -> &'static str {
    match pitch { 35 | 36 => "bd", 37 => "ss", 38 | 40 => "sn", 39 => "hc", 42 | 44 => "hh", 46 => "hho", 49 | 57 => "cymc", 51 | 59 => "cymr", 41 | 43 => "tomfl", 45 | 47 => "tomml", 48 | 50 => "tomh", _ => "cb" }
}

/// Le morceau en LilyPond (`lilypond partition.ly` donne un PDF).
pub fn lilypond(baked: &Baked, title: &str, key: Option<&loom_core::Key>) -> String {
    let (num, den) = time_signature(baked.meter_beats);
    let fifths = key_fifths(key);
    let flats = fifths < 0;
    const MAJ: [&str; 15] = ["ces", "ges", "des", "aes", "ees", "bes", "f", "c", "g", "d", "a", "e", "b", "fis", "cis"];
    let key_cmd = format!("\\key {} \\major", MAJ[(fifths + 7).clamp(0, 14) as usize]);
    let bpm = baked.bar_bpm.first().copied().unwrap_or(120.0).round();
    let mut out = format!("\\version \"2.24.0\"\n\\header {{ title = {} tagline = \"LOOM\" }}\n\n\\score {{\n  <<\n", ly_str(title));
    for t in baked.tracks.iter().filter(|t| !t.notes.is_empty()) {
        let s = staff(t, baked.bars, baked.meter_beats);
        let name = ly_str(&s.name);
        let mut body = String::new();
        for (m, items) in s.measures.iter().enumerate() {
            if let Some((sec, ..)) = baked.sections.iter().find(|(_, start, _)| *start as usize == m) {
                body.push_str(&format!("\\mark \\markup \\box {} ", ly_str(sec)));
            }
            for it in items {
                let d = ly_duration(it.dur);
                let note = if it.pitches.is_empty() {
                    if items.len() == 1 { format!("R1*{num}/{den}") } else { format!("r{d}") }
                } else if s.drum {
                    let ps: Vec<&str> = it.pitches.iter().map(|p| ly_drum(perc_pitch(&s.name, *p))).collect();
                    if ps.len() == 1 { format!("{}{d}", ps[0]) } else { format!("<{}>{d}", ps.join(" ")) }
                } else {
                    let ps: Vec<String> = it.pitches.iter().map(|p| ly_pitch(*p, flats)).collect();
                    if ps.len() == 1 { format!("{}{d}", ps[0]) } else { format!("<{}>{d}", ps.join(" ")) }
                };
                let note = if value_of(it.dur).2 && !note.starts_with('R') { format!("\\tuplet 3/2 {{ {note} }}") } else { note };
                body.push_str(&note);
                if it.tie_start { body.push('~'); }
                body.push(' ');
            }
            body.push_str("|\n      ");
        }
        if s.drum {
            out.push_str(&format!("    \\new DrumStaff \\with {{ instrumentName = {name} }} \\drummode {{\n      \\time {num}/{den} \\tempo 4 = {bpm}\n      {body}}}\n"));
        } else {
            let clef = if s.bass_clef { "bass" } else { "treble" };
            out.push_str(&format!("    \\new Staff \\with {{ instrumentName = {name} }} {{\n      \\clef {clef} {key_cmd} \\time {num}/{den} \\tempo 4 = {bpm}\n      {body}}}\n"));
        }
    }
    out.push_str("  >>\n  \\layout { }\n}\n");
    out
}

/// Écrit la partition à côté du morceau : `.musicxml` (ou `.xml`) ou `.ly`.
pub fn write(src: &str, title: &str, bars: u32, path: &std::path::Path) -> Result<usize, String> {
    if bars > MAX_BARS { return Err(format!("partition : {MAX_BARS} mesures au plus")); }
    let baked = loom_audio::bake::bake(src, bars);
    let program = loom_core::Program::parse(src);
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    let text = match ext.as_str() {
        "ly" => lilypond(&baked, title, program.key.as_ref()),
        "musicxml" | "xml" => musicxml(&baked, title, program.key.as_ref()),
        _ => return Err(format!("partition : .musicxml ou .ly, pas .{ext}")),
    };
    std::fs::write(path, text).map_err(|e| format!("{} : {e}", path.display()))?;
    Ok(baked.tracks.iter().filter(|t| !t.notes.is_empty()).count())
}

#[cfg(test)]
mod tests {
    use super::*;
    use loom_audio::bake::BakedNote;

    fn track(notes: &[(u8, f64, f64)]) -> BakedTrack {
        BakedTrack { name: "lead".into(), drum: false, notes: notes.iter().map(|&(pitch, beat, dur)| BakedNote { pitch, beat, dur, vel: 100 }).collect() }
    }

    #[test]
    fn durees_et_chiffrages() {
        assert_eq!(split_duration(24), vec![24]);
        assert_eq!(split_duration(30), vec![24, 6]);
        assert_eq!(split_duration(60), vec![48, 12]);
        assert_eq!(split_duration(8), vec![8]);
        assert_eq!(time_signature(4.0), (4, 4));
        assert_eq!(time_signature(3.5), (7, 8));
        assert_eq!(time_signature(3.0), (3, 4));
    }

    #[test]
    fn chaque_mesure_fait_le_bon_compte() {
        // une note qui déborde la barre est coupée et liée
        let s = staff(&track(&[(60, 0.0, 1.0), (64, 3.0, 2.0), (67, 6.5, 0.25)]), 2, 4.0);
        for m in &s.measures { assert_eq!(m.iter().map(|i| i.dur).sum::<u32>(), 4 * DIV); }
        let tied: Vec<_> = s.measures.iter().flatten().filter(|i| i.pitches == vec![64]).collect();
        assert_eq!(tied.len(), 2);
        assert!(tied[0].tie_start && tied[1].tie_stop);
        // 7/8
        let s = staff(&track(&[(60, 0.0, 0.5)]), 3, 3.5);
        for m in &s.measures { assert_eq!(m.iter().map(|i| i.dur).sum::<u32>(), 84); }
    }

    #[test]
    fn accords_triolets_et_orthographe() {
        let s = staff(&track(&[(60, 0.0, 1.0), (64, 0.0, 1.0), (67, 0.0, 1.0), (72, 1.0, 1.0 / 3.0), (74, 1.0 + 1.0 / 3.0, 1.0 / 3.0), (76, 1.0 + 2.0 / 3.0, 1.0 / 3.0)]), 1, 4.0);
        assert_eq!(s.measures[0][0].pitches, vec![60, 64, 67]);
        assert_eq!(s.measures[0][1].dur, 8, "triolet de croches");
        assert_eq!(spell(61, false), ('C', 1, 4));
        assert_eq!(spell(61, true), ('D', -1, 4));
        assert_eq!(ly_pitch(60, false), "c'");
        assert_eq!(ly_pitch(46, true), "bes,");
    }

    #[test]
    fn swing_reste_droit_et_triolets_restent_dans_leur_temps() {
        // croches swinguées à 58 % : écrites droites
        let swung: Vec<(u8, f64, f64)> = (0..4).flat_map(|b| [(60, b as f64, 0.5), (62, b as f64 + 0.58, 0.4)]).collect();
        let s = staff(&track(&swung), 1, 4.0);
        assert!(s.measures[0].iter().all(|i| i.dur % 6 == 0), "{:?}", s.measures[0]);
        // un vrai triolet au temps 2, croches droites ailleurs
        let mixed = [(60, 0.0, 0.5), (62, 0.5, 0.5), (64, 1.0, 1.0 / 3.0), (65, 1.0 + 1.0 / 3.0, 1.0 / 3.0), (67, 1.0 + 2.0 / 3.0, 1.0 / 3.0), (69, 2.0, 2.0)];
        let s = staff(&track(&mixed), 1, 4.0);
        let durs: Vec<u32> = s.measures[0].iter().map(|i| i.dur).collect();
        assert_eq!(durs, vec![12, 12, 8, 8, 8, 48]);
        // une voix « ride » de samples va sur la portée de batterie
        let mut ride = track(&[(60, 0.0, 0.5), (60, 1.0, 0.5)]);
        ride.name = "ride".into();
        assert!(staff(&ride, 1, 4.0).drum);
    }

    #[test]
    fn chaines_lilypond_sures_et_taille_bornee() {
        assert_eq!(ly_str("a\\"), "\"a\\\\\"");
        assert_eq!(ly_str("x\" #(system \"rm\")"), "\"x\\\" #(system \\\"rm\\\")\"");
        let b = loom_audio::bake::bake("lead: c4 d4", 2);
        let ly = lilypond(&b, "fin\\", None);
        assert!(ly.contains("title = \"fin\\\\\""), "{ly}");
        let s = staff(&track(&[(60, 0.0, 1.0)]), u32::MAX, 4.0);
        assert_eq!(s.measures.len(), MAX_BARS as usize);
        assert!(write("lead: c4", "t", MAX_BARS + 1, std::path::Path::new("/tmp/x.ly")).is_err());
    }

    #[test]
    fn armures() {
        let k = |src: &str| key_fifths(loom_core::Program::parse(&format!("key: {src}\nlead: c4")).key.as_ref());
        assert_eq!(k("c major"), 0);
        assert_eq!(k("a minor"), 0);
        assert_eq!(k("d major"), 2);
        assert_eq!(k("f major"), -1);
        assert_eq!(k("c minor"), -3);
        assert_eq!(k("e minor"), 1);
    }

    #[test]
    fn un_vrai_morceau_en_musicxml_et_lilypond() {
        let src = "bpm: 96\nkey: d minor\nsong: a*2 b*2\nkick: x . x . | in a b\nbass: 1 . 5 . | oct 2\nlead: 1 3 5 3' . 2 . 1 | tri";
        let b = loom_audio::bake::bake(src, 4);
        let x = musicxml(&b, "essai", loom_core::Program::parse(src).key.as_ref());
        assert!(x.starts_with("<?xml") && x.ends_with("</score-partwise>\n"));
        assert_eq!(x.matches("<part id=").count(), 3);
        assert_eq!(x.matches("<measure number=").count(), 12);
        assert!(x.contains("<fifths>-1</fifths>") && x.contains("<sign>percussion</sign>") && x.contains("<sign>F</sign>"));
        assert!(x.contains("<rehearsal>a</rehearsal>") && x.contains("<rehearsal>b</rehearsal>") && x.contains("<per-minute>96</per-minute>"));
        // balises équilibrées
        assert_eq!(x.matches("<note>").count(), x.matches("</note>").count());
        assert_eq!(x.matches("<measure ").count(), x.matches("</measure>").count());
        let ly = lilypond(&b, "essai", loom_core::Program::parse(src).key.as_ref());
        assert!(ly.contains("\\key f \\major") && ly.contains("\\drummode") && ly.contains("\\clef bass"));
        assert_eq!(ly.matches('{').count(), ly.matches('}').count());
    }
}
