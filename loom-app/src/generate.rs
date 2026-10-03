//! GÉNÉRER — compose a fresh, valid, musical song from curated building blocks.
//!
//! Port of the browser's `generateSong()` (`web/index.html`): the same genres,
//! tempos, scales, timbres, patterns and probabilities. The only difference is
//! the dice: the web uses `Math.random()`, here a seeded [`Rng`] so the same
//! seed always gives the same song (and tests can check hundreds of them).

/// The message the web logs after generating.
pub const DONE: &str = "[ gen ] nouveau morceau généré ✨ (reclique pour un autre)";

/// A small, fast, seedable random generator (SplitMix64). Not for crypto —
/// for musical dice that replay identically from a seed.
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed)
    }

    /// Next raw 64-bit value.
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A number in `0..1`, like `Math.random()`.
    pub fn f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// `true` with probability `p` (the JS `Math.random() < p`).
    pub fn chance(&mut self, p: f64) -> bool {
        self.f64() < p
    }

    /// One element of a non-empty slice, uniformly (the JS `R(a)`).
    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[(self.f64() * items.len() as f64) as usize % items.len()]
    }

    /// An integer in `a..=b` (the JS `ri(a, b)`).
    pub fn range(&mut self, a: i64, b: i64) -> i64 {
        a + (self.f64() * (b - a + 1) as f64) as i64
    }
}

/// A genre: its tempo range, scales, visual scenes, sidechain range, drums or not.
struct Genre {
    name: &'static str,
    bpm: (i64, i64),
    scales: &'static [&'static str],
    scenes: &'static [&'static str],
    duck: (i64, i64),
    drums: bool,
}

const GENRES: [Genre; 5] = [
    Genre { name: "techno", bpm: (124, 134), scales: &["minor", "phrygian", "dorian"], scenes: &["gtunnel", "lorenz", "tiles"], duck: (40, 60), drums: true },
    Genre { name: "ambient", bpm: (60, 84), scales: &["dorian", "lydian", "pentatonic", "aeolian"], scenes: &["shader", "aurora", "flow", "ribbon"], duck: (0, 15), drums: false },
    Genre { name: "orient", bpm: (72, 96), scales: &["hijaz", "bhairav", "pentatonic", "bayati"], scenes: &["mandala", "gfractal", "phyllo"], duck: (0, 20), drums: true },
    Genre { name: "house", bpm: (118, 126), scales: &["minor", "dorian", "major"], scenes: &["gwarp", "kaleido", "starsphere"], duck: (40, 60), drums: true },
    Genre { name: "hiphop", bpm: (84, 96), scales: &["minor", "dorian", "minpent"], scenes: &["matrix", "tiles", "comet"], duck: (20, 40), drums: true },
];

// d and a appear twice: the web weights them on purpose.
const TONICS: [&str; 8] = ["c", "d", "e", "f", "g", "a", "d", "a"];
const PROG_STYLES: [&str; 8] = ["pop", "sad", "minor", "jazz", "epic", "andalusian", "lofi", "edm"];
const LEAD_TIMBRES: [&str; 6] = [
    "saw | lp 2200", "sine | fm 1 2", "pulse | lp 2400", "wt rich | lp 3000 | double 0.3", "reed | slide",
    "sine | partials 1 0 0.5 0 0.3",
];
const BASS_TIMBRES: [&str; 4] =
    ["saw | sub 0.5 | lp 700", "sine | sub 0.6", "saw | slide | lp 520 q 8 | fenv 0.6 | drive 0.3", "square | drive 0.3 | lp 800"];
const PAD_TIMBRES: [&str; 4] = ["saw | unison 4 | lp 1200", "saw | unison 5 | vowel a", "wt warm", "bow"];
const HAT_PATTERNS: [&str; 4] = [". . x . . . x . . . x . . . x .", "x x x x x x x x x x x x x x x x", "e(9,16)", "ca(90,16)"];
const KICK_PATTERNS: [&str; 3] =
    ["x . . . x . . . x . . . x . . .", "x . . . . . x . . . x . . . . .", "x . . x . . x . x . . x . . x ."];
