//! Enregistrer : ce qu'on joue au clavier MIDI devient une ligne LOOM.
//!
//! Chaque note jouée arrive avec sa position (en mesures depuis le début de la
//! prise) et sa fin. On la cale sur la grille (16 pas par mesure au plus), on
//! écrit `_` tant qu'elle tient, et les notes jouées ensemble se rangent en piles
//! (`;`) : la plus grave sur la première ligne, la suivante sur la deuxième…
//! Une grille plus fine que nécessaire est simplifiée (8 ou 4 pas si tout y tombe).
//! Une prise de plusieurs mesures tient en une mesure de jetons : un pas qui change
//! d'une mesure à l'autre s'écrit `<mesure1/mesure2>` (l'alternance de LOOM).

/// Une note jouée : début et fin en mesures depuis le début de la prise, hauteur MIDI.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Played {
    pub on: f64,
    pub off: f64,
    pub midi: i32,
}

/// Le plus de mesures qu'une prise garde (au-delà, les notes sont laissées de côté).
pub const MAX_BARS: u32 = 64;

/// Le nombre de mesures de la prise : la mesure où tombe (une fois calé sur la
/// grille de `steps_per_bar`) le début de la dernière note, arrondie à 1, 2, 4… 64.
pub fn take_bars(notes: &[Played], steps_per_bar: u32) -> u32 {
    let spb = steps_per_bar.max(1) as f64;
    let end = notes.iter().map(|n| (n.on * spb).round() / spb).fold(0.0f64, f64::max);
    let need = (end + 1e-9).floor() as u32 + 1;
    [1, 2, 4, 8, 16, 32, MAX_BARS].into_iter().find(|b| *b >= need).unwrap_or(MAX_BARS)
}

/// La ligne `name: …` de la prise, ou `None` s'il n'y a rien à écrire.
pub fn capture_line(name: &str, notes: &[Played], steps_per_bar: u32) -> Option<String> {
    let steps_per_bar = steps_per_bar.clamp(1, 32);
    // une note jouée juste avant la barre (moins d'un demi-pas) compte pour le premier temps
    let early = 0.5 / steps_per_bar as f64;
    let notes: Vec<Played> = notes.iter().copied()
        .filter(|n| n.on > -early && n.midi > 0 && n.midi < 128 && n.on.is_finite())
        .map(|n| Played { on: n.on.max(0.0), off: if n.off.is_finite() { n.off.max(n.on) } else { n.on }, ..n })
        .collect();
    if notes.is_empty() { return None; }
    // les mesures vides avant la première note ne comptent pas : la prise commence là
    let first = notes.iter().map(|n| n.on).fold(f64::INFINITY, f64::min);
    let skip = (first + early).floor().max(0.0);
    let notes: Vec<Played> = notes.into_iter().map(|n| Played { on: (n.on - skip).max(0.0), off: (n.off - skip).max(0.0), ..n }).collect();
    let bars = take_bars(&notes, steps_per_bar);
    let total = (bars * steps_per_bar) as usize;
    // (pas de départ, pas de fin exclus, hauteur), bornés à la prise
    let mut placed: Vec<(usize, usize, i32)> = notes.iter().filter_map(|n| {
        let a = (n.on * steps_per_bar as f64).round() as usize;
        if a >= total { return None; }
        let b = ((n.off * steps_per_bar as f64).round() as usize).clamp(a + 1, total);
        Some((a, b, n.midi))
    }).collect();
    placed.sort_by_key(|p| (p.0, p.2));
    placed.dedup_by_key(|p| (p.0, p.2));
    // les piles : chaque note va sur la première ligne libre à son pas
    let mut lanes: Vec<Vec<Option<(bool, i32)>>> = Vec::new(); // (début ?, hauteur)
    for &(a, b, m) in &placed {
        let lane = match lanes.iter().position(|l| l[a].is_none()) {
            Some(i) => i,
            None => { lanes.push(vec![None; total]); lanes.len() - 1 }
        };
        // la tenue s'arrête où la ligne est déjà prise
        for (s, slot) in lanes[lane].iter_mut().enumerate().take(b).skip(a) {
            if s > a && slot.is_some() { break; }
            *slot = Some((s == a, m));
        }
    }
    // la grille la plus grossière où tous les débuts tombent
    let starts: Vec<usize> = placed.iter().map(|p| p.0).collect();
    let mut div = 1;
    for d in [8usize, 4, 2] {
        if (steps_per_bar as usize).is_multiple_of(d) && steps_per_bar as usize / d >= 4 && starts.iter().all(|s| s.is_multiple_of(d)) { div = d; break; }
    }
    // plusieurs mesures : un pas qui change d'une mesure à l'autre s'écrit <m1/m2/…>
    let per_bar = steps_per_bar as usize;
    let lines: Vec<String> = lanes.iter().map(|l| {
        // une tenue qui passe la barre se rejoue : LOOM ne lie pas d'une mesure à l'autre
        let tok = |s: usize| match l[s] {
            Some((true, m)) => crate::midi_to_name(m),
            Some((false, m)) if s.is_multiple_of(per_bar) => crate::midi_to_name(m),
            Some((false, _)) => "_".to_string(),
            None => ".".to_string(),
        };
        (0..per_bar).step_by(div).map(|s| {
            let alts: Vec<String> = (0..bars as usize).map(|b| tok(b * per_bar + s)).collect();
            if alts.iter().all(|t| *t == alts[0]) { alts[0].clone() } else { format!("<{}>", alts.join("/")) }
        }).collect::<Vec<_>>().join(" ")
    }).collect();
    Some(format!("{name}: {}", lines.join(" ; ")))
}

