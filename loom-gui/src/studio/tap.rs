//! TAP tempo: tap a key or a button in rhythm, LOOM finds the BPM.
//!
//! Ported from the browser's `tapBtn`: taps older than 2 seconds are
//! forgotten; from the second tap on, the tempo is the average gap between
//! the taps, rounded, kept between 40 and 240 BPM.

use std::time::Instant;

/// Taps older than this (in seconds) don't count any more.
const WINDOW: f64 = 2.0;

pub struct TapTempo {
    origin: Instant,
    /// When the recent taps happened, in seconds since `origin`.
    taps: Vec<f64>,
}

impl Default for TapTempo {
    fn default() -> TapTempo {
        TapTempo { origin: Instant::now(), taps: Vec::new() }
    }
}

impl TapTempo {
    /// A tap now. Returns the tempo once there are enough taps.
    pub fn tap(&mut self) -> Option<u32> {
        let now = self.origin.elapsed().as_secs_f64();
        self.tap_at(now)
    }

    /// A tap at `now` seconds (any clock that only goes forward).
    pub fn tap_at(&mut self, now: f64) -> Option<u32> {
        self.taps.retain(|t| now - t < WINDOW);
        self.taps.push(now);
        if self.taps.len() < 2 {
            return None;
        }
        let gaps = self.taps.windows(2).map(|w| w[1] - w[0]);
        let average = gaps.sum::<f64>() / (self.taps.len() - 1) as f64;
        if average <= 0.0 {
            return None;
        }
        Some((60.0 / average).round().clamp(40.0, 240.0) as u32)
    }

    /// How many taps count right now (for "tape encore…").
    pub fn count(&self) -> usize {
        self.taps.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steady_taps_give_the_tempo() {
        let mut tap = TapTempo::default();
        assert_eq!(tap.tap_at(10.0), None, "one tap is not a tempo yet");
        assert_eq!(tap.tap_at(10.5), Some(120));
        assert_eq!(tap.tap_at(11.0), Some(120));
        // uneven taps are averaged: gaps 0.5, 0.5, 0.8 → 0.6 s → 100 BPM
        assert_eq!(tap.tap_at(11.8), Some(100));
    }

    #[test]
    fn a_long_pause_starts_over() {
        let mut tap = TapTempo::default();
        tap.tap_at(0.0);
        tap.tap_at(0.4);
        assert_eq!(tap.tap_at(5.0), None, "taps older than 2 s are forgotten");
        assert_eq!(tap.count(), 1);
    }

    #[test]
    fn tempo_is_kept_between_40_and_240() {
        let mut tap = TapTempo::default();
        tap.tap_at(0.0);
        assert_eq!(tap.tap_at(0.1), Some(240));
        let mut slow = TapTempo::default();
        slow.tap_at(0.0);
        assert_eq!(slow.tap_at(1.9), Some(40));
    }
}