/// Lead melody alphabet: degrees, octave marks and two rests (so ~1 step in 5 breathes).
const LEAD_DEGREES: [&str; 10] = ["1", "2", "3", "5", "6", "1'", "3'", "5,", ".", "."];
const ECHO_DIVS: [&str; 3] = ["4", "6", "8"];
const ARP_MODES: [&str; 5] = ["up", "updown", "converge", "pinky", "down"];

/// A generated song and the genre it was drawn from.
#[derive(Debug, Clone, PartialEq)]
pub struct Generated {
    /// `techno`, `ambient`, `orient`, `house` or `hiphop`.
    pub genre: &'static str,
    /// The LOOM program.
    pub code: String,
}

/// A new song for this seed (see [`compose`]).
pub fn song(seed: u64) -> String {
    compose(seed).code
}

/// Compose a song: header directives, drums (if the genre has them), then
/// bass, pad and lead on the chord progression, maybe an arp and a bell.
pub fn compose(seed: u64) -> Generated {
    let mut rng = Rng::new(seed);
    let g = rng.pick(&GENRES);
    let tonic = rng.pick(&TONICS);
    let scale = rng.pick(g.scales);
    let bpm = rng.range(g.bpm.0, g.bpm.1);
    let song_seed = rng.range(1, 99);
    let scene = rng.pick(g.scenes);
    let prog = rng.pick(&PROG_STYLES);
    let lead = rng.pick(&LEAD_TIMBRES);
    let bass = rng.pick(&BASS_TIMBRES);
    let pad = rng.pick(&PAD_TIMBRES);
    let hat = rng.pick(&HAT_PATTERNS);
    let kick = rng.pick(&KICK_PATTERNS);

    let mut s = String::from("# ✨ genere automatiquement — reclique GENERER pour un autre\n");
    let duck = rng.range(g.duck.0, g.duck.1);
    let feel = rng.range(10, 26);
    s += &format!(
        "title: Auto\nseed: {song_seed}\nbpm: {bpm}\nkey: {tonic} {scale}\nprog: auto {prog}\nduck: {duck}\nfeel: {feel}\nwarmth: 0.4\nscene: {scene}\n"
    );
    if g.drums {
        s += &format!("kick: {kick} | sample bd | gain 0.5 | comp 0.4\n");
        if rng.chance(0.8) {
            s += &format!("hat:  {hat} | sample hh | gain 0.1 | swing {}\n", rng.range(50, 58));
        }
        if rng.chance(0.55) {
            s += "clap: . . . . x . . . . . . . x . . . | sample cp | gain 0.3\n";
        }
    }
    s += &format!("bass: ^1 . . . 5, . ^1 . ^1 . . . 6, . 5, . | {bass} | oct 2 | gate 0.5 | gain 0.15\n");
    s += &format!("pad:  ^ . . . | {pad} | env 0.3 0.3 0.7 1.5 | reverb 0.6 | gate 1.7 | gain 0.08\n");
    let melody: Vec<&str> = (0..16).map(|_| *rng.pick(&LEAD_DEGREES)).collect();
    let echo = rng.pick(&ECHO_DIVS);
    s += &format!(
        "lead: {} | {lead} | reverb 0.4 | delay 0.3 | echo {echo} 0.4 | gate 0.6 | gain 0.1\n",
        melody.join(" ")
    );
    if rng.chance(0.7) {
        let mode = rng.pick(&ARP_MODES);
        s += &format!("arp:  ^1 ^2 ^3 ^2 ^3 ^1' ^2 ^3 | arp {mode} | pluck 0.7 | autopan 0.3 | delay 0.25 | gain 0.09\n");
    }
    if rng.chance(0.5) {
        s += "bell: ^1' . . . 5 . . . | bell | reverb 0.6 | delay 0.4 | degrade 30 | gain 0.07\n";
    }
    Generated { genre: g.name, code: s }
}

