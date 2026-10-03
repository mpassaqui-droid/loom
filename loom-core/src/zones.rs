//! Les instruments multi-échantillons : zones de touches, couches de vélocité,
//! round-robin, lus dans le NOM des fichiers, comme le font la plupart des
//! samplers. `| sample piano` cherche tout ce qui s'appelle `piano_…` :
//!
//! - `piano_c3`, `piano_f#3`, `piano-a4` : une zone de touche (note racine, do3 = MIDI 48) ;
//! - `piano_c3_v1`, `piano_c3_v2` : des couches de vélocité (v1 la plus douce) ;
//! - `snare_rr1`, `snare_rr2` : des prises alternées, jouées à tour de rôle ;
//! - le fichier `piano` lui-même, s'il existe, compte comme une zone sans étiquette.
//!
//! La note jouée prend la zone de racine la plus proche (à égalité, celle du
//! dessous), puis la couche qui correspond à sa vélocité, puis la prise suivante.
//! Le moteur natif et le web choisissent exactement le même fichier.

/// Un fichier vu comme une zone d'instrument.
#[derive(Debug, Clone, PartialEq)]
pub struct Zone {
    /// Le nom complet du fichier dans la banque (`piano_c3_v2`).
    pub name: String,
    /// L'instrument (`piano`).
    pub inst: String,
    /// La note racine en MIDI, si le nom en porte une.
    pub root: Option<i32>,
    /// La couche de vélocité (0 = pas de couche).
    pub vel: u32,
    /// Le numéro de prise (0 = pas de round-robin).
    pub rr: u32,
}

fn tag_number(t: &str, prefix: &str) -> Option<u32> {
    let rest = t.strip_prefix(prefix)?;
    // trois chiffres au plus : pas de débordement, même lecture que le web
    if rest.is_empty() || rest.len() > 3 || !rest.bytes().all(|b| b.is_ascii_digit()) { return None; }
    rest.parse().ok()
}

/// La racine d'une étiquette : `c3`, `C#3`, `cs3` (dièse à l'anglaise), `db3`.
fn tag_note(t: &str) -> Option<i32> {
    let t = t.to_ascii_lowercase();
    let t = if t.len() >= 3 && t.as_bytes()[1] == b's' { format!("{}#{}", &t[..1], &t[2..]) } else { t };
    let b = t.as_bytes();
    // il faut une octave : « c » seul ou « bass » ne sont pas des notes
    if b.len() < 2 || !b[b.len() - 1].is_ascii_digit() { return None; }
    // l'octave : des chiffres seulement (« c+3 » n'est pas une note)
    let oct = t.trim_start_matches(|c: char| c.is_ascii_alphabetic() || c == '#');
    if oct.len() > 2 || !oct.bytes().all(|c| c.is_ascii_digit()) { return None; }
    crate::note_to_midi(&t)
}

/// Lit un nom de fichier comme une zone. `None` si le nom ne porte aucune étiquette
/// (note, `vN`, `rrN`) : c'est alors un échantillon ordinaire.
pub fn parse_zone(name: &str) -> Option<Zone> {
    let parts: Vec<&str> = name.split(['_', '-', ' ', '.']).collect();
    let (mut root, mut vel, mut rr) = (None, 0, 0);
    let mut end = parts.len();
    // les étiquettes sont au bout, dans n'importe quel ordre, chacune une fois
    while end > 1 {
        let t = parts[end - 1].to_ascii_lowercase();
        if rr == 0 && tag_number(&t, "rr").is_some_and(|n| n > 0) { rr = tag_number(&t, "rr")?; }
        else if vel == 0 && tag_number(&t, "v").is_some_and(|n| n > 0) { vel = tag_number(&t, "v")?; }
        else if root.is_none() && tag_note(&t).is_some() { root = tag_note(&t); }
        else { break; }
        end -= 1;
    }
    if end == parts.len() { return None; }
    let inst = name[..parts[..end].iter().map(|p| p.len() + 1).sum::<usize>() - 1].to_string();
    if inst.is_empty() { return None; }
    Some(Zone { name: name.to_string(), inst, root, vel, rr })
}

/// Toutes les zones de `inst` parmi `names` (y compris le fichier `inst` lui-même).
/// Vide si l'instrument n'a aucun fichier étiqueté.
pub fn zones_of<'a>(names: impl IntoIterator<Item = &'a str>, inst: &str) -> Vec<Zone> {
    let mut plain = false;
    let mut zones: Vec<Zone> = Vec::new();
    for n in names {
        if n == inst { plain = true; continue; }
        if let Some(z) = parse_zone(n).filter(|z| z.inst == inst) { zones.push(z); }
    }
    if zones.is_empty() { return zones; }
    if plain { zones.push(Zone { name: inst.to_string(), inst: inst.to_string(), root: None, vel: 0, rr: 0 }); }
    zones.sort_by(|a, b| a.name.cmp(&b.name));
    zones
}

