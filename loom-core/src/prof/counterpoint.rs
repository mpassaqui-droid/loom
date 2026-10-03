//! Le correcteur de contrepoint du prof : il écoute les lignes mélodiques deux à
//! deux, comme un professeur d'écriture relit une copie.
//!
//! - quintes et octaves parallèles (deux voix qui bougent dans le même sens et
//!   retombent sur la même consonance parfaite : elles fusionnent, on perd une voix) ;
//! - croisements (la voix d'en haut passe sous celle d'en bas) ;
//! - sauts de plus d'une octave dans une même voix ;
//! - tessitures, pour les voix nommées comme un chœur (soprano, alto, tenor, bass).
//!
//! Une voix qui joue des accords (une nappe) n'est pas une ligne : elle est laissée
//! de côté. Les batteries aussi. Une note tient jusqu'à la suivante de sa voix.

use crate::Program;

/// Une ligne : (temps en mesures, hauteur MIDI), dans l'ordre.
struct Line {
    name: String,
    notes: Vec<(f64, i32)>,
}

fn midi_of(freq: f32) -> i32 { (69.0 + 12.0 * (freq / 440.0).log2()).round() as i32 }

/// Les tessitures du chœur (MIDI), d'après les traités d'harmonie classiques.
fn choir_range(name: &str) -> Option<(&'static str, i32, i32)> {
    let n = name.to_lowercase();
    if n.starts_with("sop") { return Some(("soprano", 60, 79)); }
    if n.starts_with("alt") { return Some(("alto", 55, 74)); }
    if n.starts_with("ten") { return Some(("ténor", 48, 67)); }
    // « bass » reste libre : en électro c'est une basse de synthé, pas un chanteur
    if n.starts_with("basse") || n.starts_with("barit") { return Some(("basse", 40, 60)); }
    None
}

fn lines_of(program: &Program, bars: i64) -> Vec<Line> {
    let mut lines: Vec<Line> = Vec::new();
    let mut chordy: Vec<(String, usize, usize)> = Vec::new(); // (voix, attaques, attaques en accord)
    for bar in 0..bars {
        let events = program.schedule_bar(bar);
        let mut seen: Vec<(String, f32)> = Vec::new();
        for e in &events {
            let Some(f) = e.freq else { continue };
            if e.is_drum || e.token.starts_with(['.', '_', '~']) { continue; }
            let t = bar as f64 + e.phase as f64;
            let m = midi_of(f);
            let first = !seen.iter().any(|(v, p)| *v == e.voice && (*p - e.phase).abs() < 1e-4);
            let c = match chordy.iter_mut().find(|(v, ..)| *v == e.voice) {
                Some(c) => c,
                None => { chordy.push((e.voice.clone(), 0, 0)); chordy.last_mut().unwrap() }
            };
            if first { c.1 += 1; seen.push((e.voice.clone(), e.phase)); }
            let line = match lines.iter_mut().find(|l| l.name == e.voice) {
                Some(l) => l,
                None => { lines.push(Line { name: e.voice.clone(), notes: Vec::new() }); lines.last_mut().unwrap() }
            };
            match line.notes.last_mut() {
                // même attaque : un accord (ou une couche jux/off) ; on garde la note du dessus
                Some(last) if (last.0 - t).abs() < 1e-4 => {
                    if last.1 != m { if !first { c.2 += 1; } last.1 = last.1.max(m); }
                }
                _ => line.notes.push((t, m)),
            }
        }
    }
    // une voix dont plus d'un quart des attaques sont des accords est une nappe
    lines.retain(|l| {
        let (_, n, ch) = chordy.iter().find(|(v, ..)| *v == l.name).cloned().unwrap_or_default();
        l.notes.len() >= 2 && ch * 4 <= n
    });
    for l in &mut lines { l.notes.sort_by(|a, b| a.0.total_cmp(&b.0)); }
    lines
}

/// La hauteur qui sonne dans `l` au temps `t` (la dernière attaque à t ou avant).
fn at(l: &Line, t: f64) -> Option<i32> {
    let i = l.notes.partition_point(|(x, _)| *x <= t + 1e-6);
    (i > 0).then(|| l.notes[i - 1].1)
}

