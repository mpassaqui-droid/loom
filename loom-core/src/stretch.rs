//! `| stretch` : étirer un son dans le temps sans changer sa hauteur, par vocodeur
//! de phase, en gardant les attaques nettes.
//!
//! Le son est découpé en fenêtres de 2048 échantillons qui se chevauchent. On lit
//! les fenêtres plus serrées (ou plus espacées) qu'on ne les réécrit ; dans chaque
//! fenêtre, chaque fréquence garde sa vraie vitesse de rotation de phase, donc la
//! hauteur ne bouge pas. Une attaque (la caisse claire, le pincé d'une corde) se
//! repère au saut d'énergie entre deux fenêtres : là on recopie la phase d'origine
//! au lieu de la prolonger, sinon l'attaque s'étale et « bave ».
//!
//! Tout est en f64 et déterministe : le web (JS) fait exactement le même calcul.

/// Taille de fenêtre (puissance de deux).
pub const FRAME: usize = 2048;
/// Pas de réécriture : quatre fenêtres se chevauchent.
pub const HOP: usize = FRAME / 4;
/// Une fenêtre dont l'énergie monte d'au moins cette part est une attaque.
pub const TRANSIENT: f64 = 0.35;

/// FFT complexe sur place, radix 2 (n puissance de deux). `inverse` sans le 1/n.
pub fn fft(re: &mut [f64], im: &mut [f64], inverse: bool) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 { j ^= bit; bit >>= 1; }
        j |= bit;
        if i < j { re.swap(i, j); im.swap(i, j); }
    }
    let mut len = 2;
    while len <= n {
        let ang = 2.0 * std::f64::consts::PI / len as f64 * if inverse { 1.0 } else { -1.0 };
        let (wr, wi) = (ang.cos(), ang.sin());
        for start in (0..n).step_by(len) {
            let (mut cr, mut ci) = (1.0, 0.0);
            for k in 0..len / 2 {
                let (a, b) = (start + k, start + k + len / 2);
                let (tr, ti) = (re[b] * cr - im[b] * ci, re[b] * ci + im[b] * cr);
                re[b] = re[a] - tr; im[b] = im[a] - ti;
                re[a] += tr; im[a] += ti;
                let nr = cr * wr - ci * wi;
                ci = cr * wi + ci * wr;
                cr = nr;
            }
        }
        len <<= 1;
    }
}

fn hann(n: usize) -> Vec<f64> {
    (0..n).map(|i| 0.5 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / n as f64).cos()).collect()
}

fn wrap(x: f64) -> f64 {
    let tau = 2.0 * std::f64::consts::PI;
    x - tau * (x / tau).round()
}

/// Les attaques de `mono`, en échantillons. On lit des fenêtres tous les [`HOP`] ;
/// une fenêtre dont l'énergie spectrale monte d'au moins [`TRANSIENT`] signale une
/// attaque. Plusieurs fenêtres de suite voient la même : on garde celle qui monte le
/// plus, et l'attaque est placée aux trois quarts de cette fenêtre (là où entre le neuf).
pub fn onsets(mono: &[f64]) -> Vec<usize> {
    let win = hann(FRAME);
    let mut prev: Vec<f64> = vec![0.0; FRAME / 2 + 1];
    let (mut re, mut im) = (vec![0.0; FRAME], vec![0.0; FRAME]);
    let mut out: Vec<(usize, f64)> = Vec::new();
    let mut a = 0;
    while a < mono.len() {
        for i in 0..FRAME { re[i] = mono.get(a + i).copied().unwrap_or(0.0) * win[i]; im[i] = 0.0; }
        fft(&mut re, &mut im, false);
        let (mut rise, mut total) = (0.0, 0.0);
        for b in 0..=FRAME / 2 {
            let m = (re[b] * re[b] + im[b] * im[b]).sqrt();
            rise += (m - prev[b]).max(0.0);
            total += m;
            prev[b] = m;
        }
        let r = if total > 1e-3 { rise / total } else { 0.0 };
        // une fenêtre qui déborde la fin voit le silence ajouté : pas une attaque
        if a > 0 && a + FRAME <= mono.len() && r > TRANSIENT {
            let t = a + 3 * FRAME / 4;
            match out.last_mut() {
                // la même attaque vue par la fenêtre suivante
                Some(l) if t < l.0 + FRAME => { if r > l.1 { *l = (t, r); } }
                _ => out.push((t, r)),
            }
        }
        a += HOP;
    }
    // affine : dans la fenêtre, le bloc de 64 échantillons dont l'énergie saute le plus
    out.into_iter().map(|(t, _)| {
        let a = t - 3 * FRAME / 4;
        let e: Vec<f64> = (0..FRAME / 64).map(|j| (0..64).map(|i| mono.get(a + j * 64 + i).map_or(0.0, |x| x * x)).sum()).collect();
        let best = (1..e.len()).max_by(|&x, &y| {
            let r = |j: usize| e[j] / (e[j.saturating_sub(4)..j].iter().sum::<f64>() / (j - j.saturating_sub(4)) as f64 + 1e-9);
            r(x).total_cmp(&r(y))
        }).unwrap_or(0);
        a + best * 64
    }).collect()
}