/// Le fichier à jouer : la zone de racine la plus proche de `midi` (à égalité celle
/// du dessous), la couche de vélocité de `vel` (1 = la plus forte), la prise numéro
/// `counter` modulo le nombre de prises.
pub fn pick_zone(zones: &[Zone], midi: Option<f32>, vel: f32, counter: u32) -> Option<&Zone> {
    let mut cands: Vec<&Zone> = zones.iter().collect();
    if let Some(m) = midi.filter(|m| m.is_finite()) {
        let best = cands.iter().filter_map(|z| z.root)
            .min_by(|a, b| ((*a as f32 - m).abs(), *a).partial_cmp(&((*b as f32 - m).abs(), *b)).unwrap());
        if let Some(r) = best { cands.retain(|z| z.root == Some(r)); }
    }
    let mut layers: Vec<u32> = cands.iter().map(|z| z.vel).collect();
    layers.sort_unstable();
    layers.dedup();
    if layers.len() > 1 {
        let v = if vel.is_finite() { vel.clamp(0.0, 1.0) } else { 1.0 };
        let i = ((v * layers.len() as f32).ceil() as usize).clamp(1, layers.len()) - 1;
        cands.retain(|z| z.vel == layers[i]);
    }
    let mut takes: Vec<u32> = cands.iter().map(|z| z.rr).collect();
    takes.sort_unstable();
    takes.dedup();
    if takes.len() > 1 {
        let t = takes[counter as usize % takes.len()];
        cands.retain(|z| z.rr == t);
    }
    cands.into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noms_de_zones() {
        let z = parse_zone("piano_c3_v2").unwrap();
        assert_eq!((z.inst.as_str(), z.root, z.vel, z.rr), ("piano", Some(48), 2, 0));
        let z = parse_zone("grand-piano-F#4-rr3-v1").unwrap();
        assert_eq!((z.inst.as_str(), z.root, z.vel, z.rr), ("grand-piano", Some(66), 1, 3));
        assert_eq!(parse_zone("cello_cs2").unwrap().root, Some(37));
        assert_eq!(parse_zone("snare_rr2").unwrap().root, None);
        // pas d'étiquette : un échantillon ordinaire
        assert!(parse_zone("amen_break").is_none());
        assert!(parse_zone("bd").is_none());
        assert!(parse_zone("bass").is_none());
        assert!(parse_zone("c3").is_none());
        // des nombres démesurés ne sont pas des étiquettes (ni débordement ni panique)
        assert!(parse_zone("x_c999999999").is_none());
        assert!(parse_zone("snare_rr4294967296").is_none());
        assert!(parse_zone("pad_v1000").is_none());
    }

    #[test]
    fn choisit_zone_couche_et_prise() {
        let names = ["piano_c3_v1", "piano_c3_v2", "piano_g3_v1", "piano_g3_v2", "pianola", "bd", "bd_rr2", "bd_rr3"];
        let piano = zones_of(names, "piano");
        assert_eq!(piano.len(), 4);
        // e3 (52) : plus près de c3 (48) que de g3 (55) ? 4 contre 3 : g3
        assert_eq!(pick_zone(&piano, Some(52.0), 1.0, 0).unwrap().name, "piano_g3_v2");
        assert_eq!(pick_zone(&piano, Some(50.0), 1.0, 0).unwrap().name, "piano_c3_v2");
        assert_eq!(pick_zone(&piano, Some(50.0), 0.4, 0).unwrap().name, "piano_c3_v1");
        // à égalité (d#3 = 51.5 n'existe pas, on prend 51.5 en cents) : celle du dessous
        assert_eq!(pick_zone(&piano, Some(51.5), 1.0, 0).unwrap().name, "piano_c3_v2");
        // le fichier nu compte comme une prise : bd, bd_rr2, bd_rr3, bd…
        let bd = zones_of(names, "bd");
        let seq: Vec<&str> = (0..4).map(|i| pick_zone(&bd, None, 1.0, i).unwrap().name.as_str()).collect();
        assert_eq!(seq, ["bd", "bd_rr2", "bd_rr3", "bd"]);
        assert!(zones_of(names, "pianola").is_empty());
    }
}