fn place(t: f64) -> String {
    let bar = t.floor() as i64 + 1;
    format!("mesure {bar}")
}

/// Le rapport `[ contrepoint ]` d'EXPLIQUE : seulement pour une écriture en parties
/// (au moins deux voix nommées comme un chœur : soprano, alto, tenor, basse).
/// Ailleurs (électro, raga, nappes) croiser et doubler sont des choix, pas des fautes.
pub fn counterpoint(text: &str) -> Vec<String> {
    let program = Program::parse(text);
    if program.voices.iter().filter(|v| choir_range(&v.name).is_some()).count() < 2 { return Vec::new(); }
    counterpoint_report(text)
}

/// Le correcteur sur toutes les lignes mélodiques, sans condition (`loom contrepoint`).
/// Vide s'il y a moins de deux lignes.
pub fn counterpoint_report(text: &str) -> Vec<String> {
    let program = Program::parse(text);
    let declared = program.len_bars();
    let bars = if declared > 0 { (declared as i64).min(32) } else { 8 };
    let lines = lines_of(&program, bars);
    let mut out = Vec::new();
    if lines.len() < 2 { return out; }

    // registre moyen : qui est « en haut »
    let mean = |l: &Line| l.notes.iter().map(|n| n.1 as f64).sum::<f64>() / l.notes.len() as f64;
    let mut issues = 0;
    for i in 0..lines.len() {
        for j in i + 1..lines.len() {
            let (hi, lo) = if mean(&lines[i]) >= mean(&lines[j]) { (&lines[i], &lines[j]) } else { (&lines[j], &lines[i]) };
            // les règles d'écriture valent pour deux voix qui avancent note contre note :
            // au moins la moitié des attaques de la plus rare tombent ensemble
            let has = |l: &Line, t: f64| l.notes.binary_search_by(|n| n.0.total_cmp(&t)).is_ok()
                || l.notes.iter().any(|n| (n.0 - t).abs() < 1e-6);
            let times: Vec<f64> = hi.notes.iter().map(|n| n.0).filter(|&t| has(lo, t)).collect();
            let rarest = hi.notes.len().min(lo.notes.len());
            if times.len() < 4 || times.len() * 2 < rarest { continue; }
            // une doublure (même ligne transposée d'un bout à l'autre) est voulue, pas une faute
            let ivs: Vec<i32> = times.iter().filter_map(|&t| Some((at(hi, t)? - at(lo, t)?).rem_euclid(12))).collect();
            if ivs.iter().all(|i| *i == 0) { continue; }
            let close = (mean(hi) - mean(lo)).abs() <= 12.0;
            let (mut fifths, mut octaves, mut crossings) = (Vec::new(), Vec::new(), Vec::new());
            let mut prev: Option<(i32, i32)> = None;
            for &t in &times {
                let (Some(h), Some(l)) = (at(hi, t), at(lo, t)) else { continue };
                if h < l && close { crossings.push(t); }
                if let Some((ph, pl)) = prev {
                    let (dh, dl) = (h - ph, l - pl);
                    let same_way = dh != 0 && dl != 0 && dh.signum() == dl.signum();
                    let (a, b) = ((ph - pl).rem_euclid(12), (h - l).rem_euclid(12));
                    if same_way && a == b && a == 7 { fifths.push(t); }
                    if same_way && a == b && a == 0 { octaves.push(t); }
                }
                prev = Some((h, l));
            }
            let pair = format!("{}/{}", hi.name, lo.name);
            let say = |what: &str, v: &[f64], why: &str| format!(
                "[ contrepoint ] {what} entre {pair}, {}{} : {why}",
                place(v[0]), if v.len() > 1 { format!(" (×{})", v.len()) } else { String::new() });
            if !fifths.is_empty() {
                out.push(say("quintes parallèles", &fifths, "les deux voix glissent ensemble et fusionnent, une ligne disparaît. Bouge l'une par mouvement contraire"));
                issues += 1;
            }
            if !octaves.is_empty() {
                out.push(say("octaves parallèles", &octaves, "les deux voix doublent la même ligne. Si c'est voulu (unisson), garde ; sinon, mouvement contraire"));
                issues += 1;
            }
            if !crossings.is_empty() {
                out.push(say("croisement", &crossings, &format!("{} passe sous {}, l'oreille confond les lignes", hi.name, lo.name)));
                issues += 1;
            }
        }
    }
    for l in &lines {
        let leaps: Vec<f64> = l.notes.windows(2).filter(|w| (w[1].1 - w[0].1).abs() > 12).map(|w| w[1].0).collect();
        if !leaps.is_empty() {
            out.push(format!("[ contrepoint ] saut de plus d'une octave dans {}, {}{} : difficile à chanter, comble-le par une note de passage",
                l.name, place(leaps[0]), if leaps.len() > 1 { format!(" (×{})", leaps.len()) } else { String::new() }));
            issues += 1;
        }
        if let Some((role, lo, hi)) = choir_range(&l.name) {
            let out_of: Vec<i32> = l.notes.iter().map(|n| n.1).filter(|m| *m < lo || *m > hi).collect();
            if !out_of.is_empty() {
                out.push(format!("[ contrepoint ] {} sort de la tessiture de {role} ({} note(s) hors de {}-{})",
                    l.name, out_of.len(), crate::midi_to_name(lo), crate::midi_to_name(hi)));
                issues += 1;
            }
        }
    }
    if out.len() > 4 {
        let more = out.len() - 4;
        out.truncate(4);
        out.push(format!("[ contrepoint ] … et {more} autre(s) remarque(s) du même genre"));
    }
    if issues == 0 {
        out.push(format!("[ contrepoint ] {} lignes indépendantes : ni quintes ni octaves parallèles, pas de croisement", lines.len()));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quintes_paralleles_trouvees() {
        // c-g puis d-a : quinte, quinte, même sens
        let r = counterpoint("bpm: 120\nsoprano: g4 a4 b4 c5\nalto: c4 d4 e4 f4");
        assert!(r.iter().any(|l| l.contains("quintes parallèles") && l.contains("soprano/alto") && l.contains("mesure 1") && l.contains("×3")), "{r:?}");
    }

    #[test]
    fn mouvement_contraire_propre() {
        let r = counterpoint("soprano: c5 d5 e5 f5\nbasse: c4 b3 a3 g3");
        assert_eq!(r, vec!["[ contrepoint ] 2 lignes indépendantes : ni quintes ni octaves parallèles, pas de croisement"], "{r:?}");
    }

    #[test]
    fn octaves_croisement_saut_tessiture() {
        let r = counterpoint("soprano: c5 d5 e5 f5 g5\nbasse: c4 d4 e4 f4 c4");
        assert!(r.iter().any(|l| l.contains("octaves parallèles")), "{r:?}");
        // une doublure à l'octave d'un bout à l'autre est voulue
        assert!(!counterpoint("soprano: c5 d5 e5 f5\nbasse: c4 d4 e4 f4").iter().any(|l| l.contains("octaves")));
        let r = counterpoint("sopr: c5 c5 c5 c5\nalto: g4 g4 e5 g4");
        assert!(r.iter().any(|l| l.contains("croisement") && l.contains("sopr passe sous alto")), "{r:?}");
        let r = counterpoint("sopr: c4 e5 c4 d4\nalto: c3 d3 e3 f3");
        assert!(r.iter().any(|l| l.contains("saut de plus d'une octave dans sopr")), "{r:?}");
        assert!(r.iter().any(|l| l.contains("alto sort de la tessiture d'alto") || l.contains("tessiture de alto")), "{r:?}");
    }

    #[test]
    fn nappes_et_batterie_ignorees() {
        let r = counterpoint_report("kick: x . x .\npad: C G Am F\nlead: c5 d5 e5 f5");
        assert!(r.is_empty(), "{r:?}");
        // pas de chœur : EXPLIQUE se tait, même avec des quintes
        assert!(counterpoint("lead: g4 a4 b4 c5\nbass: c4 d4 e4 f4").is_empty());
        assert!(!counterpoint_report("lead: g4 a4 b4 c5\nbass: c4 d4 e4 f4").is_empty());
    }
}