/// Demi-largeur du verrou autour d'une attaque : toute fenêtre qui la contient y tombe.
const LOCK: f64 = (FRAME / 2 + HOP) as f64;

/// La carte du temps : des tronçons (début d'entrée, longueur d'entrée, début de
/// sortie, longueur de sortie, verrouillé). Chaque attaque tombe exactement à son
/// temps étiré (t × facteur, le rythme suit le tempo) et son voisinage est recopié
/// au même pas ; les creux entre deux attaques absorbent l'étirement.
struct TimeMap { segs: Vec<(f64, f64, f64, f64, bool)> }

impl TimeMap {
    fn new(onsets: &[usize], in_len: usize, out_len: usize) -> TimeMap {
        let (inl, outl) = (in_len as f64, out_len as f64);
        let f = outl / inl;
        let mut segs = Vec::new();
        let (mut pi, mut po) = (0.0, 0.0); // fin du dernier tronçon posé
        // le tout début est verrouillé aussi : un son qui commence fort garde son attaque
        for &t in std::iter::once(&0).chain(onsets) {
            let t = t as f64;
            let (ia, ib) = ((t - LOCK).max(0.0), (t + LOCK).min(inl));
            if ib <= ia { continue; }
            let (oa, ob) = (t * f - (t - ia), t * f + (ib - t));
            // il faut un creux avant (sauf collé au début) et de l'entrée après le verrou,
            // sinon la fin de la sortie n'aurait plus rien à lire
            if ia < pi || oa < po || ob > outl || (ia > pi) != (oa > po) || ib >= inl { continue; }
            if ia > pi { segs.push((pi, ia - pi, po, oa - po, false)); }
            segs.push((ia, ib - ia, oa, ob - oa, true));
            (pi, po) = (ib, ob);
        }
        if pi < inl && po < outl { segs.push((pi, inl - pi, po, outl - po, false)); }
        else if segs.is_empty() { segs.push((0.0, inl, 0.0, outl, false)); }
        TimeMap { segs }
    }

    /// La position d'entrée qui sonne au temps de sortie `t`, et si elle est verrouillée.
    fn at(&self, t: f64) -> (f64, bool) {
        let seg = self.segs.iter().find(|s| t < s.2 + s.3).unwrap_or(self.segs.last().unwrap());
        let (ia, il, oa, ol, lock) = *seg;
        (ia + (t - oa) * if ol > 0.0 { il / ol } else { 1.0 }, lock && t >= oa)
    }
}

