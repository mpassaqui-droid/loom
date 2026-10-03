//! Voix bytebeat (`nom: t: EXPR`) : la formule de `t` rendue en continu.
//!
//! `t` compte les échantillons à 8 kHz depuis le début du morceau ; la sortie
//! est `((v & 255) / 128) - 1`, rééchantillonnée vers la fréquence de la carte
//! son par échantillonnage-blocage (la valeur tient jusqu'au pas de 8 kHz
//! suivant, comme une vraie puce 8 bits). Le séquenceur pose une note bytebeat
//! par mesure, bout à bout, donc le signal ne se coupe jamais à la barre.
//!
//! Verbes respectés (comme pour un sample, ici et dans le web) : `gain`, `pan`
//! (valeur fixe), `reverb`/`delay`/`send`/`bus`, `mute`/`solo`, `chance`, `in`,
//! les variantes `@section`. Ignorés (il n'y a pas de notes) : forme d'onde,
//! enveloppe, filtre et inserts de note (`lp`, `drive`, `crush`, `echo`…),
//! verbes de motif (`rev`, `fast`…), `midi` ; `loom bake` saute la voix.

use loom_core::expr::{bytebeat_sample, bytebeat_t};
use loom_core::Expr;
use std::sync::Arc;

pub struct ByteBeat {
    expr: Arc<Expr>,
    /// Échantillon absolu (depuis le début du morceau) du prochain rendu.
    n: u64,
    /// Échantillons restant à rendre dans cette mesure.
    left: u64,
    sr: u32,
    gain: f32,
    /// Dernier `t` évalué et sa valeur (échantillonnage-blocage).
    last_t: Option<i32>,
    val: f32,
}

impl ByteBeat {
    /// Une mesure de bytebeat : `len` échantillons à partir de l'échantillon
    /// absolu `start`, à la fréquence `sr`, au niveau `gain`.
    pub fn new(expr: Arc<Expr>, start: u64, len: u64, sr: f32, gain: f32) -> ByteBeat {
        ByteBeat { expr, n: start, left: len, sr: sr.round().max(1.0) as u32, gain, last_t: None, val: 0.0 }
    }

    pub fn done(&self) -> bool { self.left == 0 }

    pub fn render(&mut self) -> f32 {
        if self.left == 0 { return 0.0; }
        let t = bytebeat_t(self.n, self.sr);
        if self.last_t != Some(t) {
            self.last_t = Some(t);
            self.val = bytebeat_sample(self.expr.eval(&[t]));
        }
        self.n += 1;
        self.left -= 1;
        self.val * self.gain
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn holds_each_8k_value_and_stops() {
        let e = Arc::new(Expr::parse("t", &['t']).unwrap());
        let mut b = ByteBeat::new(e, 0, 12, 16000.0, 1.0);
        let out: Vec<f32> = (0..14).map(|_| b.render()).collect();
        // 16 kHz : chaque t tenu deux échantillons ; t/128 - 1
        let want: Vec<f32> = (0..12).map(|k| (k / 2) as f32 / 128.0 - 1.0).chain([0.0, 0.0]).collect();
        assert_eq!(out, want);
        assert!(b.done());
    }

    #[test]
    fn continues_from_the_song_clock() {
        let e = Arc::new(Expr::parse("t*3", &['t']).unwrap());
        let mut b = ByteBeat::new(e, 44100, 1, 44100.0, 0.5);
        // t = 8000 -> 24000 & 255 = 192 -> 0.5 -> × gain 0.5
        assert_eq!(b.render(), 0.25);
    }
}
