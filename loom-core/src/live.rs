//! Le lien avec Ableton Live, côté langage (pur, testé ; le device Max l'applique).
//!
//! - `@voix.Param: 0 .25 .5 1 | slow 2` : automation d'un paramètre Live, jouée comme
//!   un motif. Valeurs de 0 à 1 (le device les ramène à l'échelle du paramètre),
//!   `.` ou `_` = garde la valeur précédente. Sans `voix.`, toutes les pistes Loom
//!   qui ont ce paramètre le suivent.
//! - `go: drop` : lance la scène Live nommée « drop » (ou `go: 3`, la 3e scène).
//! - [`clip_to_loom`] : un clip MIDI de Live devient une ligne de code LOOM.

/// Une automation à écrire dans le clip : `(temps, durée, valeur 0..1)` en noires.
#[derive(Debug, Clone, PartialEq)]
pub struct Automation {
    pub voice: Option<String>,
    pub param: String,
    pub points: Vec<(f64, f64, f64)>,
}

fn lignes(src: &str) -> Vec<String> {
    let expanded = crate::expand_macros(src);
    expanded.lines().map(|l| crate::decomment(l).trim().to_string()).filter(|l| !l.is_empty()).collect()
}

/// Plafond de pas d'automation par ligne : chaque pas est un appel LiveAPI dans Max.
pub const MAX_POINTS: usize = 4096;

/// Les lignes `@…:` du programme, déroulées sur `bars` mesures de `meter_beats` noires.
pub fn automations(src: &str, bars: u32, meter_beats: f64) -> Vec<Automation> {
    let meter_beats = if meter_beats.is_finite() && meter_beats > 0.0 { meter_beats } else { 4.0 };
    let total = bars.max(1) as f64 * meter_beats;
    let mut out = Vec::new();
    for l in lignes(src) {
        let Some(corps) = l.strip_prefix('@') else { continue };
        let Some(colon) = corps.find(':') else { continue };
        let tete = corps[..colon].trim();
        let (voice, param) = match tete.split_once('.') {
            Some((v, p)) if !v.trim().is_empty() => (Some(v.trim().to_lowercase()), p.trim().to_string()),
            _ => (None, tete.trim_start_matches('.').trim().to_string()),
        };
        if param.is_empty() { continue; }
        let mut morceaux = corps[colon + 1..].split('|');
        let valeurs: Vec<Option<f64>> = morceaux.next().unwrap_or("").split_whitespace()
            .map(|t| if t == "." || t == "_" { None } else { t.parse::<f64>().ok().filter(|v| v.is_finite()).map(|v| v.clamp(0.0, 1.0)) })
            .collect();
        if valeurs.iter().all(|v| v.is_none()) { continue; }
        let mut cycle = meter_beats;
        for verbe in morceaux {
            let mut w = verbe.split_whitespace();
            let n = |w: &mut std::str::SplitWhitespace| w.next().and_then(|x| x.parse::<f64>().ok()).filter(|x| x.is_finite() && *x > 0.0);
            match w.next() {
                Some("slow") => { if let Some(k) = n(&mut w) { cycle *= k; } }
                Some("fast") => { if let Some(k) = n(&mut w) { cycle /= k; } }
                _ => {}
            }
        }
        // au plus fin, une double-croche par valeur ; au plus long, tout le morceau
        let cycle = cycle.clamp(0.25 * valeurs.len() as f64, total.max(meter_beats));
        let pas = cycle / valeurs.len() as f64;
        let mut points = Vec::new();
        let mut debut = 0.0;
        while debut < total - 1e-9 {
            for (i, v) in valeurs.iter().enumerate() {
                let t = debut + i as f64 * pas;
                if t >= total - 1e-9 { break; }
                if let Some(v) = v { points.push((t, pas.min(total - t), *v)); }
            }
            debut += cycle;
            if points.len() > MAX_POINTS { points.truncate(MAX_POINTS); break; }
        }
        out.push(Automation { voice, param, points });
    }
    out
}