/// Une prise au clavier en cours : les positions viennent du moteur (mesure + phase),
/// la prise commence à la barre la plus proche du moment où on l'arme.
#[derive(Debug, Clone, Default)]
pub struct Capture {
    start: f64,
    held: Vec<(i32, f64)>,
    notes: Vec<Played>,
}

impl Capture {
    /// Armée à la position `pos` (en mesures depuis le début du morceau).
    pub fn new(pos: f64) -> Capture { Capture { start: pos.floor(), held: Vec::new(), notes: Vec::new() } }

    /// Le morceau est reparti du début (armée à l'arrêt, ou relancé) : la prise
    /// repart avec lui, sans les notes d'avant.
    fn follow(&mut self, pos: f64) {
        if pos - self.start < -0.5 {
            self.start = pos.floor();
            self.held.clear();
            self.notes.clear();
        }
    }

    pub fn note_on(&mut self, pos: f64, midi: i32) {
        self.follow(pos);
        self.note_off(pos, midi);
        self.held.push((midi, pos - self.start));
    }

    pub fn note_off(&mut self, pos: f64, midi: i32) {
        if let Some(i) = self.held.iter().position(|h| h.0 == midi) {
            let (m, on) = self.held.remove(i);
            self.notes.push(Played { on, off: pos - self.start, midi: m });
        }
    }

    /// Combien de notes sont déjà prises (tenues comprises).
    pub fn len(&self) -> usize { self.notes.len() + self.held.len() }
    pub fn is_empty(&self) -> bool { self.len() == 0 }

    /// Fin de la prise à `pos` : les notes encore tenues s'arrêtent là.
    pub fn finish(mut self, pos: f64, name: &str, steps_per_bar: u32) -> Option<String> {
        for (m, on) in std::mem::take(&mut self.held) { self.notes.push(Played { on, off: pos - self.start, midi: m }); }
        capture_line(name, &self.notes, steps_per_bar)
    }
}

// ---- prise au micro (le son lui-même vit dans loom-app et le web) ----

/// La partie utile d'un enregistrement brut : du premier temps de la mesure qui
/// suit `p0` (la position du morceau, en mesures, au premier échantillon) à la
/// dernière mesure complète. `latency` (secondes) recule la coupe du retard du micro.
/// Rend les échantillons, le nombre de mesures et la mesure du morceau où la prise
/// commence, ou `None` si moins d'une mesure.
pub fn take_window(samples: &[f32], sample_rate: u32, p0: f64, bar_sec: f64, latency: f64) -> Option<(Vec<f32>, u32, i64)> {
    if !bar_sec.is_finite() || bar_sec <= 0.0 || sample_rate == 0 || !p0.is_finite() { return None; }
    let sr = sample_rate as f64;
    // au plus près de la barre : un départ un poil après elle compte pour elle
    let bar_line = if p0 - p0.floor() < 0.05 { p0.floor() } else { p0.ceil() };
    let start = (((bar_line - p0) * bar_sec + latency) * sr).round().max(0.0) as usize;
    let per_bar = (bar_sec * sr).round() as usize;
    let avail = samples.len().saturating_sub(start);
    let bars = (avail / per_bar.max(1)) as u32;
    if bars == 0 { return None; }
    // 1, 2, 4, 8 ou 16 mesures : une boucle qui tombe juste
    let bars = [16, 8, 4, 2, 1].into_iter().find(|b| *b <= bars).unwrap_or(1);
    Some((samples[start..start + bars as usize * per_bar].to_vec(), bars, bar_line as i64))
}

/// La boucle finale, mesure par mesure. Une boucle de `L` mesures joue sa mesure
/// `q` aux mesures du morceau `k` où `k mod L = q` (la ligne a `| slow L`). La
/// prise neuve (`new_bars` mesures, commencée à la mesure `first_bar`) est rangée
/// à sa place ; l'ancienne (`old`, s'il y en a une) reste dessous, répétée si la
/// neuve est plus longue. Rend la boucle et `L`.
pub fn layout_take(new: &[f32], new_bars: u32, first_bar: i64, old: Option<&[f32]>) -> (Vec<f32>, u32) {
    let nb = new_bars.max(1) as i64;
    let per = new.len() / nb as usize;
    let ob = old.map(|o| ((o.len() as f64 / per.max(1) as f64).round() as i64).max(1));
    let l = ob.map_or(nb, |o| o.max(nb));
    let mut out = vec![0.0f32; l as usize * per];
    for q in 0..l {
        let dst = &mut out[q as usize * per..(q as usize + 1) * per];
        // la mesure de la prise qui tombe sur cette place : celle jouée à une mesure k ≡ q (mod l)
        let nq = (q - first_bar).rem_euclid(nb) as usize;
        let src_new = &new[nq * per..(nq + 1) * per];
        match (old, ob) {
            (Some(o), Some(ob)) => {
                let oq = (q % ob) as usize;
                for (i, d) in dst.iter_mut().enumerate() {
                    let ov = o.get(oq * per + i).copied().unwrap_or(0.0);
                    *d = (ov + src_new[i]).clamp(-1.0, 1.0);
                }
            }
            _ => dst.copy_from_slice(src_new),
        }
    }
    (out, l as u32)
}