/// Pattern verbs `loom-core` understands (what is left in `Voice::verbs` after
/// the sound verbs are taken out). Used by tests here and in `develop`.
#[cfg(test)]
pub(crate) const KNOWN_VERBS: [&str; 34] = [
    "rev", "fast", "slow", "every", "sometimes", "often", "rarely", "someby", "palindrome", "rot", "iter", "chunk", "add",
    "trans", "mirror", "thin", "accent", "mutate", "mut", "stutter", "stut", "shuffle", "shuf", "markov", "invert", "inv",
    "sequence", "seq", "develop", "dev", "ply", "linger", "segment", "range",
];

/// Every voice line became a voice, and no `| segment` was left unrecognised.
#[cfg(test)]
pub(crate) fn assert_fully_recognised(code: &str) {
    let program = loom_core::Program::parse(code);
    let voice_lines = code
        .lines()
        .filter(|l| {
            let t = l.trim();
            !t.is_empty() && !t.starts_with('#') && t.split_once(':').is_some_and(|(h, _)| {
                let head = h.trim().to_lowercase();
                let base = head.split('@').next().unwrap_or("");
                !crate::vibe::is_setting(base) && !head.starts_with("def ")
            })
        })
        .count();
    assert_eq!(program.voices.len(), voice_lines, "a voice line was dropped:\n{code}");
    for v in &program.voices {
        for verb in &v.verbs {
            let head = verb.split_whitespace().next().unwrap_or("");
            assert!(KNOWN_VERBS.contains(&head), "voice {} : unknown verb « {verb} »\n{code}", v.name);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rng_is_seeded_and_in_range() {
        let (mut a, mut b) = (Rng::new(42), Rng::new(42));
        for _ in 0..1000 {
            let x = a.f64();
            assert_eq!(x, b.f64());
            assert!((0.0..1.0).contains(&x));
            assert!((3..=7).contains(&a.range(3, 7)));
            b.range(3, 7);
        }
        assert_ne!(Rng::new(1).next_u64(), Rng::new(2).next_u64());
        let mut r = Rng::new(9);
        let hits = (0..10_000).filter(|_| r.chance(0.3)).count();
        assert!((2700..3300).contains(&hits), "{hits}");
    }

    #[test]
    fn same_seed_same_song() {
        assert_eq!(song(7), song(7));
        assert_ne!(song(7), song(8));
    }

    #[test]
    fn two_hundred_songs_parse_and_make_sense() {
        let mut genres = std::collections::BTreeSet::new();
        for seed in 0..200 {
            let Generated { genre, code } = compose(seed);
            genres.insert(genre);
            assert_fully_recognised(&code);
            let p = loom_core::Program::parse(&code);
            let names: Vec<&str> = p.voices.iter().map(|v| v.name.as_str()).collect();
            for must in ["bass", "pad", "lead"] {
                assert!(names.contains(&must), "seed {seed}: no {must}");
            }
            assert_eq!(names.contains(&"kick"), genre != "ambient", "seed {seed} ({genre})");
            assert!(p.key.is_some() && (60.0..=134.0).contains(&p.bpm), "seed {seed}");
            assert!(!p.prog.is_empty(), "seed {seed}: prog: auto gave no chords");
            assert!(code.starts_with("# ✨") && code.ends_with('\n'));
            // it really plays: the first bars schedule notes
            assert!((0..4).map(|bar| p.schedule_bar(bar).len()).sum::<usize>() > 10, "seed {seed}: silent");
            let lead = p.voices.iter().find(|v| v.name == "lead").unwrap();
            assert_eq!(lead.tokens.len(), 16);
        }
        assert_eq!(genres.len(), 5, "every genre shows up: {genres:?}");
    }
}