/// `go: scène` : la scène Live à lancer quand ce code est joué.
pub fn go_scene(src: &str) -> Option<String> {
    lignes(src).iter().rev().find_map(|l| {
        let (tete, reste) = l.split_once(':')?;
        (tete.trim().eq_ignore_ascii_case("go") && !reste.trim().is_empty()).then(|| reste.trim().to_string())
    })
}

fn nom_note(midi: i32) -> String {
    const N: [&str; 12] = ["c", "c#", "d", "d#", "e", "f", "f#", "g", "g#", "a", "a#", "b"];
    format!("{}{}", N[midi.rem_euclid(12) as usize], midi.div_euclid(12) - 1)
}

/// Un clip MIDI (`(hauteur, début, durée, vélocité)` en noires) devenu une ligne LOOM,
/// au double-croche. Les notes qui se chevauchent passent sur des lignes empilées (`;`),
/// `_` prolonge une note, un clip de plusieurs mesures se joue avec `| slow N`.
/// En batterie, une ligne par élément (kick, snare, clap, hat, en General MIDI), chaque
/// coup devient `x` (`x!` quand il est fort) ; les autres pads restent des notes.
pub fn clip_to_loom(nom: &str, notes: &[(i32, f64, f64, u8)], longueur: f64, meter_beats: f64, batterie: bool) -> String {
    let meter_beats = if meter_beats.is_finite() && meter_beats > 0.0 { meter_beats } else { 4.0 };
    let longueur = if longueur.is_finite() && longueur > 0.0 { longueur } else { meter_beats };
    let mut nom: String = nom.to_lowercase().chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    if nom.is_empty() || nom.starts_with(|c: char| c.is_ascii_digit()) { nom = format!("clip{nom}"); }
    if crate::DIRECTIVES.contains(&nom.as_str()) || nom == "go" { nom.push('2'); }
    let mesures = ((longueur / meter_beats).ceil() as usize).clamp(1, 64);
    let par_mesure = (meter_beats * 4.0).round().max(1.0) as usize;
    let total = mesures * par_mesure;
    // le pas de chaque note : arrondi à la double-croche, jamais hors du clip
    let pas = |debut: f64| -> Option<usize> {
        if !debut.is_finite() || debut >= longueur { return None; }
        Some(((debut.max(0.0) * 4.0).round() as usize).min(total - 1))
    };
    let mut tri: Vec<&(i32, f64, f64, u8)> = notes.iter().collect();
    tri.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal).then(b.0.cmp(&a.0)));
    let suffixe = if mesures > 1 { format!(" | slow {mesures}") } else { String::new() };
    let mut lignes: Vec<String> = Vec::new();
    let mut melodie: Vec<&(i32, f64, f64, u8)> = Vec::new();
    if batterie {
        // une ligne par élément de batterie (General MIDI), les autres pads en notes
        const PADS: [(&str, &[i32]); 4] = [("kick", &[35, 36]), ("snare", &[37, 38, 40]), ("clap", &[39]), ("hat", &[42, 44, 46])];
        let mut voies: Vec<(&str, Vec<String>)> = PADS.iter().map(|(n, _)| (*n, vec![".".to_string(); total])).collect();
        for n in &tri {
            let Some(s) = pas(n.1) else { continue };
            match PADS.iter().position(|(_, p)| p.contains(&n.0)) {
                Some(k) => {
                    let fort = n.3 >= 110;
                    let c = &mut voies[k].1[s];
                    if c == "." || (fort && c == "x") { *c = if fort { "x!".into() } else { "x".into() }; }
                }
                None => melodie.push(n),
            }
        }
        for (n, v) in voies {
            if v.iter().any(|c| c != ".") { lignes.push(format!("{n}: {}{suffixe}", v.join(" "))); }
        }
        if melodie.is_empty() {
            if lignes.is_empty() { lignes.push(format!("{nom}: {}{suffixe}", vec!["."; total].join(" "))); }
            return lignes.join("\n");
        }
    } else {
        melodie = tri;
    }
    let mut couches: Vec<Vec<String>> = Vec::new();
    for &&(hauteur, debut, duree, _) in &melodie {
        let Some(s) = pas(debut) else { continue };
        let long = (((duree.max(0.0)) * 4.0).round() as usize).max(1).min(total - s);
        let libre = |c: &Vec<String>| (s..s + long).all(|i| c[i] == ".");
        let k = match couches.iter().position(libre) {
            Some(k) => k,
            None => { couches.push(vec![".".into(); total]); couches.len() - 1 }
        };
        couches[k][s] = nom_note(hauteur.clamp(0, 127));
        for c in &mut couches[k][s + 1..s + long] { *c = "_".into(); }
    }
    if couches.is_empty() { couches.push(vec![".".into(); total]); }
    let motif = couches.iter().map(|c| c.join(" ")).collect::<Vec<_>>().join(" ; ");
    lignes.push(format!("{nom}: {motif}{suffixe}"));
    lignes.join("\n")
}