/// La ligne qui joue la prise, calée sur `bars` mesures et sur le tempo : un seul
/// départ toutes les `bars` mesures.
pub fn take_line(name: &str, bars: u32) -> String {
    if bars > 1 { format!("{name}: x | sample {name} | stretch | loopAt {bars} | slow {bars}") }
    else { format!("{name}: x | sample {name} | stretch | loopAt 1") }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(on: f64, off: f64, midi: i32) -> Played { Played { on, off, midi } }

    #[test]
    fn une_melodie_calee() {
        // do mi sol do, un peu en avance ou en retard, sur les noires : la grille passe à 4
        let notes = [p(-0.01, 0.24, 60), p(0.26, 0.49, 64), p(0.49, 0.74, 67), p(0.74, 0.99, 72)];
        assert_eq!(capture_line("rec", &notes, 16).unwrap(), "rec: c4 e4 g4 c5");
        // une note tenue sur deux temps garde sa tenue
        let notes = [p(0.0, 0.5, 60), p(0.5, 0.6, 64), p(0.75, 0.8, 67)];
        assert_eq!(capture_line("rec", &notes, 16).unwrap(), "rec: c4 _ e4 g4");
        // des notes courtes sur les croches : grille simplifiée à 8
        let notes = [p(0.0, 0.05, 60), p(0.125, 0.17, 62), p(0.5, 0.55, 64)];
        assert_eq!(capture_line("rec", &notes, 16).unwrap(), "rec: c4 d4 . . e4 . . .");
    }

    #[test]
    fn accords_en_piles_et_plusieurs_mesures() {
        let notes = [p(0.0, 0.25, 48), p(0.0, 0.25, 52), p(0.0, 0.25, 55), p(1.5, 1.75, 50)];
        let l = capture_line("keys", &notes, 16).unwrap();
        assert_eq!(l, "keys: <c3/.> . <./d3> . ; <e3/.> . . . ; <g3/.> . . .");
        // la ligne se relit et sonne aux bons endroits, mesure après mesure
        let prog = crate::Program::parse(&l);
        let notes = |b: i64| { let mut v: Vec<(String, f32)> = prog.schedule_bar(b).iter().filter(|e| e.freq.is_some()).map(|e| (e.token.clone(), e.phase)).collect(); v.sort_by(|a, b| a.0.cmp(&b.0)); v };
        assert_eq!(notes(0), [("c3".into(), 0.0), ("e3".into(), 0.0), ("g3".into(), 0.0)]);
        assert_eq!(notes(1), [("d3".to_string(), 0.5)]);
        assert!(capture_line("x", &[], 16).is_none());
        // la prise armée en fin de mesure 3 commence à la barre 4 ; une note tenue à l'arrêt
        let mut c = Capture::new(3.97);
        c.note_on(3.99, 60);
        c.note_on(4.25, 64);
        c.note_off(4.5, 60);
        assert_eq!(c.len(), 2);
        assert_eq!(c.finish(4.75, "rec", 16).unwrap(), "rec: c4 _ . . ; . e4 _ .");
        assert_eq!(take_bars(&[p(4.2, 4.3, 60)], 16), 8);
        // une note sur le dernier demi-pas tombe sur la mesure suivante, pas à la trappe
        assert_eq!(capture_line("rec", &[p(0.0, 0.2, 60), p(0.99, 1.2, 62)], 16).unwrap(), "rec: <c4/d4> . . .");
        // une tenue qui passe la barre se rejoue au début de la mesure suivante
        assert_eq!(capture_line("rec", &[p(0.5, 1.5, 60), p(1.75, 1.9, 64)], 16).unwrap(), "rec: <./c4> <./_> <c4/.> <_/e4>");
        // armée à l'arrêt (le moteur montrait encore la mesure 37) : la prise suit la relance
        let mut c = Capture::new(37.2);
        c.note_on(0.0, 60);
        c.note_off(0.25, 60);
        assert_eq!(c.finish(0.5, "rec", 16).unwrap(), "rec: c4 . . .");
        // armée en fin de mesure : une note juste après compte
        let mut c = Capture::new(3.6);
        c.note_on(3.75, 64);
        c.note_off(3.9, 64);
        assert!(c.finish(4.0, "rec", 16).unwrap().contains("e4"));
        // armée trop tôt : les mesures vides du début tombent
        assert_eq!(capture_line("rec", &[p(2.0, 2.25, 60), p(2.25, 2.5, 64)], 16).unwrap(), "rec: c4 e4 . .");
    }
}