/// Étire `channels` (un ou deux canaux de même longueur) à `out_len` échantillons,
/// hauteur gardée. Les attaques sont détectées sur la somme des canaux, pour que les
/// deux côtés se recalent ensemble.
pub fn stretch(channels: &[Vec<f32>], out_len: usize) -> Vec<Vec<f32>> {
    // des canaux de longueurs différentes : on garde la plus courte ; un NaN ou un
    // infini deviendrait du NaN partout après la FFT, il compte pour un silence
    let in_len = channels.iter().map(|c| c.len()).min().unwrap_or(0);
    let clean: Vec<Vec<f32>> = channels.iter().map(|c| c[..in_len].iter().map(|x| if x.is_finite() { *x } else { 0.0 }).collect()).collect();
    let channels = &clean[..];
    if in_len == 0 || out_len == 0 { return channels.iter().map(|_| vec![0.0; out_len]).collect(); }
    // les fenêtres commencent avant 0 pour que le tout début soit couvert à plein
    let pad = FRAME - HOP;
    let frames = (out_len + pad).div_ceil(HOP) + 1;
    let win = hann(FRAME);
    let mono: Vec<f64> = (0..in_len).map(|i| channels.iter().map(|c| c[i] as f64).sum::<f64>()).collect();
    let map = TimeMap::new(&onsets(&mono), in_len, out_len);
    // pour chaque fenêtre : où lire (le centre suit la carte) et faut-il recaler la phase
    let half = (FRAME / 2) as f64;
    let mut plan: Vec<(f64, bool)> = Vec::with_capacity(frames);
    let mut was_locked = false;
    for k in 0..frames {
        // le centre de la fenêtre de sortie suit la carte
        let (c, lock) = map.at((k * HOP) as f64 - pad as f64 + half);
        let reset = k == 0 || (lock && !was_locked);
        was_locked = lock;
        plan.push(((c - half).round(), reset));
    }
    let omega: Vec<f64> = (0..=FRAME / 2).map(|b| 2.0 * std::f64::consts::PI * b as f64 / FRAME as f64).collect();
    let total = frames * HOP + FRAME;
    let mut norm = vec![0.0f64; total];
    for k in 0..frames { for i in 0..FRAME { norm[k * HOP + i] += win[i] * win[i]; } }
    channels.iter().map(|ch| {
        let mut acc = vec![0.0f64; total];
        let (mut re, mut im) = (vec![0.0; FRAME], vec![0.0; FRAME]);
        let mut last = vec![0.0f64; FRAME / 2 + 1];
        let mut synth = vec![0.0f64; FRAME / 2 + 1];
        for k in 0..frames {
            let (a, reset) = plan[k];
            for i in 0..FRAME {
                let j = a + i as f64;
                re[i] = if j >= 0.0 { ch.get(j as usize).map_or(0.0, |x| *x as f64) } else { 0.0 } * win[i];
                im[i] = 0.0;
            }
            fft(&mut re, &mut im, false);
            // le vrai pas d'analyse depuis la fenêtre d'avant
            let step = if k == 0 { 0.0 } else { a - plan[k - 1].0 };
            for b in 0..=FRAME / 2 {
                let mag = (re[b] * re[b] + im[b] * im[b]).sqrt();
                let ph = im[b].atan2(re[b]);
                if reset || step <= 0.0 {
                    synth[b] = ph;
                } else {
                    let dev = wrap(ph - last[b] - omega[b] * step);
                    synth[b] = wrap(synth[b] + (omega[b] + dev / step) * HOP as f64);
                }
                last[b] = ph;
                re[b] = mag * synth[b].cos();
                im[b] = mag * synth[b].sin();
                if b > 0 && b < FRAME / 2 { re[FRAME - b] = re[b]; im[FRAME - b] = -im[b]; }
            }
            im[0] = 0.0;
            im[FRAME / 2] = 0.0;
            fft(&mut re, &mut im, true);
            let s = k * HOP;
            for i in 0..FRAME { acc[s + i] += re[i] / FRAME as f64 * win[i]; }
        }
        (pad..pad + out_len).map(|i| if norm[i] > 1e-3 { (acc[i] / norm[i]) as f32 } else { 0.0 }).collect()
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn freq_of(x: &[f32], sr: f64) -> f64 {
        // passages par zéro montants, au milieu du signal
        let mid = &x[x.len() / 4..3 * x.len() / 4];
        let ups = mid.windows(2).filter(|w| w[0] <= 0.0 && w[1] > 0.0).count();
        ups as f64 * sr / mid.len() as f64
    }

    #[test]
    fn la_fin_ne_tombe_pas_dans_le_silence() {
        // un son qui s'arrête net (fausse attaque au bout) et des sons très courts
        for (len, factor) in [(4410usize, 10.0), (4410, 100.0), (1000, 2.0), (1500, 4.0), (44100, 3.0), (1, 5.0)] {
            let x: Vec<f32> = (0..len).map(|i| (i as f32 * 0.2).sin() * 0.5).collect();
            let n = (len as f64 * factor) as usize;
            let out = &stretch(&[x], n)[0];
            assert_eq!(out.len(), n);
            if len < 100 { continue; }
            let tail = out[n - n / 10..n - n / 20].iter().fold(0.0f32, |m, v| m.max(v.abs()));
            assert!(tail > 0.1, "{len} ×{factor} : fin muette ({tail})");
        }
    }

    #[test]
    fn canaux_inegaux_et_valeurs_folles() {
        let out = stretch(&[vec![0.1; 3000], vec![0.2; 2900]], 6000);
        assert_eq!((out.len(), out[0].len(), out[1].len()), (2, 6000, 6000));
        let mut x: Vec<f32> = (0..9000).map(|i| (i as f32 * 0.1).sin()).collect();
        x[100] = f32::NAN;
        x[200] = f32::INFINITY;
        assert!(stretch(&[x], 9000)[0].iter().all(|v| v.is_finite()));
    }

    #[test]
    fn fft_aller_retour() {
        let mut re: Vec<f64> = (0..16).map(|i| (i as f64 * 0.7).sin()).collect();
        let orig = re.clone();
        let mut im = vec![0.0; 16];
        fft(&mut re, &mut im, false);
        fft(&mut re, &mut im, true);
        for (a, b) in re.iter().zip(&orig) { assert!((a / 16.0 - b).abs() < 1e-12); }
    }

    #[test]
    fn la_hauteur_reste_la_duree_change() {
        let sr = 44100.0;
        let sine: Vec<f32> = (0..44100).map(|i| (2.0 * std::f64::consts::PI * 440.0 * i as f64 / sr).sin() as f32 * 0.5).collect();
        for factor in [0.5, 1.5, 2.0] {
            let n = (44100.0 * factor) as usize;
            let out = stretch(std::slice::from_ref(&sine), n);
            assert_eq!(out[0].len(), n);
            let f = freq_of(&out[0], sr);
            assert!((f - 440.0).abs() < 8.0, "×{factor} : {f} Hz"); // un passage par zéro de plus ou de moins = 4 Hz
            // le niveau ne bouge pas (sinus de crête 0,5)
            let peak = out[0][n / 4..3 * n / 4].iter().fold(0.0f32, |m, x| m.max(x.abs()));
            assert!((peak - 0.5).abs() < 0.1, "×{factor} : crête {peak}");
        }
    }

    #[test]
    fn les_attaques_restent_nettes() {
        // quatre coups (bruit amorti) espacés de 0,25 s, étirés ×2, serrés ×0,75, ×1,3
        let sr = 44100usize;
        let mut seed = 1u32;
        let mut noise = || { seed = seed.wrapping_mul(1664525).wrapping_add(1013904223); (seed >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0 };
        let x: Vec<f32> = (0..sr).map(|i| { let t = (i % (sr / 4)) as f32 / sr as f32; noise() * (-t * 60.0).exp() * 0.8 }).collect();
        for factor in [2.0, 0.75, 1.3] {
            let n = (sr as f64 * factor) as usize;
            let out = &stretch(std::slice::from_ref(&x), n)[0];
            assert_eq!(out.len(), n);
            // les coups dans la sortie : le premier échantillon au-dessus de 0,3 après un creux
            let mut hits: Vec<usize> = Vec::new();
            for (i, v) in out.iter().enumerate() {
                let calm = out[i.saturating_sub(sr / 10)..i.saturating_sub(64)].iter().all(|x| x.abs() < 0.05);
                if v.abs() > 0.3 && calm && hits.last().is_none_or(|h| i - h > sr / 10) { hits.push(i); }
            }
            assert_eq!(hits.len(), 4, "×{factor} {hits:?}");
            assert!(hits[0] < 50, "{hits:?}");
            // l'étirement passe entre les coups : ils restent au quart de la sortie
            for w in hits.windows(2) { assert!((w[1] - w[0]).abs_diff(n / 4) < sr / 200, "×{factor} {hits:?}"); }
            // et chaque attaque reste sèche : presque rien dans les 30 ms d'avant
            for &h in &hits[1..] {
                let before = out[h - 1300..h - 100].iter().fold(0.0f32, |m, v| m.max(v.abs()));
                let peak = out[h..h + 600].iter().fold(0.0f32, |m, v| m.max(v.abs()));
                assert!(before < peak * 0.1, "×{factor} pré-écho {before} contre {peak} à {h}");
            }
        }
    }
}