fn esc(s: &str) -> String {
    let mut o = String::new();
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o
}

/// Ce que le device applique dans Live, en JSON : `{"go":…, "auto":[{voice, param, points}]}`.
pub fn live_json(src: &str, bars: u32, meter_beats: f64) -> String {
    let go = match go_scene(src) { Some(g) => format!("\"{}\"", esc(&g)), None => "null".into() };
    let esc = |x: &str| esc(x);
    let autos = automations(src, bars, meter_beats).iter().map(|a| {
        let v = match &a.voice { Some(v) => format!("\"{}\"", esc(v)), None => "null".into() };
        let p = a.points.iter().map(|(t, d, x)| format!("[{t:.5},{d:.5},{x:.5}]")).collect::<Vec<_>>().join(",");
        format!("{{\"voice\":{v},\"param\":\"{}\",\"points\":[{p}]}}", esc(&a.param))
    }).collect::<Vec<_>>().join(",");
    format!("{{\"go\":{go},\"auto\":[{autos}]}}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn automation_un_point_par_pas_sur_tout_le_morceau() {
        let a = automations("kick: x . x .\n@bass.Filter Freq: 0 .5 . 1", 2, 4.0);
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].voice.as_deref(), Some("bass"));
        assert_eq!(a[0].param, "Filter Freq");
        // 3 valeurs par mesure (le « . » garde la précédente), 2 mesures
        assert_eq!(a[0].points.len(), 6);
        assert_eq!(a[0].points[0], (0.0, 1.0, 0.0));
        assert_eq!(a[0].points[2], (3.0, 1.0, 1.0));
        assert_eq!(a[0].points[3].0, 4.0);
    }

    #[test]
    fn automation_slow_fast_sans_voix_et_bornes() {
        let a = automations("@Volume: 0 2 | slow 2", 2, 4.0);
        assert_eq!(a[0].voice, None);
        assert_eq!(a[0].points, vec![(0.0, 4.0, 0.0), (4.0, 4.0, 1.0)]);
        let f = automations("@x.Pan: 0 1 | fast 2", 1, 4.0);
        assert_eq!(f[0].points.len(), 4);
        assert!(automations("@x.Pan: . . junk", 1, 4.0).is_empty());
        assert!(automations("@: 1", 1, 4.0).is_empty());
    }

    #[test]
    fn les_lignes_live_ne_sont_pas_des_voix() {
        let p = crate::Program::parse("kick: x . x .\n@bass.Filter: 0 1\ngo: drop");
        assert_eq!(p.voices.len(), 1);
        assert_eq!(p.voices[0].name, "kick");
        assert_eq!(go_scene("go: Drop 2\nkick: x"), Some("Drop 2".into()));
        assert_eq!(go_scene("kick: x"), None);
    }

    #[test]
    fn clip_melodique_avec_accord_et_tenue() {
        // do-mi ensemble au temps 1 (une noire), sol en double-croche au temps 3
        let n = [(48, 0.0, 1.0, 100), (52, 0.0, 1.0, 100), (55, 2.0, 0.25, 100)];
        let l = clip_to_loom("Lead 1", &n, 4.0, 4.0, false);
        assert_eq!(l, "lead1: e3 _ _ _ . . . . g3 . . . . . . . ; c3 _ _ _ . . . . . . . . . . . .");
        let p = crate::Program::parse(&l);
        assert_eq!(p.voices.len(), 1);
        let notes: Vec<String> = p.schedule_bar(0).into_iter().filter(|e| e.token != "_").map(|e| e.token).collect();
        assert_eq!(notes, vec!["e3", "g3", "c3"], "trois notes rejouées");
    }

    #[test]
    fn clip_batterie_une_ligne_par_element() {
        let n = [(36, 0.0, 0.1, 100), (36, 4.0, 0.1, 120), (38, 4.0, 0.1, 90), (42, 0.5, 0.1, 80), (60, 1.0, 0.5, 100)];
        let l = clip_to_loom("Beat", &n, 8.0, 4.0, true);
        let lignes: Vec<&str> = l.lines().collect();
        assert_eq!(lignes.len(), 4, "{l}");
        assert!(lignes[0].starts_with("kick: x . . .") && lignes[0].ends_with("| slow 2") && lignes[0].contains(" x! "), "{l}");
        assert!(lignes[1].starts_with("snare: ") && lignes[2].starts_with("hat: . . x"), "{l}");
        assert!(lignes[3].starts_with("beat: . . . . c4 _"), "un pad hors batterie reste une note : {l}");
        let p = crate::Program::parse(&l);
        assert_eq!(p.voices.iter().filter(|v| ["kick", "snare", "hat"].contains(&v.name.as_str())).count(), 3);
    }

    #[test]
    fn clip_bornes() {
        assert!(clip_to_loom("Key", &[], 4.0, 4.0, false).starts_with("key2: ."));
        assert!(clip_to_loom("3", &[], 4.0, 4.0, false).starts_with("clip3: "));
        // note à 3.9 temps : dernière double-croche, pas perdue ; mesure 0 ou NaN : 4 temps
        assert!(clip_to_loom("a", &[(60, 3.9, 0.1, 100)], 4.0, 4.0, false).ends_with(" c4"));
        assert!(clip_to_loom("a", &[(60, 0.0, 1.0, 100)], 4.0, 0.0, false).starts_with("a: c4"));
        assert!(clip_to_loom("a", &[(60, 0.0, 1.0, 100)], f64::NAN, f64::NAN, false).starts_with("a: c4"));
        assert!(clip_to_loom("a", &[(60, 9.0, 1.0, 100), (60, -1.0, 1.0, 100)], 4.0, 4.0, false).starts_with("a: c4 _ _ _ . "));
    }

    #[test]
    fn automation_bornee() {
        assert!(automations("@a.P: 1 | fast inf", 8, 4.0)[0].points.len() <= MAX_POINTS);
        let f = automations("@a.P: 1 0 | fast 1e7", 8, 4.0);
        assert_eq!(f[0].points.len(), 128, "au plus fin une double-croche : 8 mesures × 16");
        assert!(automations("@a.P: nan inf", 1, 4.0).is_empty());
        assert!(automations("@a.P: 1", 1, 0.0)[0].points.len() == 1);
    }

    #[test]
    fn json_pour_le_device() {
        let t = live_json("go: my\tscene\n@a.P\u{1}x: 1", 1, 4.0);
        assert!(t.contains("my\\u0009scene") && t.contains("P\\u0001x"), "{t}");
        let j = live_json("go: drop\n@a.P: 1", 1, 4.0);
        assert_eq!(j, "{\"go\":\"drop\",\"auto\":[{\"voice\":\"a\",\"param\":\"P\",\"points\":[[0.00000,4.00000,1.00000]]}]}");
    }
}
