//! LOOM core — the shared language engine.
//!
//! This is a faithful Rust port of the JavaScript prototype's logic. The same
//! crate is wrapped by `loom-wasm` (browser / phone) and `loom-tui` (terminal),
//! so the language behaves identically everywhere. No external dependencies.

mod synth;
pub use synth::{expand_macros, BusConfig, Bend, Echo, Env, Eq3, FilterType, Flange, Fm, Harm, Lfo, MixSettings, Param, Reson, Room, SendSpec, SigShape, Synth};

/// Deterministic mono PCM renderer (P6-1 of the unified synthesis/FX engine) —
/// std-only, no dependencies; consumes the Rust scheduler's events directly.
pub mod render;
pub mod render_fx;

pub mod prof;

/// Expressions entières pures (bytebeat `t:` et pas-programmes `=(…)`).
pub mod expr;
pub mod live;
pub mod harmony;
pub use expr::Expr;

use std::sync::Arc;

/// Polymeter / explicit polyrhythm on one voice, from `| poly …`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Poly {
    /// `poly N`: run an independent N-step loop per bar and drift against the
    /// bar grid when the token count != N (Reich-style phasing).
    Meter(u32),
    /// `poly m:n`: explicit polyrhythm — the bar is cut into `n` windows (the
    /// grid), each holding `m` equally-spaced steps (a m-over-n tuplet), so a
    /// `3:2` voice really plays three against two (triolet).
    Grid(u32, u32),
}

/// A single voice (one line of LOOM source), already parsed.
#[derive(Debug, Clone, PartialEq)]
pub struct Voice {
    pub name: String,
    pub tokens: Vec<String>,
    /// Stacked simultaneous lines (`a b ; c d`). `tokens` is `stacks[0]`; when there
    /// is only one line `stacks` holds just it. Verbs apply to every line.
    pub stacks: Vec<Vec<String>>,
    pub verbs: Vec<String>,
    pub wave: Wave,
    pub gain: f32,
    pub is_drum: bool,
    /// Reference octave for this voice's scale degrees (`| oct N`). When `None`,
    /// the key's octave is used. Has no effect on absolute note names.
    pub octave: Option<i32>,
    /// Arpeggio mode (`| arp up|down|…`). Spreads chord tokens across the step.
    pub arp: Option<ArpMode>,
    /// Sections this voice plays in (`| in verse chorus`). `None` = always on.
    /// Only consulted when the program declares a `song:` arrangement.
    pub sections: Option<Vec<String>>,
    /// If this is a per-section variant (`bass@chorus:`), the section it's for.
    /// `None` = the base voice (used when no variant matches the section).
    pub section: Option<String>,
    /// Swing (`| swing N`): delays every odd 16th by a fraction of a step (0..0.9).
    pub swing: f32,
    /// Per-voice timing nudge (`| shift N` / `| nudge N`): shifts the whole voice
    /// by a fraction of a step (-0.5..0.5). Positive = laid back (late), negative = pushed.
    pub nudge: f32,
    /// Polymeter / explicit polyrhythm (`| poly …`).
    pub poly: Option<Poly>,
    /// Stereo position (`| pan N`, -1 left … 1 right). `None` = centre.
    pub pan: Option<f32>,
    /// `| jux <verb>`: play the original hard-left plus a `<verb>`-transformed copy
    /// hard-right (a stereo split). Holds the verb to apply to the right copy.
    pub jux: Option<String>,
    /// `| off N <verb>`: add a quieter copy shifted N steps later (canon/echo),
    /// optionally with `<verb>` applied. Holds `(shift_in_steps, optional_verb)`.
    pub off: Option<(f32, Option<String>)>,
    /// `| degrade N`: randomly drop a fraction (0..0.95) of this voice's hits each
    /// bar (seeded, so reproducible). 0 = no dropping.
    pub degrade: f32,
    /// `| chance N`: the whole voice plays only on a fraction (0..1) of bars
    /// (seeded). `None` = always plays.
    pub chance: Option<f32>,
    /// How the voice SOUNDS (filters, envelope, FM, samples, effects…). Shared
    /// with every event it produces, so cloning is cheap.
    pub synth: Arc<Synth>,
    /// Voix bytebeat (`nom: t: EXPR`) : l'expression de `t` (échantillons à
    /// 8 kHz depuis le début du morceau). Une telle voix ne produit AUCUN
    /// événement (`tokens` = un silence `~`) : le moteur audio la rend en continu.
    pub bytebeat: Option<Arc<Expr>>,
    /// `| sync AUTRE` : le cycle du motif repart à chaque reprise du cycle de la
    /// voix `AUTRE` (voir [`SyncTo`]). `None` sans sync, ou si la voix est inconnue.
    pub sync: Option<SyncTo>,
}

/// `| sync AUTRE` (live_loop sync de Sonic Pi, réalignement de polymétrie).
///
/// Sémantique exacte (identique dans le miroir JS) :
/// - toute voix est vue comme une suite de pas, `spb` pas par mesure : `poly N`
///   → N, `poly m:n` → m·n, une voix simple → le nombre de pas de SA mesure
///   (motif effectif, verbes appliqués). Le pas global est `s = mesure·spb + i`.
/// - le cycle du meneur `AUTRE` dure `num/den` mesures : `poly N` avec L jetons
///   → L/N ; `poly m:n` avec L jetons → L/(m·n) ; une voix simple → 1 mesure,
///   ou K mesures avec un verbe `slow K` ; une voix bytebeat → 1 mesure.
///   Ses reprises tombent aux temps `k·num/den` mesures depuis le début.
/// - sans sync, le jeton joué au pas `s` est `jetons[s mod L]` (poly) ou
///   `jetons[i]` (voix simple). Avec sync, il devient `jetons[(s − s_r) mod L]`
///   où `s_r` est le premier pas de la voix à l'instant de la dernière reprise
///   du meneur ou après (`r = ⌊s·den/(spb·num)⌋`, `s_r = ⌈r·num·spb/den⌉`) :
///   le motif repart de son premier jeton à chaque reprise du meneur.
/// - le meneur est cherché parmi toutes les voix (la variante de base d'abord) ;
///   il n'a pas besoin de jouer à cette mesure. Son propre `sync` est ignoré
///   (pas de chaîne). Une voix inconnue, ou soi-même, est ignorée sans erreur
///   (le linter du web le signale). `| sync 3` (un nombre) reste le hard sync
///   d'oscillateur : un nom de voix commence par une lettre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncTo {
    pub leader: String,
    /// Durée du cycle du meneur en mesures : `num / den`.
    pub num: u32,
    pub den: u32,
}

/// Pas local depuis la dernière reprise du meneur (voir [`SyncTo`]).
fn sync_local(s: i64, spb: i64, sy: &SyncTo) -> i64 {
    let (a, b) = (sy.num.max(1) as i64, sy.den.max(1) as i64);
    let spb = spb.max(1);
    let r = (s * b).div_euclid(spb * a);
    let sr = (r * a * spb + b - 1).div_euclid(b);
    s - sr
}

/// Durée du cycle d'une voix meneuse, en mesures `(num, den)`.
fn cycle_of(v: &Voice) -> (u32, u32) {
    if v.bytebeat.is_some() { return (1, 1); }
    let len = v.tokens.len().max(1) as u32;
    match v.poly {
        Some(Poly::Meter(n)) => (len, n.max(1)),
        Some(Poly::Grid(m, n)) => (len, (m * n).max(1)),
        None => {
            let slow = v.verbs.iter().find_map(|vb| {
                let mut w = vb.split_whitespace();
                (w.next() == Some("slow")).then(|| w.next().and_then(|x| x.parse::<i64>().ok()).unwrap_or(2).max(1))
            });
            (slow.unwrap_or(1).min(u32::MAX as i64) as u32, 1)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wave { Sine, Square, Saw, Triangle }

/// Arpeggio direction for a chord voice (`| arp up|down|updown|downup`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArpMode { Up, Down, UpDown, DownUp, Converge, Diverge, Thumb, Pinky }

fn arp_mode(w: &str) -> ArpMode {
    match w {
        "down" => ArpMode::Down,
        "updown" | "ud" => ArpMode::UpDown,
        "downup" | "du" => ArpMode::DownUp,
        "converge" | "conv" => ArpMode::Converge,
        "diverge" | "div" => ArpMode::Diverge,
        "thumb" | "thumbup" => ArpMode::Thumb,
        "pinky" | "pinkyup" => ArpMode::Pinky,
        _ => ArpMode::Up,
    }
}

/// Order a chord's notes for an arpeggio. UpDown/DownUp don't repeat the
/// turning-point notes (…g bb g eb…), so they read as a smooth bounce.
fn arp_order(notes: &[String], mode: ArpMode) -> Vec<String> {
    let rev = || notes.iter().rev().cloned().collect::<Vec<_>>();
    match mode {
        ArpMode::Up => notes.to_vec(),
        ArpMode::Down => rev(),
        ArpMode::UpDown => {
            let mut v = notes.to_vec();
            if notes.len() > 2 { v.extend(notes[1..notes.len() - 1].iter().rev().cloned()); }
            v
        }
        ArpMode::DownUp => {
            let mut v = rev();
            if notes.len() > 2 { v.extend(notes[1..notes.len() - 1].iter().cloned()); }
            v
        }
        ArpMode::Converge => {
            // outside-in: lowest, highest, 2nd lowest, 2nd highest…
            let (mut lo, mut hi, mut v) = (0usize, notes.len(), Vec::new());
            while lo < hi {
                v.push(notes[lo].clone()); lo += 1;
                if lo < hi { hi -= 1; v.push(notes[hi].clone()); }
            }
            v
        }
        ArpMode::Diverge => { let mut v = arp_order(notes, ArpMode::Converge); v.reverse(); v }
        ArpMode::Thumb => {
            // the bottom note alternates with each of the others
            let v: Vec<String> = notes.iter().skip(1).flat_map(|n| [notes[0].clone(), n.clone()]).collect();
            if v.is_empty() { notes.to_vec() } else { v }
        }
        ArpMode::Pinky => {
            // each note alternates with the top note
            let Some(top) = notes.last() else { return Vec::new() };
            let v: Vec<String> = notes[..notes.len() - 1].iter().flat_map(|n| [n.clone(), top.clone()]).collect();
            if v.is_empty() { notes.to_vec() } else { v }
        }
    }
}

impl Wave {
    fn from_word(w: &str) -> Option<Wave> {
        match w {
            "sine" => Some(Wave::Sine),
            "square" => Some(Wave::Square),
            "saw" | "sawtooth" => Some(Wave::Saw),
            "tri" | "triangle" => Some(Wave::Triangle),
            "pulse" => Some(Wave::Square), // pulse emulated as square
            _ => None,
        }
    }
}

const DRUMS: [&str; 8] = ["kick","bd","snare","sd","hat","hh","clap","cp"];

/// One scheduled sound event within a bar.
/// (`synth` is the sound design of the voice that produced it.)
#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    /// Position within the bar, 0.0..1.0 (multiply by bar length in seconds).
    pub phase: f32,
    pub voice: String,
    pub token: String,
    pub wave: Wave,
    pub gain: f32,
    pub is_drum: bool,
    /// Velocity multiplier from accents (`!`) and crescendo/decrescendo rolls (`*N>`/`*N<`).
    pub vel: f32,
    /// Exact frequency in Hz for pitched notes — carries microtonal precision that
    /// the rounded `token` name can't (e.g. quarter-tones from custom scales).
    /// `None` for drums and tokens that don't resolve to a pitch.
    pub freq: Option<f32>,
    /// Stereo pan (-1 left … 1 right) for this event's layer. `None` = centre.
    /// Set by `| pan N` and by the `jux`/`off` layers. Carries the stereo split.
    pub pan: Option<f32>,
    /// Length of the step this event occupies, as a fraction of the bar (0..1).
    /// A native synth uses it as the note's gate length.
    pub dur: f32,
    /// The voice's sound design (filters, envelopes, samples, effects…).
    pub synth: Arc<Synth>,
}

/// Names that introduce a *directive* (key/scale/seed/song/duck/feel/…), not a voice.
const DIRECTIVES: [&str; 23] = [
    "key", "scale", "seed", "song", "arrange", "duck", "feel", "scene", "prog", "tune",
    "bpm", "tempo", "meter", "accel", "groove", "title", "warmth", "film", "loop", "bus", "room", "send", "morph",
];

/// Euclidean rhythm: distribute `k` hits as evenly as possible across `n`
/// steps, then rotate left by `rot`. E(3,8) = the tresillo `x . . x . . x .`.
fn euclid(k: usize, n: usize, rot: i32) -> Vec<bool> {
    if n == 0 { return Vec::new(); }
    let k = k.min(n);
    let mut v: Vec<bool> = (0..n).map(|i| (i * k) % n < k).collect();
    let r = rot.rem_euclid(n as i32) as usize;
    v.rotate_left(r);
    v
}

/// If a token is a euclidean generator `e(k,n)` or `e(k,n,rot)`, expand it into
/// its `x`/`.` step sequence. Returns None for any other token.
fn expand_euclid_token(tok: &str) -> Option<Vec<String>> {
    let inner = tok.strip_prefix("e(")?.strip_suffix(')')?;
    let nums: Vec<&str> = inner.split(',').collect();
    if nums.len() < 2 || nums.len() > 3 { return None; }
    let k: usize = nums[0].trim().parse().ok()?;
    let n: usize = nums[1].trim().parse().ok()?;
    let rot: i32 = if nums.len() == 3 { nums[2].trim().parse().ok()? } else { 0 };
    Some(euclid(k, n, rot).into_iter()
        .map(|b| if b { "x".to_string() } else { ".".to_string() })
        .collect())
}

/// `morse(n)` -> the first n terms of the Thue-Morse sequence as x/. (a famously
/// non-repeating, self-similar rhythm: `x` when popcount(i) is odd).
fn expand_morse_token(tok: &str) -> Option<Vec<String>> {
    let n: usize = tok.strip_prefix("morse(")?.strip_suffix(')')?.trim().parse().ok()?;
    Some((0..n).map(|i| if (i as u32).count_ones() & 1 == 1 { "x".into() } else { ".".into() }).collect())
}

/// `fib(n)` -> the first n terms of the Fibonacci word as x/. (substitution
/// 0->01, 1->0), a quasi-periodic self-similar pattern.
fn expand_fib_token(tok: &str) -> Option<Vec<String>> {
    let n: usize = tok.strip_prefix("fib(")?.strip_suffix(')')?.trim().parse().ok()?;
    let mut s = vec![0u8];
    while s.len() < n {
        let mut nx = Vec::with_capacity(s.len() * 2);
        for c in &s { if *c == 0 { nx.push(0); nx.push(1); } else { nx.push(0); } }
        s = nx;
    }
    s.truncate(n);
    Some(s.iter().map(|c| if *c == 0 { "x".into() } else { ".".into() }).collect())
}

/// `prime(n)` -> hits on prime positions.
fn expand_prime_token(tok: &str) -> Option<Vec<String>> {
    let n: usize = tok.strip_prefix("prime(")?.strip_suffix(')')?.trim().parse().ok()?;
    let is_p = |k: usize| -> bool { if k < 2 { return false; } let mut i = 2; while i * i <= k { if k.is_multiple_of(i) { return false; } i += 1; } true };
    Some((0..n).map(|i| if is_p(i) { "x".into() } else { ".".into() }).collect())
}
/// `golden(n)` -> a golden-ratio (Beatty) rhythm, quasi-periodic.
fn expand_golden_token(tok: &str) -> Option<Vec<String>> {
    let n: usize = tok.strip_prefix("golden(")?.strip_suffix(')')?.trim().parse().ok()?;
    let phi = 1.618_033_988_75_f64;
    Some((0..n).map(|i| if (((i + 1) as f64) / phi).floor() != ((i as f64) / phi).floor() { "x".into() } else { ".".into() }).collect())
}
/// `collatz(n)` -> the Collatz sequence parity as a rhythm.
fn expand_collatz_token(tok: &str) -> Option<Vec<String>> {
    let start: u64 = tok.strip_prefix("collatz(")?.strip_suffix(')')?.trim().parse().ok()?;
    let mut n = start.max(1); let mut o = Vec::new(); let mut g = 0;
    while o.len() < 64 && g < 300 {
        o.push(if n % 2 == 1 { "x".to_string() } else { ".".to_string() });
        if n == 1 { break; }
        n = if n % 2 == 1 { 3 * n + 1 } else { n / 2 }; g += 1;
    }
    Some(o)
}
/// `cantor(n)` -> the Cantor set (no base-3 digit equal to 1) as a fractal rhythm.
fn expand_cantor_token(tok: &str) -> Option<Vec<String>> {
    let n: usize = tok.strip_prefix("cantor(")?.strip_suffix(')')?.trim().parse().ok()?;
    let in_c = |mut k: usize| -> bool { if k == 0 { return true; } while k > 0 { if k % 3 == 1 { return false; } k /= 3; } true };
    Some((0..n).map(|i| if in_c(i) { "x".into() } else { ".".into() }).collect())
}

/// `lsys(n)` / `l-sys(n)` — a Lindenmayer system building a self-similar
/// melodic phrase. Bare `lsys(N)` (or `l-sys(N)`) uses the classic Loom rule
/// table (bit-identical to the JS engine). The custom form
/// `l-sys(n, axiom, sym>prod, …)` defines its own starting symbols and
/// production rules (`>`, productions joined by `.`, one per symbol —
/// deterministic). Symbols are scale-degree tokens. e.g.
/// `l-sys(16, 1, 1>1.5.2, 5>3.1, 3>1)` rewrites until ≥16 then truncates.
fn expand_lsys_token(tok: &str) -> Option<Vec<String>> {
    let inner = tok.strip_prefix("l-sys(").or_else(|| tok.strip_prefix("lsys("))?.strip_suffix(')')?;
    let fields: Vec<&str> = inner.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
    let n: usize = fields.first()?.parse().ok()?;
    let mut axiom: Vec<String> = vec!["1".into()];
    let mut custom: Vec<(String, Vec<String>)> = Vec::new();
    for f in &fields[1..] {
        if let Some(at) = f.find('>') {
            let sym = f[..at].trim();
            let prod: Vec<String> = f[at + 1..].trim().split('.').filter(|s| !s.is_empty()).map(|s| s.to_string()).collect();
            if !sym.is_empty() && !prod.is_empty() { custom.push((sym.to_string(), prod)); }
        } else {
            axiom = f.split('.').map(|s| s.to_string()).collect();
        }
    }
    const DEFAULT: [(&str, &[&str]); 6] = [
        ("1", &["1", "2"]), ("2", &["3", "1"]), ("3", &["5", "2"]),
        ("5", &["6", "3"]), ("6", &["1'", "5"]), ("1'", &["3'", "1"]),
    ];
    let rule = |c: &str| -> Option<Vec<String>> {
        for (sym, prod) in &custom { if sym == c { return Some(prod.clone()); } }
        if custom.is_empty() {
            for (sym, prod) in DEFAULT { if sym == c { return Some(prod.iter().map(|s| s.to_string()).collect()); } }
        }
        None
    };
    let mut s = axiom;
    for _ in 0..8 {
        if s.len() >= n { break; }
        let mut nx: Vec<String> = Vec::new();
        for c in &s { match rule(c) { Some(r) => nx.extend(r), None => nx.push(c.clone()) } }
        if nx.len() == s.len() { break; }
        s = nx;
    }
    s.truncate(n);
    Some(s)
}
/// `pi(n)` — digits of π as scale degrees (matches JS).
fn expand_pi_token(tok: &str) -> Option<Vec<String>> {
    let n: usize = tok.strip_prefix("pi(")?.strip_suffix(')')?.trim().parse().ok()?;
    const PI: &[u8] = b"3141592653589793238462643383279502884197169399375105820974944592";
    Some((0..n).map(|i| { let d = (PI[i % PI.len()] - b'0') as i64;
        if d == 0 { ".".into() } else { ((d - 1).rem_euclid(7) + 1).to_string() } }).collect())
}
/// `recaman(n)` — Recamán's sequence as a melody (matches JS).
fn expand_recaman_token(tok: &str) -> Option<Vec<String>> {
    let n: usize = tok.strip_prefix("recaman(")?.strip_suffix(')')?.trim().parse().ok()?;
    let mut seen = std::collections::HashSet::new();
    let mut a: i64 = 0; let mut out = Vec::with_capacity(n);
    for i in 0..n {
        out.push((a.rem_euclid(7) + 1).to_string());
        seen.insert(a);
        let back = a - (i as i64 + 1);
        a = if back > 0 && !seen.contains(&back) { back } else { a + (i as i64 + 1) };
    }
    Some(out)
}
/// `logistic(n)` — the logistic map (chaos) as a melody (matches JS; pure arithmetic = bit-exact).
fn expand_logistic_token(tok: &str) -> Option<Vec<String>> {
    let n: usize = tok.strip_prefix("logistic(")?.strip_suffix(')')?.trim().parse().ok()?;
    let mut x = 0.5f64; let mut out = Vec::with_capacity(n);
    for _ in 0..n { x = 3.91 * x * (1.0 - x); out.push((((x * 7.0).floor() as i64) + 1).clamp(1, 7).to_string()); }
    Some(out)
}
/// `trib(n)` — the Tribonacci word as a rhythm (matches JS).
fn expand_trib_token(tok: &str) -> Option<Vec<String>> {
    let n: usize = tok.strip_prefix("trib(")?.strip_suffix(')')?.trim().parse().ok()?;
    let mut s = String::from("0"); let mut g = 0;
    while s.len() < n && g < 40 { let mut x = String::new();
        for c in s.chars() { match c { '0' => x.push_str("01"), '1' => x.push_str("02"), _ => x.push('0') } } s = x; g += 1; }
    Some(s.chars().take(n).map(|c| if c == '0' { "x".into() } else { ".".into() }).collect())
}
/// `bin(n)` — the binary digits of n as a rhythm (matches JS).
fn expand_bin_token(tok: &str) -> Option<Vec<String>> {
    let v: u64 = tok.strip_prefix("bin(")?.strip_suffix(')')?.trim().parse().ok()?;
    if v == 0 { return Some(vec![".".into()]); }
    Some(format!("{:b}", v).chars().map(|c| if c == '1' { "x".into() } else { ".".into() }).collect())
}
/// Expand bar/seed-INDEPENDENT generative tokens in place.
/// Shaped melodic contour (constraint composition): `arc(n)` rises to a peak and
/// falls back to the tonic (resolves), `rise(n)` ascends, `fall(n)` descends to the
/// tonic. Snapped to scale degrees 1..range (default 7). Deterministic.
fn expand_shape_token(tok: &str) -> Option<Vec<String>> {
    let (kind, rest) = ["arc", "rise", "fall"].iter()
        .find_map(|k| tok.strip_prefix(&format!("{}(", k)).map(|r| (*k, r)))?;
    let inner = rest.strip_suffix(')')?;
    let mut it = inner.split(',');
    let n: usize = it.next()?.trim().parse().ok()?;
    if n == 0 || n > 1024 { return None; }
    let range: f64 = it.next().and_then(|s| s.trim().parse::<f64>().ok()).unwrap_or(7.0).max(2.0);
    Some((0..n).map(|i| {
        let t = if n > 1 { i as f64 / (n as f64 - 1.0) } else { 0.0 };
        let c: f64 = match kind { "arc" => 1.0 - (2.0 * t - 1.0).abs(), "rise" => t, _ => 1.0 - t };
        let d = (1.0 + (range - 1.0) * c).round().clamp(1.0, range) as i64;
        d.to_string()
    }).collect())
}

/// `tonnetz(n[,start[,moves]])` -> neo-Riemannian walk: n triads from `start`
/// (e.g. C, Am, F#m) transformed by P/L/R moves (default PLR), each chord sharing
/// two notes with the next (smooth voice-leading). Emits chord symbols.
fn expand_tonnetz_token(tok: &str) -> Option<Vec<String>> {
    let inner = tok.strip_prefix("tonnetz(")?.strip_suffix(')')?;
    let mut it = inner.split(',');
    let n: usize = it.next()?.trim().parse().ok()?;
    if n == 0 || n > 256 { return None; }
    let start = it.next().map(|s| s.trim()).filter(|s| !s.is_empty()).unwrap_or("C");
    let mv: Vec<char> = it.next().map(|s| s.trim().to_uppercase()).unwrap_or_default()
        .chars().filter(|c| *c == 'P' || *c == 'L' || *c == 'R').collect();
    let mv = if mv.is_empty() { vec!['P', 'L', 'R'] } else { mv };
    let b = start.as_bytes();
    let base: i32 = match b.first()?.to_ascii_uppercase() { b'C'=>0,b'D'=>2,b'E'=>4,b'F'=>5,b'G'=>7,b'A'=>9,b'B'=>11,_=>return None };
    let mut i = 1; let mut root = base;
    match b.get(1) { Some(b'#') => { root += 1; i = 2; } Some(b'b') => { root -= 1; i = 2; } _ => {} }
    let mut minr = start[i..].to_lowercase().starts_with('m');
    root = root.rem_euclid(12);
    const NM: [&str; 12] = ["C","Db","D","Eb","E","F","Gb","G","Ab","A","Bb","B"];
    let mut out = Vec::with_capacity(n);
    for k in 0..n {
        out.push(format!("{}{}", NM[root as usize], if minr { "m" } else { "" }));
        match mv[k % mv.len()] {
            'P' => minr = !minr,
            'R' => if minr { root = (root + 3) % 12; minr = false; } else { root = (root + 9) % 12; minr = true; },
            'L' => if minr { root = (root + 8) % 12; minr = false; } else { root = (root + 4) % 12; minr = true; },
            _ => {}
        }
    }
    Some(out)
}

/// `spectrum(n[,fund])` -> the first n harmonics of `fund` (default c2) as the
/// nearest note names: the overtone series = spectral harmony (Grisey/Murail).
fn expand_spectrum_token(tok: &str) -> Option<Vec<String>> {
    fn note_midi(s: &str) -> Option<i32> {
        let b = s.as_bytes();
        let base = match b.first()?.to_ascii_lowercase() { b'c'=>0,b'd'=>2,b'e'=>4,b'f'=>5,b'g'=>7,b'a'=>9,b'b'=>11,_=>return None };
        let mut i = 1; let mut semi = base;
        match b.get(1) { Some(b'#') => { semi += 1; i = 2; } Some(b'b') => { semi -= 1; i = 2; } _ => {} }
        let oct: i32 = std::str::from_utf8(&b[i..]).ok()?.parse().ok()?;
        Some(12 * (oct + 1) + semi)
    }
    let inner = tok.strip_prefix("spectrum(")?.strip_suffix(')')?;
    let mut it = inner.split(',');
    let n: usize = it.next()?.trim().parse().ok()?;
    if n == 0 || n > 64 { return None; }
    let fund = it.next().map(|s| s.trim()).filter(|s| !s.is_empty()).unwrap_or("c2");
    let f0 = 440.0_f64 * 2f64.powf((note_midi(fund)? - 69) as f64 / 12.0);
    Some((1..=n).map(|k| {
        let freq = f0 * k as f64;
        let midi = (69.0 + 12.0 * (freq / 440.0).log2()).round() as i32;
        midi_to_name(midi)
    }).collect())
}

/// `sieve(N,M:R,M:R,…)` -> Xenakis sieve: N steps, a hit where ANY residual class
/// matches (i mod M == R). Number theory -> complex, non-periodic rhythms.
fn expand_sieve_token(tok: &str) -> Option<Vec<String>> {
    let inner = tok.strip_prefix("sieve(")?.strip_suffix(')')?;
    let mut parts = inner.split(',');
    let n: usize = parts.next()?.trim().parse().ok()?;
    if n == 0 || n > 1024 { return None; }
    let classes: Vec<(usize, usize)> = parts.filter_map(|p| {
        let (m, r) = p.split_once(':')?;
        let m: usize = m.trim().parse().ok()?;
        let r: usize = r.trim().parse().ok()?;
        if m == 0 { None } else { Some((m, r % m)) }
    }).collect();
    if classes.is_empty() { return None; }
    Some((0..n).map(|i| if classes.iter().any(|&(m, r)| i % m == r) { "x".to_string() } else { ".".to_string() }).collect())
}

/// World-rhythm bank: named timelines (clave, tresillo, bembe…) expand to x/.
/// patterns. The SAME table lives in the JS engine, so parity is automatic.
pub fn world_rhythm(name: &str) -> Option<&'static str> {
    Some(match name {
        "tresillo"  => "x . . x . . x .",
        "cinquillo" => "x . x x . x x .",
        "son"       => "x . . x . . x . . . x . x . . .",
        "rumba"     => "x . . x . . . x . . x . x . . .",
        "bossa"     => "x . . x . . x . . . x . . x . .",
        "bembe"     => "x . x x . x . x x . x .",
        "baiao"     => "x . . x . . x . x . . x . . x .",
        "maqsum"    => "x x . x x . x .",
        "ayoub"     => "x . . x x . x .",
        "samba"     => "x . x . . x . x x . x . . x . x",
        "montuno"   => ". . x . . x . . . . x . . x . .",
        "shiko"     => "x . . . x . x . . . x . x . . .",
        _ => return None,
    })
}
fn expand_rhythm_token(tok: &str) -> Option<Vec<String>> {
    world_rhythm(tok).map(|p| p.split_whitespace().map(|s| s.to_string()).collect())
}

fn expand_euclid(tokens: Vec<String>) -> Vec<String> {
    let mut out = Vec::with_capacity(tokens.len());
    for t in tokens {
        if let Some(seq) = expand_euclid_token(&t) { out.extend(seq); }
        else if let Some(seq) = expand_morse_token(&t) { out.extend(seq); }
        else if let Some(seq) = expand_fib_token(&t) { out.extend(seq); }
        else if let Some(seq) = expand_prime_token(&t) { out.extend(seq); }
        else if let Some(seq) = expand_golden_token(&t) { out.extend(seq); }
        else if let Some(seq) = expand_collatz_token(&t) { out.extend(seq); }
        else if let Some(seq) = expand_cantor_token(&t) { out.extend(seq); }
        else if let Some(seq) = expand_lsys_token(&t) { out.extend(seq); }
        else if let Some(seq) = expand_pi_token(&t) { out.extend(seq); }
        else if let Some(seq) = expand_recaman_token(&t) { out.extend(seq); }
        else if let Some(seq) = expand_logistic_token(&t) { out.extend(seq); }
        else if let Some(seq) = expand_trib_token(&t) { out.extend(seq); }
        else if let Some(seq) = expand_bin_token(&t) { out.extend(seq); }
        else if let Some(seq) = expand_rhythm_token(&t) { out.extend(seq); }
        else if let Some(seq) = expand_sieve_token(&t) { out.extend(seq); }
        else if let Some(seq) = expand_shape_token(&t) { out.extend(seq); }
        else if let Some(seq) = expand_spectrum_token(&t) { out.extend(seq); }
        else if let Some(seq) = expand_tonnetz_token(&t) { out.extend(seq); }
        else { out.push(t); }
    }
    out
}

/// Parse LOOM source text into voices.
/// Strip an end-of-line comment: a `#` preceded by whitespace (or at line start)
/// begins a comment. Preserves `7#`/`f#` accidentals (no space before the `#`).
pub(crate) fn decomment(s: &str) -> &str {
    let b = s.as_bytes();
    for i in 0..b.len() {
        if b[i] == b'#' && (i == 0 || b[i - 1] == b' ' || b[i - 1] == b'\t') { return &s[..i]; }
    }
    s
}

pub fn parse(src: &str) -> Vec<Voice> {
    let expanded = expand_macros(src);
    let src = expanded.as_str();
    let mut out = Vec::new();
    let mut names_sync: Vec<Option<String>> = Vec::new();
    for raw in src.lines() {
        let line = decomment(raw).trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        let Some(colon) = line.find(':') else { continue };
        let full = line[..colon].trim().to_lowercase();
        if DIRECTIVES.contains(&full.as_str()) { continue; }
        // `@voix.Param:` = automation Live, `go:` = scène Live (voir `live`) : pas des voix
        if full.starts_with('@') { continue; }
        // `name@section` = a per-section variant of the voice `name`
        let (name, section) = match full.split_once('@') {
            Some((b, s)) => (b.to_string(), Some(s.to_string())),
            None => (full, None),
        };
        // skip directive@section lines too (e.g. key@chorus, scene@drop) — not voices
        if DIRECTIVES.contains(&name.as_str()) || name == "go" { continue; }
        let rest = &line[colon + 1..];
        // voix bytebeat : `nom: t: EXPR | verbes`. L'expression peut contenir des
        // `|` (OU binaire), d'où la coupe dédiée (voir `expr::split_bytebeat`).
        let mut bytebeat = None;
        let rest = match rest.trim_start().strip_prefix("t:") {
            Some(body) => {
                let (ex, tail) = expr::split_bytebeat(body);
                match Expr::parse(ex, &['t']) {
                    Ok(e) => bytebeat = Some(Arc::new(e)),
                    Err(_) => continue, // expression invalide : la voix est ignorée
                }
                format!("~{tail}")
            }
            None => rest.to_string(),
        };
        let mut segs = split_bars(&rest).into_iter().map(|s| s.trim());

        let pattern = segs.next().unwrap_or("");
        // stacks: `;` splits the pattern into simultaneous lines
        let stacks: Vec<Vec<String>> = pattern.split(';')
            .map(|p| expand_euclid(tokenize_pat(p)))
            .filter(|t| !t.is_empty())
            .collect();
        if stacks.is_empty() { continue; }
        let tokens = stacks[0].clone();

        let is_drum = DRUMS.contains(&name.as_str());
        let mut wave = Wave::Square;
        let mut gain = 0.22_f32;
        let mut octave = None;
        let mut arp = None;
        let mut sections: Option<Vec<String>> = None;
        let mut swing = 0.0_f32;
        let mut nudge = 0.0_f32;
        let mut poly = None;
        let mut pan = None;
        let mut jux = None;
        let mut off = None;
        let mut degrade = 0.0_f32;
        let mut chance = None;
        let mut sync_name: Option<String> = None;
        let mut synth = Synth::default();
        let mut verbs = Vec::new();

        for seg in segs {
            if seg.is_empty() { continue; }
            let mut parts = seg.split_whitespace();
            let head = parts.next().unwrap_or("");
            if let Some(w) = Wave::from_word(head) {
                wave = w;
            } else if head == "gain" {
                if let Some(g) = parts.next().and_then(|x| x.parse::<f32>().ok()) {
                    gain = g.clamp(0.0, 1.0);
                }
            } else if head == "oct" {
                octave = parts.next().and_then(|x| x.parse::<i32>().ok());
            } else if head == "arp" {
                arp = Some(arp_mode(parts.next().unwrap_or("up")));
                if let Some(n) = parts.next().and_then(|x| x.parse::<u32>().ok()).filter(|n| *n > 1) {
                    synth.arp_octaves = Some(n.min(4));
                }
            } else if head == "swing" {
                if let Some(mut nn) = parts.next().and_then(|x| x.parse::<f32>().ok()) {
                    if nn > 1.0 { nn /= 100.0; }
                    swing = nn.clamp(0.0, 0.9);
                }
            } else if head == "shift" || head == "nudge" {
                if let Some(nn) = parts.next().and_then(|x| x.parse::<f32>().ok()) {
                    nudge = nn.clamp(-0.5, 0.5);
                }
            } else if head == "poly" {
                let spec = parts.next().unwrap_or("16");
                poly = Some(if let Some((m, n)) = spec.split_once(':') {
                    let m = m.trim().parse::<u32>().ok().filter(|m| *m > 0).unwrap_or(1);
                    let n = n.trim().parse::<u32>().ok().filter(|n| *n > 0).unwrap_or(1);
                    Poly::Grid(m, n)
                } else {
                    Poly::Meter(spec.parse::<u32>().ok().unwrap_or(16).max(1))
                });
            } else if head == "pan" {
                // a plain number is the layer pan; sweeps/signals live in the synth
                pan = parts.next().and_then(|x| x.parse::<f32>().ok()).map(|p| p.clamp(-1.0, 1.0));
                synth.apply_segment(seg, &mut wave);
            } else if head == "jux" {
                let verb = parts.collect::<Vec<_>>().join(" ");
                if !verb.is_empty() { jux = Some(verb); }
            } else if head == "off" {
                let n = parts.next().and_then(|x| x.parse::<f32>().ok()).unwrap_or(1.0);
                let verb = parts.collect::<Vec<_>>().join(" ");
                off = Some((n, if verb.is_empty() { None } else { Some(verb) }));
            } else if head == "degrade" {
                let mut nn = parts.next().and_then(|x| x.parse::<f32>().ok()).unwrap_or(50.0);
                if nn > 1.0 { nn /= 100.0; }
                degrade = nn.clamp(0.0, 0.95);
            } else if head == "chance" {
                let c2 = parts.next().and_then(|x| x.parse::<f32>().ok()).unwrap_or(0.5);
                chance = Some((if c2 > 1.0 { c2 / 100.0 } else { c2 }).clamp(0.0, 1.0));
            } else if head == "sync" && seg.split_whitespace().nth(1).is_some_and(|w| w.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')) {
                // `| sync AUTRE` : une voix (un nombre = le hard sync du synthé)
                sync_name = parts.next().map(|w| w.to_lowercase());
            } else if head == "in" {
                let secs: Vec<String> = parts.map(|s| s.to_lowercase()).collect();
                if !secs.is_empty() { sections = Some(secs); }
            } else if !synth.apply_segment(seg, &mut wave) {
                verbs.push(seg.to_string());
            }
        }
        let synth = Arc::new(synth);
        out.push(Voice { name, tokens, stacks, verbs, wave, gain, is_drum, octave, arp, sections, section, swing, nudge, poly, pan, jux, off, degrade, chance, synth, bytebeat, sync: None });
        names_sync.push(sync_name);
    }
    // `| sync AUTRE` : résolu quand toutes les voix sont connues
    for k in 0..out.len() {
        let Some(name) = &names_sync[k] else { continue };
        if *name == out[k].name { continue; }
        let leader = out.iter().find(|w| w.name == *name && w.section.is_none())
            .or_else(|| out.iter().find(|w| w.name == *name));
        if let Some(l) = leader {
            let (num, den) = cycle_of(l);
            out[k].sync = Some(SyncTo { leader: name.clone(), num, den });
        }
    }
    out
}

/// Apply a single verb to a token list for a given bar index. `seed` feeds the
/// probabilistic verbs (`sometimes`/`someby`/…) so they replay deterministically.
fn apply_verb(tokens: &[String], verb: &str, bar: i64, seed: u64) -> Vec<String> {
    let parts: Vec<&str> = verb.split_whitespace().collect();
    match parts.first().copied() {
        Some("rev") => tokens.iter().rev().cloned().collect(),
        Some("fast") => {
            let n = parts.get(1).and_then(|x| x.parse::<usize>().ok()).unwrap_or(2).max(1);
            let mut r = Vec::with_capacity(tokens.len() * n);
            for _ in 0..n { r.extend(tokens.iter().cloned()); }
            r
        }
        Some("slow") => {
            let n = parts.get(1).and_then(|x| x.parse::<i64>().ok()).unwrap_or(2).max(1);
            if bar.rem_euclid(n) == 0 { tokens.to_vec() }
            else { tokens.iter().map(|_| ".".to_string()).collect() }
        }
        Some("every") => {
            let n = parts.get(1).and_then(|x| x.parse::<i64>().ok()).unwrap_or(2).max(1);
            if bar.rem_euclid(n) == n - 1 {
                let inner = parts[2..].join(" ");
                apply_verb(tokens, &inner, bar, seed)
            } else { tokens.to_vec() }
        }
        // probabilistic verbs: apply the inner verb on a fraction of bars (seeded)
        Some(w @ ("sometimes" | "often" | "rarely" | "someby")) => {
            let (p, skip) = match w {
                "often" => (0.75, 1),
                "rarely" => (0.25, 1),
                "someby" => (parts.get(1).and_then(|x| x.parse::<f32>().ok()).unwrap_or(50.0) / 100.0, 2),
                _ => (0.5, 1),
            };
            if rand01(seed, bar, verb, 0) < p {
                apply_verb(tokens, &parts[skip..].join(" "), bar, seed)
            } else { tokens.to_vec() }
        }
        // Tidal-style transforms:
        Some("palindrome") => {
            // forwards on even bars, reversed on odd bars
            if bar.rem_euclid(2) == 0 { tokens.to_vec() }
            else { tokens.iter().rev().cloned().collect() }
        }
        Some("rot") => {
            // static rotation: shift the pattern left by N steps (N<0 = right)
            if tokens.is_empty() { return tokens.to_vec(); }
            let n = parts.get(1).and_then(|x| x.parse::<i64>().ok()).unwrap_or(1);
            let k = n.rem_euclid(tokens.len() as i64) as usize;
            let mut r = tokens.to_vec();
            r.rotate_left(k);
            r
        }
        Some("iter") => {
            // shift the pattern start one step further each bar, looping after N
            if tokens.is_empty() { return tokens.to_vec(); }
            let n = parts.get(1).and_then(|x| x.parse::<i64>().ok()).unwrap_or(4).max(1);
            let k = (bar.rem_euclid(n)).rem_euclid(tokens.len() as i64) as usize;
            let mut r = tokens.to_vec();
            r.rotate_left(k);
            r
        }
        Some("chunk") => {
            // split into N chunks, apply the inner verb to a DIFFERENT chunk each bar
            if tokens.is_empty() { return tokens.to_vec(); }
            let n = parts.get(1).and_then(|x| x.parse::<i64>().ok()).unwrap_or(4).max(1) as usize;
            let w = bar.rem_euclid(n as i64) as usize;
            let part = (tokens.len() / n).max(1);
            let s = (w * part).min(tokens.len());
            let e = if w == n - 1 { tokens.len() } else { (s + part).min(tokens.len()) };
            let inner = parts[2..].join(" ");
            let mut r = tokens[..s].to_vec();
            r.extend(apply_verb(&tokens[s..e], &inner, bar, seed));
            r.extend_from_slice(&tokens[e..]);
            r
        }
        Some("add" | "trans") => {
            // diatonic transpose of every scale-degree token by N steps
            let n = parts.get(1).and_then(|x| x.parse::<i64>().ok()).unwrap_or(0);
            tokens.iter().map(|t| transpose_deg(t, n)).collect()
        }
        Some("mirror") => {
            let mut r = tokens.to_vec();
            r.extend(tokens.iter().rev().cloned());
            r
        }
        Some("thin") => {
            let n = parts.get(1).and_then(|x| x.parse::<usize>().ok()).unwrap_or(2).max(2);
            tokens.iter().enumerate()
                .map(|(i, t)| if i % n == 0 { t.clone() } else { ".".to_string() }).collect()
        }
        Some("accent") => {
            let n = parts.get(1).and_then(|x| x.parse::<usize>().ok()).unwrap_or(4).max(1);
            tokens.iter().enumerate()
                .map(|(i, t)| if i % n == 0 && t != "." && t != "~" && !t.contains('!') { format!("{}!", t) } else { t.clone() })
                .collect()
        }
        // generative variation (seeded — matches the JS engine bit-for-bit)
        Some("mutate") | Some("mut") => {
            let amt = parts.get(1).and_then(|x| x.parse::<f64>().ok()).unwrap_or(25.0) / 100.0;
            let first_non_rest = tokens.iter().find(|t| t.as_str() != ".").cloned();
            tokens.iter().enumerate().map(|(i, t)| {
                if js_rng(seed as u32, bar, &format!("mut:{}", verb), i as i64) >= amt { return t.clone(); }
                let r = js_rng(seed as u32, bar, &format!("mut2:{}", verb), i as i64);
                let deg0 = t.as_bytes().first().map(|&c| (b'1'..=b'7').contains(&c)).unwrap_or(false);
                if deg0 { let st = [-2i64, -1, 1, 2]; return transpose_deg(t, st[((r * 4.0).floor() as i64).rem_euclid(4) as usize]); }
                if t == "." { return first_non_rest.clone().unwrap_or_else(|| t.clone()); }
                t.clone()
            }).collect()
        }
        Some("stutter") | Some("stut") => {
            // beat-repeat: loop the first 1/N of the pattern across the bar
            let n = parts.get(1).and_then(|x| x.parse::<usize>().ok()).filter(|n| *n != 0).unwrap_or(4).max(2);
            let slice = &tokens[..(tokens.len() / n).max(1).min(tokens.len())];
            slice.iter().cycle().take(tokens.len()).cloned().collect()
        }
        Some("shuffle") | Some("shuf") => {
            let mut a = tokens.to_vec();
            let mut i = a.len();
            while i > 1 { i -= 1; let j = (js_rng(seed as u32, bar, &format!("shuf:{}", verb), i as i64) * ((i + 1) as f64)).floor() as usize; a.swap(i, j); }
            a
        }
        Some("markov") => {
            if tokens.len() < 2 { return tokens.to_vec(); }
            // markov [N] [learn: <phrase…>] — chaîne d'ordre N ; avec `learn:` la
            // chaîne est APPRISE de la phrase (un morceau existant) au lieu de la
            // voix elle-même, mais la ligne générée garde la longueur de la voix.
            let mut src: Vec<String> = tokens.to_vec();
            let mut ord = 1usize;
            if parts.get(1).copied() == Some("learn:") {
                let ls: Vec<String> = parts[2..].iter().map(|s| s.to_string()).collect();
                if ls.len() >= 2 { src = ls; }
            } else {
                ord = parts.get(1).and_then(|x| x.parse::<usize>().ok()).unwrap_or(1).max(1);
                if parts.get(2).copied() == Some("learn:") {
                    let ls: Vec<String> = parts[3..].iter().map(|s| s.to_string()).collect();
                    if ls.len() >= 2 { src = ls; }
                }
            }
            if src.len() < 2 { return tokens.to_vec(); }
            // markov N: order-N (context = last N tokens). N>=2 only; N=1 keeps the original.
            if ord >= 2 {
                let o = ord.min(src.len() - 1);
                let mut tr: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
                for i in 0..src.len() {
                    let key = (0..o).map(|j| src[(i + j) % src.len()].clone()).collect::<Vec<_>>().join(",");
                    tr.entry(key).or_default().push(src[(i + o) % src.len()].clone());
                }
                let mut out = src[0..o].to_vec();
                let mut ctx = src[0..o].to_vec();
                for i in o..tokens.len() {
                    let key = ctx.join(",");
                    let opts = tr.get(&key).cloned().unwrap_or_else(|| src.to_vec());
                    let idx = ((js_rng(seed as u32, bar, &format!("mk:{}", verb), i as i64) * (opts.len() as f64)).floor() as usize) % opts.len();
                    out.push(opts[idx].clone()); ctx.push(opts[idx].clone()); ctx.remove(0);
                }
                return out;
            }
            let mut trans: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
            for i in 0..src.len() { let a = src[i].clone(); let b = src[(i + 1) % src.len()].clone(); trans.entry(a).or_default().push(b); }
            let mut out = Vec::with_capacity(tokens.len());
            let mut cur = src[0].clone();
            for i in 0..tokens.len() {
                out.push(cur.clone());
                let opts = trans.get(&cur).cloned().unwrap_or_else(|| src.to_vec());
                let idx = ((js_rng(seed as u32, bar, &format!("mk:{}", verb), i as i64) * (opts.len() as f64)).floor() as usize) % opts.len();
                cur = opts[idx].clone();
            }
            out
        }
        // melodic inversion (mirror degrees around the first note) + sequence
        Some("invert") | Some("inv") => {
            let deg = |t: &str| -> Option<i64> { let b = t.as_bytes(); if b.is_empty() || !(b'1'..=b'7').contains(&b[0]) { return None; }
                let mut o = 0i64; for &c in &b[1..] { if c == b'\'' { o += 1; } else if c == b',' { o -= 1; } else { break; } } Some((b[0] - b'1') as i64 + o * 7) };
            let pivot = match tokens.iter().find_map(|t| deg(t)) { Some(p) => p, None => return tokens.to_vec() };
            tokens.iter().map(|t| { let b = t.as_bytes(); if b.is_empty() || !(b'1'..=b'7').contains(&b[0]) { return t.clone(); }
                let mut o = 0i64; let mut k = 1; while k < b.len() && (b[k] == b'\'' || b[k] == b',') { if b[k] == b'\'' { o += 1; } else { o -= 1; } k += 1; }
                let d = (b[0] - b'1') as i64 + o * 7; let nd = 2 * pivot - d; let base = nd.rem_euclid(7); let oc = (nd as f64 / 7.0).floor() as i64;
                let mut s = String::new(); s.push((b'1' + base as u8) as char); for _ in 0..oc.abs() { s.push(if oc > 0 { '\'' } else { ',' }); }
                s.push_str(std::str::from_utf8(&b[k..]).unwrap_or("")); s }).collect()
        }
        Some("sequence") | Some("seq") => {
            let stp = parts.get(1).and_then(|x| x.parse::<i64>().ok()).unwrap_or(2);
            let mut r = tokens.to_vec(); r.extend(tokens.iter().map(|t| transpose_deg(t, stp))); r
        }
        // complement rhythm (tiling-canon primitive): hits<->rests. A voice + its
        // | hocket copy tile the bar — every step covered once (interlock).
        Some("hocket") | Some("complement") =>
            tokens.iter().map(|t| if t == "." || t == "~" { "x".to_string() } else { ".".to_string() }).collect(),
        // Tidal verbs (mirrored op-for-op in web/index.html applyOne):
        // ply N: each event repeated N times inside its own step
        Some("ply") => {
            let n = parts.get(1).and_then(|x| x.parse::<usize>().ok()).unwrap_or(2).clamp(1, 16);
            if n == 1 { return tokens.to_vec(); }
            tokens.iter().map(|t| if t == "." || t == "~" { t.clone() }
                else { format!("[{}]", vec![t.as_str(); n].join(" ")) }).collect()
        }
        // linger F: loop the first fraction F of the bar (F<0: the last |F|)
        Some("linger") => {
            if tokens.is_empty() { return tokens.to_vec(); }
            let f = parts.get(1).and_then(|x| x.parse::<f64>().ok()).filter(|f| f.is_finite() && *f != 0.0).unwrap_or(0.25);
            if f.abs() >= 1.0 { return tokens.to_vec(); }
            let len = tokens.len();
            let k = ((len as f64 * f.abs() + 0.5).floor() as usize).clamp(1, len);
            let slice = if f > 0.0 { &tokens[..k] } else { &tokens[len - k..] };
            slice.iter().cycle().take(len).cloned().collect()
        }
        // segment N: resample the line into N even steps (nearest earlier token)
        Some("segment") => {
            if tokens.is_empty() { return tokens.to_vec(); }
            let len = tokens.len();
            let n = parts.get(1).and_then(|x| x.parse::<usize>().ok()).unwrap_or(len).clamp(1, 64);
            (0..n).map(|i| tokens[i * len / n].clone()).collect()
        }
        // range A B: rescale the pattern's lowest..highest degree onto A..B
        Some("range") => {
            let (Some(a), Some(b)) = (parts.get(1).and_then(|x| range_bound(x)), parts.get(2).and_then(|x| range_bound(x)))
                else { return tokens.to_vec() };
            let idx: Vec<Option<(i64, &str)>> = tokens.iter().map(|t| deg_index(t)).collect();
            let lo = idx.iter().flatten().map(|d| d.0).min();
            let hi = idx.iter().flatten().map(|d| d.0).max();
            let (Some(lo), Some(hi)) = (lo, hi) else { return tokens.to_vec() };
            tokens.iter().zip(idx).map(|(t, d)| match d {
                None => t.clone(),
                Some((i, mods)) => {
                    let ni = if hi == lo { a } else { a + ((i - lo) * (b - a) * 2 + (hi - lo)).div_euclid(2 * (hi - lo)) };
                    deg_from_index(ni, mods)
                }
            }).collect()
        }
        // motivic development: an arc of transformations picked by the bar, so a
        // section EVOLVES instead of repeating. Reuses parity-tested verbs.
        Some("develop") | Some("dev") => {
            const CY: [&str; 8] = ["", "rot 1", "add 2", "rev", "add -2", "rot 3", "invert", "add 4"];
            let op = CY[bar.rem_euclid(CY.len() as i64) as usize];
            if op.is_empty() { tokens.to_vec() } else { apply_verb(tokens, op, bar, seed) }
        }
        _ => tokens.to_vec(),
    }
}

/// `range` helper: a plain scale degree token (`5`, `3'`, `2,` plus `?*!`/digit
/// mods) as a 0-based diatonic index with its trailing mods. Same grammar as JS.
fn deg_index(tok: &str) -> Option<(i64, &str)> {
    let b = tok.as_bytes();
    if b.is_empty() || !(b'1'..=b'7').contains(&b[0]) { return None; }
    let mut o = 0i64;
    let mut k = 1;
    while k < b.len() && (b[k] == b'\'' || b[k] == b',') { o += if b[k] == b'\'' { 1 } else { -1 }; k += 1; }
    let mods = &tok[k..];
    if !mods.bytes().all(|c| matches!(c, b'?' | b'*' | b'!' | b'0'..=b'9')) { return None; }
    Some(((b[0] - b'1') as i64 + o * 7, mods))
}

/// `range A B` bound: an integer degree (`8` = 1 an octave up) or a degree token (`5'`).
fn range_bound(s: &str) -> Option<i64> {
    if let Ok(n) = s.parse::<i64>() { return Some(n.clamp(-27, 29) - 1); }
    deg_index(s).filter(|d| d.1.is_empty()).map(|d| d.0)
}

/// 0-based diatonic index back to a degree token (`'`/`,` octave marks) + mods.
fn deg_from_index(i: i64, mods: &str) -> String {
    let oc = i.div_euclid(7);
    let mut s = (i.rem_euclid(7) + 1).to_string();
    for _ in 0..oc.abs() { s.push(if oc > 0 { '\'' } else { ',' }); }
    s.push_str(mods);
    s
}

/// Diatonically transpose one degree token by `n` scale-steps (octave wrap, marks/mods kept).
/// Leaves notes/chords/accidentals untouched (mirrors the web engine's transposeDeg).
fn transpose_deg(tok: &str, n: i64) -> String {
    if n == 0 { return tok.to_string(); }
    let bytes = tok.as_bytes();
    if bytes.is_empty() { return tok.to_string(); }
    let d0 = (bytes[0] as char).to_digit(10);
    let deg = match d0 { Some(d) if (1..=7).contains(&d) => d as i64, _ => return tok.to_string() };
    // parse octave marks ' (up) , (down); collect any trailing mods (?* digits, !)
    let mut octs: i64 = 0;
    let mut mods = String::new();
    for &c in &bytes[1..] {
        match c {
            b'\'' => octs += 1,
            b',' => octs -= 1,
            b'?' | b'*' | b'!' => { mods.push(c as char); }
            b'0'..=b'9' if !mods.is_empty() => { mods.push(c as char); }
            _ => return tok.to_string(), // accidental / unexpected -> leave as-is
        }
    }
    let d = deg - 1 + n;
    let nd = d.rem_euclid(7) + 1;
    octs += d.div_euclid(7);
    let mut out = nd.to_string();
    for _ in 0..octs.abs() { out.push(if octs > 0 { '\'' } else { ',' }); }
    out.push_str(&mods);
    out
}

/// One generation of an elementary cellular automaton (Wolfram `rule`, `n`
/// cells, toroidal), evolved `gen` steps from a single centre seed. Each bar
/// advances the generation, so a `ca(rule,n)` pattern evolves over time.
fn ca_row(rule: u8, n: usize, gen: i64) -> Vec<bool> {
    let mut row = vec![false; n];
    row[n / 2] = true;
    let steps = gen.rem_euclid(1 << 20) as usize; // safety bound
    for _ in 0..steps {
        let prev = row.clone();
        for i in 0..n {
            let l = prev[(i + n - 1) % n] as u8;
            let c = prev[i] as u8;
            let r = prev[(i + 1) % n] as u8;
            let idx = (l << 2) | (c << 1) | r;
            row[i] = ((rule >> idx) & 1) == 1;
        }
    }
    row
}

/// Expand a bar-aware generative token `ca(rule,n)` into its `x`/`.` row.
fn expand_generative_token(tok: &str, bar: i64) -> Option<Vec<String>> {
    let inner = tok.strip_prefix("ca(")?.strip_suffix(')')?;
    let parts: Vec<&str> = inner.split(',').collect();
    if parts.is_empty() || parts.len() > 2 { return None; }
    let rule: u8 = parts[0].trim().parse().ok()?;
    let n: usize = parts.get(1).and_then(|s| s.trim().parse().ok()).unwrap_or(16).max(1);
    Some(ca_row(rule, n, bar).into_iter().map(|b| if b { "x".into() } else { ".".into() }).collect())
}

/// `walk(n[,range])` — seeded random-walk melody (matches the JS engine bit-for-bit).
fn expand_walk_token(tok: &str, bar: i64, seed: u64) -> Option<Vec<String>> {
    let inner = tok.strip_prefix("walk(")?.strip_suffix(')')?;
    let mut it = inner.split(',');
    let n: usize = it.next()?.trim().parse().ok()?;
    let range: i64 = it.next().and_then(|s| s.trim().parse().ok()).unwrap_or(7).max(2);
    let steps = [-2i64, -1, -1, 1, 1, 2];
    let mut d: i64 = 1;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        out.push(d.clamp(1, range).to_string());
        let r = js_rng(seed as u32, bar, &format!("walk:{}", i), i as i64);
        let st = steps[((r * 6.0).floor() as i64).rem_euclid(6) as usize];
        d = (d + st).clamp(1, range);
    }
    Some(out)
}
/// `perlin(n)` — smooth value-noise melody (seed-dependent; uses sin, so degree-level not bit-exact).
fn expand_perlin_token(tok: &str, seed: u64) -> Option<Vec<String>> {
    let n: usize = tok.strip_prefix("perlin(")?.strip_suffix(')')?.trim().parse().ok()?;
    let h = |k: i64| -> f64 { let s = ((k as f64 + 1.0) * 12.9898 + (seed as f64) * 0.137).sin() * 43758.5453; s - s.floor() };
    let f = (n as f64 / 6.0).max(2.0);
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let x = i as f64 / f; let x0 = x.floor(); let tt = x - x0; let sm = tt * tt * (3.0 - 2.0 * tt);
        let val = h(x0 as i64) * (1.0 - sm) + h(x0 as i64 + 1) * sm;
        out.push((((val * 7.0).floor() as i64) + 1).clamp(1, 7).to_string());
    }
    Some(out)
}
/// `drunk(n)` — drifting-density rhythm (matches the JS engine bit-for-bit).
fn expand_drunk_token(tok: &str, bar: i64, seed: u64) -> Option<Vec<String>> {
    let n: usize = tok.strip_prefix("drunk(")?.strip_suffix(')')?.trim().parse().ok()?;
    let mut p = 0.5f64;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let r = js_rng(seed as u32, bar, "drunk", i as i64);
        out.push(if r < p { "x".to_string() } else { ".".to_string() });
        p = (p + (r - 0.5) * 0.45).clamp(0.15, 0.85);
    }
    Some(out)
}
/// `pink(n)` / `1f(n)` — 1/f (pink) noise melody (Voss): sum of octaves that hold
/// for 2^k steps -> a fractal, "natural" contour. Bit-identical to the JS engine.
fn expand_1f_token(tok: &str, bar: i64, seed: u64) -> Option<Vec<String>> {
    let inner = tok.strip_prefix("pink(").or_else(|| tok.strip_prefix("1f("))?.strip_suffix(')')?;
    let mut it = inner.split(',');
    let n: usize = it.next()?.trim().parse().ok()?;
    let range: i64 = it.next().and_then(|s| s.trim().parse().ok()).unwrap_or(7).max(2);
    let b = 5usize;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let mut s = 0.0f64;
        for k in 0..b {
            let block = (i / (1usize << k)) as i64;
            s += js_rng(seed as u32, bar, &format!("1f:{}", k), block);
        }
        let v = s / b as f64;
        out.push((((v * range as f64).floor() as i64) + 1).clamp(1, range).to_string());
    }
    Some(out)
}

/// `gendy(n)` — Xenakis dynamic-stochastic (GENDYN): a random walk that REFLECTS
/// off the scale boundaries (triangle fold), giving a bouncing contour. Seeded;
/// bit-identical to the JS engine.
fn expand_gendy_token(tok: &str, bar: i64, seed: u64) -> Option<Vec<String>> {
    let inner = tok.strip_prefix("gendy(")?.strip_suffix(')')?;
    let mut it = inner.split(',');
    let n: usize = it.next()?.trim().parse().ok()?;
    let range: f64 = it.next().and_then(|s| s.trim().parse::<f64>().ok()).unwrap_or(7.0).max(2.0);
    let span = (range - 1.0).max(1.0);
    let mut d = 1.0 + span / 2.0;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        out.push((d.round() as i64).clamp(1, range as i64).to_string());
        d += (js_rng(seed as u32, bar, "gendy", i as i64) - 0.5) * 3.0;
        let m = 2.0 * span;
        let t = (d - 1.0).rem_euclid(m);
        d = 1.0 + if t <= span { t } else { m - t };
    }
    Some(out)
}

/// Random in strict sequence (`tick` croît à chaque tirage) — le même dans les
/// 2 moteurs, pour que xys/evolve soient bit-identiques JS↔Rust.
fn rng_seq(seed: u32, bar: i64, name: &str, tick: &mut i64) -> f64 {
    let v = js_rng(seed, bar, name, *tick);
    *tick += 1;
    v
}

/// `xys(n, root, sym>a:p|b:q, …)` — MARCHE DE MARKOV (Xenakis, les
/// « probabilités de transition » d'Achorripsis) : depuis `root`, chaque note
/// émet le symbole courant puis tire une transition `a:p|b:q` (poids `:p`) par
/// `rng` seedé — exactement `n` notes, bit-identique JS. Un symbole sans sortie
/// reste sur place (état absorbant / drone). e.g.
/// `xys(16, 1, 1>2:2|3:1, 2>1:1, 5>1.5:2, 3>2:1)`.
fn expand_xys_token(tok: &str, bar: i64, seed: u64) -> Option<Vec<String>> {
    let inner = tok.strip_prefix("xys(")?.strip_suffix(')')?;
    let fields: Vec<&str> = inner.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
    let n: usize = fields.first()?.parse().ok()?;
    if n == 0 { return Some(Vec::new()); }
    let root = fields.get(1).and_then(|s| s.split('.').next()).filter(|s| !s.is_empty()).unwrap_or("1").to_string();
    let mut table: Vec<(String, Vec<(String, i64)>)> = Vec::new();
    for f in &fields[2..] {
        let Some(at) = f.find('>') else { continue };
        let sym = f[..at].trim();
        if sym.is_empty() { continue; }
        let mut alts: Vec<(String, i64)> = Vec::new();
        for alt in f[at + 1..].split('|') {
            let (sym2, weight) = match alt.rsplit_once(':') {
                Some((p, w)) => (p, w.trim().parse::<i64>().ok().unwrap_or(1).max(1)),
                None => (alt, 1),
            };
            let sym2 = sym2.trim();
            if sym2.is_empty() { continue; }
            alts.push((sym2.to_string(), weight));
        }
        if !alts.is_empty() { table.push((sym.to_string(), alts)); }
    }
    let mut cur = root;
    let mut out = Vec::with_capacity(n);
    let mut tick: i64 = 0;
    for _ in 0..n {
        out.push(cur.clone());
        let alts = table.iter().find(|(s2, _)| *s2 == cur);
        if let Some((_, alts)) = alts {
            let total: i64 = alts.iter().map(|a| a.1).sum::<i64>().max(1);
            let mut acc = 0.0f64;
            let u2 = rng_seq(seed as u32, bar, "xys", &mut tick);
            let mut picked = &alts[alts.len() - 1].0;
            for (sym2, w) in alts {
                acc += *w as f64 / total as f64;
                if u2 < acc { picked = sym2; break; }
            }
            cur = picked.clone();
        }
    }
    Some(out)
}

/// `evolve(n[,gen][,pop])` — MÉLODIE GÉNÉTIQUE (seedée, bit-identique JS) : une
/// petite population aléatoire de séquences de degrés concourt sur une FITNESS
/// entière (contour lisse + attraction vers la tonique/quinte + variété) ; les
/// meilleurs se reproduisent par crossover point-simple + mutation 20 %, et le
/// plus apte de longueur `n` survit. Chaque tirage utilise `rng(seed,bar,"evolve",k)`
/// avec un compteur strictement croissant — les 2 moteurs tirent dans le même ordre.
fn expand_evolve_token(tok: &str, bar: i64, seed: u64) -> Option<Vec<String>> {
    let inner = tok.strip_prefix("evolve(")?.strip_suffix(')')?;
    let fields: Vec<&str> = inner.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
    let n: usize = fields.first()?.parse().ok()?;
    if n == 0 { return Some(Vec::new()); }
    let gen: usize = fields.get(1).and_then(|s| s.parse().ok()).unwrap_or(6).clamp(1, 32);
    let pop: usize = fields.get(2).and_then(|s| s.parse().ok()).unwrap_or(8).clamp(4, 24);
    let maxd: i64 = 7;
    let mut tick: i64 = 0;
    let mut u = || -> f64 { rng_seq(seed as u32, bar, "evolve", &mut tick) };
    let mut team: Vec<Vec<u8>> = (0..pop).map(|_| (0..n).map(|_| ((u() * maxd as f64) as u8) + 1).collect()).collect();
    let fitness = |v: &[u8]| -> i64 {
        let mut f = 0i64; let mut prev = v[0] as i64;
        for &x in &v[1..] {
            let d = (x as i64 - prev).abs();
            f += match d { 0 => 2, 1 => 3, 2 => 2, _ => 0 } - d;
            prev = x as i64;
        }
        for &x in v { let c = (x as i64 - 1).abs().min((x as i64 - 5).abs()); f += 2 - c; }
        let mut seen = [false; 7];
        for w in v.windows(2) { let d = ((w[1] as i64 - w[0] as i64).abs() % 7) as usize; seen[d] = true; }
        f += seen.iter().filter(|b| **b).count() as i64 * 2;
        f
    };
    for _g in 0..gen {
        let mut scored: Vec<(i64, Vec<u8>)> = team.iter().map(|v| (fitness(v), v.clone())).collect();
        scored.sort_by_key(|e| std::cmp::Reverse(e.0));
        let keep = (pop / 4).max(pop.saturating_sub(4));
        let mut nextp: Vec<Vec<u8>> = scored.iter().take(keep).map(|(_, v)| v.clone()).collect();
        let total: i64 = scored.iter().map(|(f, _)| (*f).max(1)).sum();
        let mut guard = 0;
        while nextp.len() < pop && guard < pop * 16 {
            guard += 1;
            let acc0 = u();
            let mut acc = 0.0f64; let mut pa = scored[0].1.clone();
            for (f, v) in &scored { acc += (*f).max(1) as f64 / total as f64; if acc0 < acc { pa = v.clone(); break; } }
            let acc1 = u();
            let mut acc2 = 0.0f64; let mut pb = scored[0].1.clone();
            for (f, v) in &scored { acc2 += (*f).max(1) as f64 / total as f64; if acc1 < acc2 { pb = v.clone(); break; } }
            let cut = ((u() * n as f64).floor() as usize) % n;
            let mut child: Vec<u8> = pa[..cut].iter().chain(pb[cut..].iter()).cloned().collect();
            if child.len() != n { child = pa[..n].to_vec(); }
            for cell in child.iter_mut() { if u() < 0.2 { *cell = ((u() * maxd as f64) as u8) + 1; } }
            nextp.push(child);
        }
        nextp.truncate(pop);
        if nextp.len() == team.len() { team = nextp; }
    }
    let best = team.iter().max_by_key(|v| fitness(v)).unwrap();
    Some(best.iter().map(|&d| d.to_string()).collect())
}

/// `solve(n[, target:T][, ladder:L][, range:lo-hi][, arc])` — RÉSOLUTION SOUS
/// CONTRAINTES : une MÉLODIE qui respecte les règles données. Le dernier pas est
/// FIXÉ sur `target` (la tonique par défaut), tout enchaînement reste à `ladder`
/// degrés au plus du suivant (marche), tout reste dans `[lo,hi]` (ambitus), et
/// `arc` impose un contour qui monte puis redescend sur la cible. Rempli à
/// RECULONS depuis la cible — une vraie recherche, seedée (bit-identique JS).
fn expand_solve_token(tok: &str, bar: i64, seed: u64) -> Option<Vec<String>> {
    let inner = tok.strip_prefix("solve(")?.strip_suffix(')')?;
    let mut it = inner.split(',');
    let n: usize = it.next()?.trim().parse::<usize>().unwrap_or(16).clamp(1, 64);
    let (mut target, mut ladder, mut lo, mut hi, mut arc) = (1i64, 2i64, 1i64, 7i64, false);
    for c in it {
        let c = c.trim();
        if c.is_empty() { continue; }
        if let Some(v) = c.strip_prefix("target:") { target = v.trim().parse().unwrap_or(1); }
        else if let Some(v) = c.strip_prefix("ladder:") { ladder = v.trim().parse().unwrap_or(2); }
        else if let Some(v) = c.strip_prefix("range:") {
            if let Some((a, b)) = v.trim().split_once('-') { lo = a.trim().parse().unwrap_or(1); hi = b.trim().parse().unwrap_or(7); }
        }
        else if c == "arc" { arc = true; }
    }
    target = target.clamp(1, 7);
    ladder = ladder.clamp(1, 7);
    lo = lo.clamp(1, 7);
    hi = hi.clamp(1, 7);
    if lo > hi { std::mem::swap(&mut lo, &mut hi); }
    target = target.clamp(lo, hi);
    let mut out = vec![0i64; n];
    out[n - 1] = target;
    let peak = if arc { ((n as f64 * 0.62).floor() as usize).clamp(1, n.saturating_sub(2).max(1)) } else { 0 };
    let (h, s0) = (hi.min(target + 4), (target - 1).clamp(lo, hi));
    for i in (0..n - 1).rev() {
        let next = out[i + 1];
        let feasible: Vec<i64> = (lo..=hi).filter(|d| (d - next).abs() <= ladder).collect();
        let u = js_rng(seed as u32, bar, &format!("solve:{}", i), i as i64);
        if !arc {
            out[i] = feasible[((u * feasible.len() as f64).floor() as usize) % feasible.len()];
            continue;
        }
        let w = if i <= peak {
            let t = i as f64 / peak as f64;
            s0 as f64 + (h - s0) as f64 * t
        } else {
            let m = (n - 1 - peak).max(1) as f64;
            h as f64 - (h - target) as f64 * ((i - peak) as f64 / m)
        };
        let total: f64 = feasible.iter().map(|d| 1.0 / (1.0 + ((*d as f64) - w).abs())).sum();
        let mut acc = u * total;
        let mut idx = feasible.len() - 1;
        for (k, d) in feasible.iter().enumerate() {
            let wt = 1.0 / (1.0 + ((*d as f64) - w).abs());
            if acc < wt { idx = k; break; }
            acc -= wt;
        }
        out[i] = feasible[idx];
    }
    Some(out.into_iter().map(|d| d.to_string()).collect())
}

/// Resolve a voice's effective tokens for a given bar (all verbs + bar-aware
/// generative expansions applied).
pub fn effective(voice: &Voice, bar: i64) -> Vec<String> {
    effective_seeded(voice, bar, 0)
}

/// Apply a voice's verbs + generative expansion to an explicit token list.
fn effective_of(toks: &[String], voice: &Voice, bar: i64, seed: u64) -> Vec<String> {
    let mut t = toks.to_vec();
    for v in &voice.verbs { t = apply_verb(&t, v, bar, seed); }
    let mut out = Vec::with_capacity(t.len());
    for tok in t {
        if let Some(seq) = expand_generative_token(&tok, bar) { out.extend(seq); }
        else if let Some(seq) = expand_walk_token(&tok, bar, seed) { out.extend(seq); }
        else if let Some(seq) = expand_drunk_token(&tok, bar, seed) { out.extend(seq); }
        else if let Some(seq) = expand_perlin_token(&tok, seed) { out.extend(seq); }
        else if let Some(seq) = expand_1f_token(&tok, bar, seed) { out.extend(seq); }
        else if let Some(seq) = expand_gendy_token(&tok, bar, seed) { out.extend(seq); }
        else if let Some(seq) = expand_solve_token(&tok, bar, seed) { out.extend(seq); }
        else if let Some(seq) = expand_xys_token(&tok, bar, seed) { out.extend(seq); }
        else if let Some(seq) = expand_evolve_token(&tok, bar, seed) { out.extend(seq); }
        else { out.push(tok); }
    }
    out
}

/// Like [`effective`] but with an explicit `seed` for probabilistic verbs.
pub fn effective_seeded(voice: &Voice, bar: i64, seed: u64) -> Vec<String> {
    effective_of(&voice.tokens, voice, bar, seed)
}

/// Produce every sound event for a given bar across all voices, with no key
/// (scale degrees are left unresolved). See [`schedule_bar_keyed`].
pub fn schedule_bar(voices: &[Voice], bar: i64) -> Vec<Event> {
    schedule_bar_keyed(voices, None, bar)
}

/// Produce every sound event for a given bar across all voices.
/// `phase` is normalized 0..1; the audio backend multiplies by bar seconds.
///
/// When `key` is `Some`, melodic scale-degree tokens (`1`..`7`, with optional
/// `'`/`,` octave marks) are resolved into absolute note names (e.g. `eb3`), so
/// every downstream backend can keep using [`note_to_freq`] unchanged.
pub fn schedule_bar_keyed(voices: &[Voice], key: Option<&Key>, bar: i64) -> Vec<Event> {
    schedule_bar_seeded(voices, key, bar, 0)
}

/// Like [`schedule_bar_keyed`] but with an explicit RNG `seed` for probabilistic
/// tokens (`x?`, `c3?70`). Same seed + bar ⇒ identical events every time.
pub fn schedule_bar_seeded(voices: &[Voice], key: Option<&Key>, bar: i64, seed: u64) -> Vec<Event> {
    sched_prog(voices, key, &[], bar, seed, &[])
}

/// Like [`schedule_bar_seeded`] but with a chord progression so `^`/`^N` tokens
/// follow the harmony (see [`parse_prog`]).
pub fn schedule_bar_prog(voices: &[Voice], key: Option<&Key>, prog: &[String], bar: i64, seed: u64) -> Vec<Event> {
    sched_prog(voices, key, prog, bar, seed, &[])
}

/// Per-step onset offset (fraction of a bar) from swing, per-voice nudge, and the
/// program groove template. Mirrors the JS engine: `t0 += (nudge + swing? + groove)*step`.
fn step_offset(i: usize, n: usize, v: &Voice, groove: &[f32]) -> f32 {
    let sw = if i % 2 == 1 { v.swing } else { 0.0 };
    let g = if groove.is_empty() { 0.0 } else { groove[i % groove.len()] };
    (v.nudge + sw + g) / n as f32
}

fn sched_prog(voices: &[Voice], key: Option<&Key>, prog: &[String], bar: i64, seed: u64, groove: &[f32]) -> Vec<Event> {
    let mut events = Vec::new();
    for v in voices {
        // une voix bytebeat ne joue pas de notes : le moteur audio la rend à part
        if v.bytebeat.is_some() { continue; }
        // `| chance N`: the whole voice sits this bar out on a fraction of bars (seeded).
        if let Some(c) = v.chance {
            if rand01(seed, bar, &format!("{}chc", v.name), 0) >= c { continue; }
        }
        // Effective key for this voice: a `| oct N` overrides the key's octave.
        let voice_key: Option<Key> = key.map(|k| match v.octave {
            Some(o) => Key { tonic: k.tonic, scale: k.scale.clone(), octave: o },
            None => k.clone(),
        });
        let base_oct = voice_key.as_ref().map(|k| k.octave).or(v.octave).unwrap_or(DEFAULT_OCTAVE);
        // polymeter / explicit polyrhythm (`| poly N` / `| poly m:n`):
        // • `poly N` runs an independent N-step loop indexed by the GLOBAL step
        //   count (bar*N + i), so a token list of length != N drifts against the
        //   bar grid each bar (Reich phasing). Mirrors the JS engine.
        // • `poly m:n` cuts the bar into n windows of m equally-spaced steps each
        //   (a m-over-n tuplet) — 3:2 = triolet contre la grille binaire. Same
        //   global-step bookkeeping, so it drifts exactly like `poly (n*m)` but
        //   the grouping m-in-n is explicit (and re-accents each window with the
        //   same tokens, i.e. a 3-token phrase lands on every 2-grid step).
        match v.poly {
            Some(Poly::Meter(spb)) => {
                let spb = spb.max(1) as usize;
                let nn = v.tokens.len();
                if nn > 0 {
                    for i in 0..spb {
                        let s = bar * spb as i64 + i as i64;
                        let ls = v.sync.as_ref().map_or(s, |sy| sync_local(s, spb as i64, sy));
                        let idx = ls.rem_euclid(nn as i64) as usize;
                        let off = step_offset(i, spb, v, groove);
                        let p0 = i as f32 / spb as f32 + off;
                        let p1 = (i + 1) as f32 / spb as f32 + off;
                        emit_token(&mut events, &v.tokens[idx], p0, p1, v, voice_key.as_ref(),
                                   base_oct, bar, seed, s.max(0) as usize, prog, (idx as i64, nn as i64));
                    }
                }
                continue;
            }
            Some(Poly::Grid(m, n)) => {
                let m = m.max(1) as usize;
                let n = n.max(1) as usize;
                let total = n * m; // steps per bar
                let nn = v.tokens.len();
                if nn > 0 {
                    for w in 0..n {
                        for i in 0..m {
                            let g = w * m + i; // global step index within the bar
                            let s = bar * total as i64 + g as i64;
                            let ls = v.sync.as_ref().map_or(s, |sy| sync_local(s, total as i64, sy));
                            let idx = ls.rem_euclid(nn as i64) as usize;
                            let off = step_offset(g, total, v, groove);
                            let p0 = g as f32 / total as f32 + off;
                            let p1 = (g + 1) as f32 / total as f32 + off;
                            emit_token(&mut events, &v.tokens[idx], p0, p1, v, voice_key.as_ref(),
                                       base_oct, bar, seed, s.max(0) as usize, prog, (idx as i64, nn as i64));
                        }
                    }
                }
                continue;
            }
            None => {}
        }
        // stacks: each `;`-separated line plays simultaneously (a single line = one stack)
        let lines: &[Vec<String>] = if v.stacks.len() > 1 { &v.stacks } else { std::slice::from_ref(&v.tokens) };
        for line in lines {
            let mut eff = effective_of(line, v, bar, seed);
            if eff.is_empty() { continue; }
            // `| sync AUTRE` : la mesure repart du début à chaque reprise du meneur
            if let Some(sy) = &v.sync {
                let n = eff.len() as i64;
                eff = (0..n).map(|i| eff[sync_local(bar * n + i, n, sy).rem_euclid(n) as usize].clone()).collect();
            }
            // Pattern algebra layers (mirrors the JS engine):
            //  jux <verb> : original hard-left + a <verb>-copy hard-right (stereo split)
            //  off N <verb>: for each layer, an extra copy shifted N steps later, quieter
            // Each layer is (voice-with-pan/gain overrides, tokens, shift-in-steps).
            let mut layers: Vec<(Voice, Vec<String>, f32)> = Vec::new();
            if let Some(jverb) = &v.jux {
                let mut left = v.clone();  left.pan = Some(-0.82);
                let mut right = v.clone(); right.pan = Some(0.82);
                let rtoks = apply_verb(&eff, jverb, bar, seed);
                layers.push((left, eff.clone(), 0.0));
                layers.push((right, rtoks, 0.0));
            } else {
                layers.push((v.clone(), eff.clone(), 0.0));
            }
            if let Some((shift, overb)) = &v.off {
                let extra: Vec<(Voice, Vec<String>, f32)> = layers.iter().map(|(lv, le, sh)| {
                    let mut ov = lv.clone();
                    ov.gain = lv.gain * 0.6;
                    let toks = match overb { Some(vb) => apply_verb(le, vb, bar, seed), None => le.clone() };
                    (ov, toks, sh + shift)
                }).collect();
                layers.extend(extra);
            }
            for (lv, toks, shift) in &layers {
                let n = toks.len();
                if n == 0 { continue; }
                for (i, raw) in toks.iter().enumerate() {
                    // `| degrade N`: randomly drop this hit (seeded, same as JS).
                    if lv.degrade > 0.0 && rand01(seed, bar, &format!("{}dg", lv.name), i) < lv.degrade { continue; }
                    let off = step_offset(i, n, lv, groove) + shift / n as f32;
                    let p0 = i as f32 / n as f32 + off;
                    let p1 = (i + 1) as f32 / n as f32 + off;
                    emit_token(&mut events, raw, p0, p1, lv, voice_key.as_ref(), base_oct, bar, seed, i, prog, (i as i64, n as i64));
                }
            }
        }
    }
    events
}

/// Split a pattern on whitespace but keep `[a b]` and `{a b}` groups (nested, mixed) as one token.
/// Les parenthèses aussi gardent leurs espaces (`=(i * 3)`, `e(3, 8)`) sans couper
/// le jeton avant elles : `e(` et `=(` commencent en plein jeton.
pub fn tokenize_pat(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut paren = 0i32;
    let mut cur = String::new();
    for ch in s.chars() {
        if ch == '(' {
            paren += 1; cur.push(ch);
        } else if ch == ')' {
            if paren > 0 { paren -= 1; }
            cur.push(ch);
        } else if ch == '[' || ch == '{' {
            if depth == 0 && !cur.is_empty() { out.push(std::mem::take(&mut cur)); }
            depth += 1; cur.push(ch);
        } else if ch == ']' || ch == '}' {
            if depth > 0 { depth -= 1; }
            cur.push(ch);
            if depth == 0 { out.push(std::mem::take(&mut cur)); }
        } else if ch.is_whitespace() {
            if depth == 0 && paren == 0 { if !cur.is_empty() { out.push(std::mem::take(&mut cur)); } }
            else { cur.push(ch); }
        } else { cur.push(ch); }
    }
    if !cur.is_empty() { out.push(cur); }
    out.into_iter().filter(|t| !t.starts_with('@') && !t.starts_with('>')).collect()
}

/// Coupe une ligne de voix sur les `|` hors parenthèses : un pas-programme
/// `=(i|b)` ou un `xys(…1>2:2|3:1…)` garde son `|`. Parenthèses déséquilibrées :
/// coupe simple sur chaque `|` (comportement d'avant).
pub fn split_bars(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut from = 0;
    for (k, c) in s.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            '|' if depth == 0 => { out.push(&s[from..k]); from = k + 1; }
            _ => {}
        }
    }
    if depth != 0 { return s.split('|').collect(); }
    out.push(&s[from..]);
    out
}

/// Pas-programme `=(EXPR)` suivi de modificateurs (`?70`, `!`, `*3`…) : le jeton
/// calculé au moment de jouer le pas. Variables : `i` (position du pas dans sa
/// ligne, après les verbes ; dans un groupe `[…]`, dans le groupe ; en `poly`,
/// l'indice dans la liste de jetons), `b` (numéro de mesure), `n` (nombre de pas
/// de cette ligne). Valeur : avec `key:`/`scale:`, un degré compté depuis 0
/// (0 = tonique `1`, 7 = `1'`, -1 = `7,`, borné à ±35) ; sans tonalité, une note
/// MIDI (bornée 0..127, 60 = c4) ; batterie : non nul = coup, 0 = silence.
/// Expression invalide : silence. Rend `None` si `tok` n'est pas un pas-programme.
fn step_program(tok: &str, is_drum: bool, keyed: bool, i: i64, n: i64, bar: i64) -> Option<String> {
    let body = tok.strip_prefix('=')?;
    if !body.starts_with('(') { return None; }
    let mut depth = 0i32;
    let mut close = None;
    for (k, c) in body.char_indices() {
        if c == '(' { depth += 1; } else if c == ')' { depth -= 1; if depth == 0 { close = Some(k); break; } }
    }
    let Some(close) = close else { return Some(".".into()) };
    let Ok(e) = Expr::parse(&body[1..close], &['i', 'b', 'n']) else { return Some(".".into()) };
    let clamp = |x: i64, lo: i64, hi: i64| x.clamp(lo, hi) as i32;
    let v = e.eval(&[clamp(i, i32::MIN as i64, i32::MAX as i64), clamp(bar, i32::MIN as i64, i32::MAX as i64), clamp(n, 0, i32::MAX as i64)]);
    let mods = &body[close + 1..];
    let base = if is_drum {
        (if v != 0 { "x" } else { "." }).to_string()
    } else if keyed {
        deg_from_index(v.clamp(-35, 35) as i64, "")
    } else {
        midi_to_name(v.clamp(0, 127))
    };
    if base == "." { return Some(base); }
    Some(base + mods)
}

/// Recursively emit events for one token within the phase range `[p0, p1)`.
/// Handles `{a b}` random choice (seeded), `<a/b>` alternation, `[a b]` sub-sequences
/// (recursive subdivision), `?`/`*` mods, `!` accents and `*N>`/`*N<` rolls, and arps.
#[allow(clippy::too_many_arguments)]
fn emit_token(events: &mut Vec<Event>, tok: &str, p0: f32, p1: f32, v: &Voice,
              key: Option<&Key>, base_oct: i32, bar: i64, seed: u64, salt: usize, prog: &[String], pos: (i64, i64)) {
    // {a b c}: pick ONE alternative at random (seeded -> reproducible)
    let mut t = tok.to_string();
    if t.len() >= 2 && t.starts_with('{') && t.ends_with('}') {
        let inner = tokenize_pat(&t[1..t.len() - 1]);
        if inner.is_empty() { return; }
        let r = rand01(seed, bar, &v.name, salt.wrapping_add(7));
        let idx = ((r * inner.len() as f32) as usize).min(inner.len() - 1);
        t = inner[idx].clone();
    }
    // <a/b>: alternation by bar
    t = pick_alt(&t, bar).to_string();
    // ^ / ^N: follow the chord progression
    if !prog.is_empty() && t.starts_with('^') { t = resolve_prog(&t, prog, bar, key, base_oct); }
    // =(EXPR) : pas-programme, calculé maintenant (voir `step_program`)
    if let Some(c) = step_program(&t, v.is_drum, key.is_some(), pos.0, pos.1, bar) { t = c; }
    if t == "." || t == "~" { return; }
    // [a b ...]: subdivide this range among the children (recursive)
    if t.len() >= 2 && t.starts_with('[') && t.ends_with(']') {
        let inner = tokenize_pat(&t[1..t.len() - 1]);
        let m = inner.len();
        if m == 0 { return; }
        let span = (p1 - p0) / m as f32;
        for (k, child) in inner.iter().enumerate() {
            emit_token(events, child, p0 + k as f32 * span, p0 + (k + 1) as f32 * span,
                       v, key, base_oct, bar, seed, salt.wrapping_mul(31).wrapping_add(k), prog, (k as i64, m as i64));
        }
        return;
    }
    let (base, prob, ratchet, accent, ramp_dir) = token_mods(&t);
    if let Some(p) = prob {
        if rand01(seed, bar, &v.name, salt) >= p { return; }
    }
    if base.is_empty() || base == "." || base == "~" { return; }
    let notes = resolve_token(base, v.is_drum, base_oct, key);
    // Exact pitch for a single scale-degree note: the `token` name is rounded to
    // 12-TET, but a custom microtonal scale needs the unrounded frequency. For
    // everything else (chords, plain notes) we fall back to the note name below.
    let deg_freq: Option<f32> = match key {
        Some(k) if !v.is_drum && is_degree(base) => degree_to_midi(base, k).map(midi_f_to_freq),
        _ => None,
    };
    let freq_for = |note: &str| -> Option<f32> {
        if v.is_drum { None } else { deg_freq.or_else(|| note_to_freq(note)) }
    };
    let span = p1 - p0;
    if let Some(mode) = v.arp {
        if !v.is_drum && notes.len() > 1 {
            // `| arp up 2`: stack the chord again one (or more) octaves higher
            let mut span_notes = notes.clone();
            for o in 1..v.synth.arp_octaves.unwrap_or(1) as i32 {
                span_notes.extend(notes.iter().filter_map(|n| note_to_midi(n)).map(|m| midi_to_name(m + 12 * o)));
            }
            let ord = arp_order(&span_notes, mode);
            let m = ord.len();
            for (j, note) in ord.iter().enumerate() {
                events.push(Event { phase: p0 + (j as f32 / m as f32) * span, voice: v.name.clone(),
                    token: note.clone(), wave: v.wave, gain: v.gain, is_drum: v.is_drum, vel: accent, freq: freq_for(note), pan: v.pan, dur: span / m as f32, synth: v.synth.clone() });
            }
            return;
        }
    }
    for j in 0..ratchet {
        let phase = p0 + (j as f32 / ratchet as f32) * span;
        let vel = accent * ramp_factor(j, ratchet, ramp_dir);
        for note in &notes {
            events.push(Event { phase, voice: v.name.clone(), token: note.clone(),
                wave: v.wave, gain: v.gain, is_drum: v.is_drum, vel, freq: freq_for(note), pan: v.pan, dur: span / ratchet as f32, synth: v.synth.clone() });
        }
    }
}

/// Convert a note name like "c2", "eb3", "g#4" to a MIDI note number.
pub fn note_to_midi(note: &str) -> Option<i32> {
    let b = note.as_bytes();
    if b.is_empty() { return None; }
    let letter = (b[0] as char).to_ascii_lowercase();
    let base = match letter { 'c'=>0,'d'=>2,'e'=>4,'f'=>5,'g'=>7,'a'=>9,'b'=>11, _=>return None };
    let mut idx = 1;
    let mut semi: i32 = base;
    if idx < b.len() && (b[idx] == b'#' || b[idx] == b'b') {
        if b[idx] == b'#' { semi += 1; } else { semi -= 1; }
        idx += 1;
    }
    let octave: i32 = note[idx..].parse().ok()?;
    Some(12 * (octave + 1) + semi)
}

/// Convert a note name like "c2", "eb3", "g#4" to a frequency in Hz.
pub fn note_to_freq(note: &str) -> Option<f32> {
    note_to_midi(note).map(midi_to_freq)
}

/// MIDI note number → frequency in Hz (A4 = midi 69 = 440 Hz).
pub fn midi_to_freq(midi: i32) -> f32 {
    440.0 * 2f32.powf((midi as f32 - 69.0) / 12.0)
}

/// Fractional MIDI → Hz. Same formula, but accepts microtonal pitches (e.g. a
/// quarter-tone is midi 60.5). Lets custom numeric scales sound exactly.
pub fn midi_f_to_freq(midi: f64) -> f32 {
    440.0 * 2f32.powf((midi as f32 - 69.0) / 12.0)
}

/// Microtuning systems (mirrors the web engine's `tune:` directive).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Tuning {
    Equal,
    /// 5-limit just intonation.
    Just,
    /// 3-limit Pythagorean.
    Pyth,
    /// N-tone equal temperament (pure octaves, N equal steps).
    Edo(u32),
    /// Werckmeister III well-temperament (1691).
    Werck,
    /// Kirnberger III well-temperament.
    Kirn,
    /// Quarter-comma meantone (pure major thirds).
    Meantone,
}

/// Cents deviation from 12-TET for a semitone interval (0..=11) from the tonic.
/// 5-limit JI and Pythagorean tables match the web engine exactly.
pub fn tune_cents(interval: i32, tuning: Tuning) -> f32 {
    const JI: [f32; 12]   = [0.0, 11.73, 3.91, 15.64, -13.69, -1.96, -9.78, 1.96, 13.69, -15.64, -3.91, -11.73];
    const PYTH: [f32; 12] = [0.0, -9.78, 3.91, -5.87, 7.82, -1.96, 11.73, 1.96, -7.82, 5.87, -3.91, 9.78];
    const WERCK: [f32; 12] = [0.0, -10.0, -8.0, -6.0, -10.0, -2.0, -12.0, -4.0, -8.0, -12.0, -4.0, -8.0];
    const KIRN: [f32; 12]  = [0.0, -10.0, -7.0, -6.0, -14.0, -2.0, -10.0, -3.0, -8.0, -10.0, -4.0, -12.0];
    const MEAN: [f32; 12]  = [0.0, -24.0, -7.0, 10.0, -14.0, 3.0, -21.0, -3.0, -28.0, -10.0, 7.0, -17.0];
    let iv = interval.rem_euclid(12) as usize;
    match tuning {
        Tuning::Equal => 0.0,
        Tuning::Just => JI[iv],
        Tuning::Pyth => PYTH[iv],
        Tuning::Werck => WERCK[iv],
        Tuning::Kirn => KIRN[iv],
        Tuning::Meantone => MEAN[iv],
        // N-EDO: snap the semitone interval to the nearest EDO step, return the cents delta.
        Tuning::Edo(n) => {
            let n = n.max(1) as f32;
            let steps = (interval as f32 * n / 12.0).round();
            steps * 1200.0 / n - interval as f32 * 100.0
        }
    }
}

/// Frequency of a scale degree under a tuning system (cents applied on top of the chromatic pitch).
pub fn degree_freq(midi: i32, interval_from_tonic: i32, tuning: Tuning) -> f32 {
    midi_to_freq(midi) * 2f32.powf(tune_cents(interval_from_tonic, tuning) / 1200.0)
}

// ---------------------------------------------------------------------------
// Music theory layer (roadmap item 1): key + scale + scale degrees.
//
// A melodic token can now be a *scale degree* (1..7) that resolves against a
// declared key, e.g.
//     key: c minor
//     bass: 1 . 5 . 3 . 5 .      # 1=c, 5=g, 3=eb  (the b3 comes from the scale)
// Octave marks: `'` raises one octave, `,` lowers one (Lilypond-style).
// Chromatic alterations on degrees: `4#` / `7b` raise or lower the degree by a
// semitone (see `is_degree` / `degree_to_midi`); otherwise the scale supplies them.
// ---------------------------------------------------------------------------

/// A declared tonality: a tonic pitch-class plus a scale (intervals from the
/// tonic, within one octave) and the reference octave for degree 1.
#[derive(Debug, Clone, PartialEq)]
pub struct Key {
    /// Tonic pitch class, 0..=11 with c=0.
    pub tonic: i32,
    /// Scale intervals in semitones from the tonic, e.g. major = [0,2,4,5,7,9,11].
    /// Stored as f64 so custom numeric scales can be microtonal, e.g.
    /// `key: d 0 1.5 3 4 6 7.5 9 10` (quarter-tones). Named scales are integers.
    pub scale: Vec<f64>,
    /// Octave that degree 1 sounds in (c in this octave = degree 1 of a C key).
    pub octave: i32,
}

/// Default reference octave for degree 1 (C3 region — sits between bass and lead).
const DEFAULT_OCTAVE: i32 = 3;

/// Pitch class (0..=11) for a tonic word like "c", "c#", "eb", "f#".
fn tonic_pc(word: &str) -> Option<i32> {
    let b = word.as_bytes();
    if b.is_empty() { return None; }
    let base = match (b[0] as char).to_ascii_lowercase() {
        'c'=>0,'d'=>2,'e'=>4,'f'=>5,'g'=>7,'a'=>9,'b'=>11, _=>return None,
    };
    let mut semi: i32 = base;
    if b.len() > 1 {
        match b[1] { b'#' => semi += 1, b'b' => semi -= 1, _ => {} }
    }
    Some(semi.rem_euclid(12))
}

/// Scale intervals (one octave) for a named scale. Returns None if unknown.
pub fn scale_intervals(name: &str) -> Option<Vec<i32>> {
    let s: Vec<i32> = match name {
        "major" | "ionian"            => vec![0, 2, 4, 5, 7, 9, 11],
        "minor" | "aeolian"           => vec![0, 2, 3, 5, 7, 8, 10],
        "dorian"                      => vec![0, 2, 3, 5, 7, 9, 10],
        "phrygian"                    => vec![0, 1, 3, 5, 7, 8, 10],
        "lydian"                      => vec![0, 2, 4, 6, 7, 9, 11],
        "mixolydian"                  => vec![0, 2, 4, 5, 7, 9, 10],
        "locrian"                     => vec![0, 1, 3, 5, 6, 8, 10],
        "pentatonic" | "majpent"      => vec![0, 2, 4, 7, 9],
        "minpent"                     => vec![0, 3, 5, 7, 10],
        // extra Western scales
        "harmonic_minor" | "harmonicminor" => vec![0, 2, 3, 5, 7, 8, 11],
        "melodic_minor" | "melodicminor"   => vec![0, 2, 3, 5, 7, 9, 11],
        "whole_tone" | "wholetone"         => vec![0, 2, 4, 6, 8, 10],
        "blues"                            => vec![0, 3, 5, 6, 7, 10],
        "chromatic"                        => (0..12).collect(),
        // Maqam family (12-TET approximations; true quarter tones are a future
        // microtonal layer — see NIGHT_LOG). Tonic-based interval sets.
        "hijaz"                            => vec![0, 1, 4, 5, 7, 8, 10],
        "hijazkar" | "double_harmonic" | "byzantine" => vec![0, 1, 4, 5, 7, 8, 11],
        "nahawand"                         => vec![0, 2, 3, 5, 7, 8, 10], // ~ minor
        "kurd"                             => vec![0, 1, 3, 5, 7, 8, 10], // ~ phrygian
        "ajam"                             => vec![0, 2, 4, 5, 7, 9, 11], // ~ major
        // Raga (approximations)
        "bhairav"                          => vec![0, 1, 4, 5, 7, 8, 11],
        "yaman"                            => vec![0, 2, 4, 6, 7, 9, 11], // ~ lydian
        "todi"                             => vec![0, 1, 3, 6, 7, 8, 11],
        // Gamelan & Asian pentatonics (approximated in 12-TET)
        "slendro"                          => vec![0, 2, 5, 7, 10],
        "pelog"                            => vec![0, 1, 3, 7, 8],
        "hirajoshi"                        => vec![0, 2, 3, 7, 8],
        "insen"                            => vec![0, 1, 5, 7, 10],
        "iwato"                            => vec![0, 1, 5, 6, 10],
        "egyptian"                         => vec![0, 2, 5, 7, 10],
        // symmetric & jazz
        "octatonic" | "octatonic_hw" | "diminished" => vec![0, 1, 3, 4, 6, 7, 9, 10],
        "octatonic_wh"                     => vec![0, 2, 3, 5, 6, 8, 9, 11],
        "augmented"                        => vec![0, 3, 4, 7, 8, 11],
        "bebop" | "bebop_dominant"         => vec![0, 2, 4, 5, 7, 9, 10, 11],
        "bebop_major"                      => vec![0, 2, 4, 5, 7, 8, 9, 11],
        "phrygian_dominant" | "spanish"    => vec![0, 1, 4, 5, 7, 8, 10],
        // exotic / world
        "hungarian_minor" | "gypsy"        => vec![0, 2, 3, 6, 7, 8, 11],
        "hungarian_major"                  => vec![0, 3, 4, 6, 7, 9, 10],
        "neapolitan_minor"                 => vec![0, 1, 3, 5, 7, 8, 11],
        "neapolitan_major"                 => vec![0, 1, 3, 5, 7, 9, 11],
        "enigmatic"                        => vec![0, 1, 4, 6, 8, 10, 11],
        "persian"                          => vec![0, 1, 4, 5, 6, 8, 11],
        "prometheus"                       => vec![0, 2, 4, 6, 9, 10],
        "marwa"                            => vec![0, 1, 4, 6, 7, 9, 11],
        "purvi"                            => vec![0, 1, 4, 6, 7, 8, 11],
        // more pentatonics
        "kumoi"                            => vec![0, 2, 3, 7, 9],
        "yo"                               => vec![0, 2, 5, 7, 9],
        "blues_major" | "majblues"         => vec![0, 2, 3, 4, 7, 9],
        // maqam with neutral (quarter-tone) steps — these are 12-TET roundings;
        // the web engine renders them with true microtones. See NIGHT_LOG.
        "bayati"                           => vec![0, 2, 3, 5, 7, 8, 10],
        "rast"                             => vec![0, 2, 4, 5, 7, 9, 11],
        "saba"                             => vec![0, 2, 3, 4, 7, 8, 10],
        "sikah"                            => vec![0, 2, 4, 6, 7, 9, 11],
        "huzam"                            => vec![0, 2, 4, 5, 7, 9, 10],
        "jiharkah"                         => vec![0, 2, 4, 5, 7, 9, 11],
        // overtone / contemporary (added to match the web engine)
        "overtone" | "acoustic" | "lydian_dominant" => vec![0, 2, 4, 6, 7, 9, 10],
        "altered" | "super_locrian"        => vec![0, 1, 3, 4, 6, 8, 10],
        "lydian_augmented"                 => vec![0, 2, 4, 6, 8, 9, 11],
        "messiaen3"                        => vec![0, 2, 3, 4, 6, 7, 8, 10, 11],
        "messiaen4"                        => vec![0, 1, 2, 5, 6, 7, 8, 11],
        "ritusen"                          => vec![0, 2, 5, 7, 9],
        "chinese"                          => vec![0, 4, 6, 7, 11],
        _ => return None,
    };
    Some(s)
}

/// Parse `key@SECTION: <tonic> <scale>` lines into a (section, Key) map (modulation).
pub fn parse_key_map(src: &str) -> Vec<(String, Key)> {
    let mut out = Vec::new();
    for raw in src.lines() {
        let line = raw.trim();
        let Some(colon) = line.find(':') else { continue };
        let head = line[..colon].trim().to_lowercase();
        if let Some(sec) = head.strip_prefix("key@") {
            let rest = line[colon + 1..].trim();
            if let Some(k) = parse_key(&format!("key: {}", rest)) {
                out.push((sec.trim().to_string(), k));
            }
        }
    }
    out
}

/// Read the `key:` / `scale:` directives from a program. Returns None if the
/// program declares no key (degrees then simply don't resolve, as before).
///
/// - `key: <tonic> [scale]`  e.g. `key: c minor`, `key: f# dorian`, `key: a`
/// - `scale: <name>`         overrides just the scale, tonic defaults to c
pub fn parse_key(src: &str) -> Option<Key> {
    let mut tonic: Option<i32> = None;
    let mut scale_name: Option<String> = None;
    let mut scale_nums: Option<Vec<f64>> = None; // custom numeric (possibly microtonal) scale

    for raw in src.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        let Some(colon) = line.find(':') else { continue };
        let name = line[..colon].trim().to_lowercase();
        let rest = line[colon + 1..].trim().to_lowercase();
        let mut words = rest.split_whitespace().peekable();
        match name.as_str() {
            "key" => {
                if let Some(t) = words.next().and_then(tonic_pc) { tonic = Some(t); }
                // `key: c 0 2 3.5 5 ...` -> custom numeric scale (microtonal allowed)
                if matches!(words.peek(), Some(w) if w.parse::<f64>().is_ok()) {
                    scale_nums = Some(words.filter_map(|w| w.parse::<f64>().ok()).collect());
                } else if let Some(s) = words.next() {
                    scale_name = Some(s.to_string());
                }
            }
            "scale" => {
                if matches!(words.peek(), Some(w) if w.parse::<f64>().is_ok()) {
                    scale_nums = Some(words.filter_map(|w| w.parse::<f64>().ok()).collect());
                } else if let Some(s) = words.next() {
                    scale_name = Some(s.to_string());
                }
            }
            _ => {}
        }
    }

    if tonic.is_none() && scale_name.is_none() && scale_nums.is_none() { return None; }
    let scale: Vec<f64> = match scale_nums {
        Some(nums) if !nums.is_empty() => nums,
        _ => scale_name
            .as_deref()
            .and_then(scale_intervals)
            .unwrap_or_else(|| scale_intervals("major").unwrap())
            .into_iter()
            .map(|x| x as f64)
            .collect(),
    };
    Some(Key { tonic: tonic.unwrap_or(0), scale, octave: DEFAULT_OCTAVE })
}

/// Is this token a scale degree? Form: a digit 1..7, an optional chromatic
/// alteration (`#` raise / `b` lower) right after the digit, then any number of
/// octave marks (`'` up, `,` down). Because it starts with a digit it can never
/// collide with a note name (which always starts with a letter a..g).
fn is_degree(token: &str) -> bool {
    let mut chars = token.chars().peekable();
    match chars.next() {
        Some(c) if ('1'..='7').contains(&c) => {}
        _ => return false,
    }
    if matches!(chars.peek(), Some('#') | Some('b')) { chars.next(); }
    chars.all(|c| c == '\'' || c == ',')
}

/// Resolve a scale-degree token (e.g. "1", "5", "7#", "3b", "1'") to a MIDI
/// note for a key. The optional `#`/`b` right after the digit shifts the note a
/// semitone (e.g. `7#` is the raised 7th — the harmonic-minor leading tone).
/// Degrees beyond the scale length wrap up an octave (so `6` in a 5-note
/// pentatonic = degree 1, one octave higher).
fn degree_to_midi(token: &str, key: &Key) -> Option<f64> {
    let mut chars = token.chars().peekable();
    let d = chars.next()?.to_digit(10)? as i32; // 1..7
    let acc = match chars.peek() {
        Some('#') => { chars.next(); 1 }
        Some('b') => { chars.next(); -1 }
        _ => 0,
    };
    let mut oct_marks = 0i32;
    for c in chars {
        match c { '\'' => oct_marks += 1, ',' => oct_marks -= 1, _ => return None }
    }
    let len = key.scale.len() as i32;
    if len == 0 { return None; }
    let idx = d - 1;
    let wrap = idx.div_euclid(len);
    let step = key.scale[idx.rem_euclid(len) as usize]; // f64: may be a quarter-tone
    let octave = key.octave + oct_marks + wrap;
    Some((12 * (octave + 1) + key.tonic + acc) as f64 + step)
}

/// Resolve a token to a frequency: a scale degree if a key is given and the
/// token is a degree, otherwise an absolute note name (`c3`, `eb2`, …).
pub fn token_to_freq(token: &str, key: Option<&Key>) -> Option<f32> {
    if let Some(k) = key {
        if is_degree(token) {
            return degree_to_midi(token, k).map(midi_f_to_freq);
        }
    }
    note_to_freq(token)
}

/// MIDI note → an absolute note-name string (flat spelling), e.g. 48 -> "c3".
fn midi_to_name(midi: i32) -> String {
    const NAMES: [&str; 12] = ["c","db","d","eb","e","f","gb","g","ab","a","bb","b"];
    let octave = midi.div_euclid(12) - 1;
    let pc = midi.rem_euclid(12) as usize;
    format!("{}{}", NAMES[pc], octave)
}

/// Parse a chord symbol into its MIDI notes, e.g. `Cm7` -> [c, eb, g, bb].
///
/// Convention: the root must be an UPPERCASE letter A–G (so `Cm7`, `F`, `G7`).
/// Lowercase tokens stay single notes/degrees (`c3`, `eb2`), which is how we
/// disambiguate `C` the chord from `c3` the note. Uppercase note names that
/// carry an octave (`C3`) fail the quality match and fall back to a plain note.
/// Pitch class (0..=11) of a chord/bass note like `C`, `F#`, `Bb` (uppercase root).
fn chord_root_pc(s: &str) -> Option<i32> {
    let b = s.as_bytes();
    if b.is_empty() { return None; }
    let mut root: i32 = match b[0] as char {
        'C'=>0,'D'=>2,'E'=>4,'F'=>5,'G'=>7,'A'=>9,'B'=>11, _=>return None,
    };
    if b.len() > 1 { match b[1] { b'#' => root += 1, b'b' => root -= 1, _ => {} } }
    Some(root.rem_euclid(12))
}

pub fn parse_chord(tok: &str, octave: i32) -> Option<Vec<i32>> {
    // slash chord / inversion: `Cm7/G` puts G in the bass, below the chord.
    let (tok, slash_bass) = match tok.split_once('/') {
        Some((c, b)) => (c, chord_root_pc(b)),
        None => (tok, None),
    };
    let b = tok.as_bytes();
    if b.is_empty() { return None; }
    let base: i32 = match b[0] as char {
        'C'=>0,'D'=>2,'E'=>4,'F'=>5,'G'=>7,'A'=>9,'B'=>11, _=>return None,
    };
    let mut idx = 1;
    let mut root = base;
    if idx < b.len() && (b[idx] == b'#' || b[idx] == b'b') {
        if b[idx] == b'#' { root += 1; } else { root -= 1; }
        idx += 1;
    }
    let intervals: &[i32] = match &tok[idx..] {
        "" | "maj" | "M"            => &[0, 4, 7],
        "m" | "min" | "-"           => &[0, 3, 7],
        "5"                         => &[0, 7],            // power chord
        "dim" | "o"                 => &[0, 3, 6],
        "aug" | "+"                 => &[0, 4, 8],
        "sus2"                      => &[0, 2, 7],
        "sus4" | "sus"              => &[0, 5, 7],
        "6"                         => &[0, 4, 7, 9],
        "m6" | "min6"               => &[0, 3, 7, 9],
        "69" | "6/9"                => &[0, 4, 7, 9, 14],
        "7"                         => &[0, 4, 7, 10],
        "maj7" | "M7"               => &[0, 4, 7, 11],
        "m7" | "min7" | "-7"        => &[0, 3, 7, 10],
        "mmaj7" | "mMaj7" | "m#7"   => &[0, 3, 7, 11],
        "m7b5" | "min7b5"           => &[0, 3, 6, 10],     // half-diminished
        "dim7" | "o7"               => &[0, 3, 6, 9],
        "7b5"                       => &[0, 4, 6, 10],
        "7#5" | "7+5" | "aug7"      => &[0, 4, 8, 10],
        "7b9"                       => &[0, 4, 7, 10, 13],
        "7#9"                       => &[0, 4, 7, 10, 15],
        "7#11"                      => &[0, 4, 7, 10, 18],
        "add9"                      => &[0, 4, 7, 14],
        "add11"                     => &[0, 4, 7, 17],
        "9"                         => &[0, 4, 7, 10, 14],
        "maj9" | "M9"               => &[0, 4, 7, 11, 14],
        "m9" | "min9"               => &[0, 3, 7, 10, 14],
        "11"                        => &[0, 7, 10, 14, 17],
        "m11"                       => &[0, 3, 7, 10, 14, 17],
        "13"                        => &[0, 4, 7, 10, 14, 21],
        "maj13" | "M13"             => &[0, 4, 7, 11, 14, 21],
        "m13"                       => &[0, 3, 7, 10, 14, 21],
        _ => return None,
    };
    let root_midi = 12 * (octave + 1) + root;
    let mut notes: Vec<i32> = intervals.iter().map(|i| root_midi + i).collect();
    if let Some(bass_pc) = slash_bass {
        // place the named bass note an octave below the root
        notes.insert(0, 12 * octave + bass_pc);
    }
    Some(notes)
}

// ---------------------------------------------------------------------------
// Probability & determinism (roadmap B2/B3).
//
// A token may carry a probability suffix: `x?` ~ 50%, `x?70` = 70%. Whether it
// fires is decided by a deterministic hash of (seed, bar, voice, step), so a
// given `seed:` always replays the exact same performance — reproducible and
// testable, with no RNG state and no dependencies.
// ---------------------------------------------------------------------------

// Deterministic pseudo-random value in [0,1) from the performance coordinates.
// 32-bit FNV-1a — identical to the JS engine's fnv() (ASCII voice names).
fn js_fnv(s: &str) -> u32 {
    let mut h: u32 = 2166136261;
    for b in s.bytes() { h ^= b as u32; h = h.wrapping_mul(16777619); }
    h
}
// Seeded RNG -> [0,1), BIT-IDENTICAL to the JS engine's rng() (Math.imul mixing).
// This is what makes walk/drunk/probability produce the SAME result in both engines.
fn js_rng(seed: u32, bar: i64, vname: &str, step: i64) -> f64 {
    let mut h: u32 = (seed ^ 0x9e3779b1).wrapping_mul(2654435761);
    h = (h ^ (bar as u32)).wrapping_mul(2246822519);
    h = (h ^ js_fnv(vname)).wrapping_mul(3266489917);
    h = (h ^ ((step + 1) as u32)).wrapping_mul(668265263);
    h ^= h >> 15;
    (h as f64) / 4294967296.0
}
fn rand01(seed: u64, bar: i64, voice: &str, step: usize) -> f32 {
    js_rng(seed as u32, bar, voice, step as i64) as f32
}

/// The engine's deterministic random number (0..1) for a seed/bar/voice/step —
/// the same sequence as the browser's `rng()`, for players that add humanisation.
pub fn rng01(seed: u64, bar: i64, voice: &str, step: usize) -> f32 {
    rand01(seed, bar, voice, step)
}

/// Digits immediately following position `at` in `tok` (a `?`/`*` marker).
fn digits_after(tok: &str, at: usize) -> &str {
    let rest = &tok[at + 1..];
    let end = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
    &rest[..end]
}

/// Parse a token's modifiers, in any order: probability `?P` (`x?` ~50%,
/// `x?70` = 70%) and ratchet `*N` (`x*3` = 3 sub-hits). Returns the bare base
/// token, an optional fire probability in 0..1, and a ratchet count ≥ 1.
fn token_mods(tok: &str) -> (&str, Option<f32>, usize, f32, i8) {
    // a group/alt token (`<` `{` `[`) carries its own `>`/`<` brackets — don't read those as a roll ramp
    let grp = tok.starts_with('<') || tok.starts_with('{') || tok.starts_with('[');
    let q = tok.find('?');
    let r = tok.find('*');
    let e = tok.find('!');
    let gt = if grp { None } else { tok.find('>').filter(|&i| i > 0) };
    let lt = if grp { None } else { tok.find('<').filter(|&i| i > 0) };
    let ramp_dir: i8 = if gt.is_some() { 1 } else if lt.is_some() { -1 } else { 0 };
    let cands = [q, r, e, gt, lt];
    let cut = cands.iter().filter_map(|&x| x).min();
    let cut = match cut {
        Some(c) => c,
        None => return (tok, None, 1, 1.0, 0), // no modifiers at all
    };
    let prob = q.map(|p| {
        let d = digits_after(tok, p);
        if d.is_empty() { 0.5 } else { (d.parse::<f32>().unwrap_or(50.0) / 100.0).clamp(0.0, 1.0) }
    });
    let ratchet = r
        .map(|p| digits_after(tok, p).parse::<usize>().unwrap_or(2).max(1))
        .unwrap_or(1);
    // accent: each trailing `!` multiplies velocity (`!`=x1.4, `!!`=x1.8…)
    let accent = e.map(|p| {
        let cnt = tok[p..].bytes().take_while(|&b| b == b'!').count();
        1.0 + 0.4 * cnt as f32
    }).unwrap_or(1.0);
    (&tok[..cut], prob, ratchet, accent, ramp_dir)
}

/// Velocity ramp across a ratchet's N sub-hits (crescendo +1 / decrescendo -1).
fn ramp_factor(j: usize, n: usize, dir: i8) -> f32 {
    if dir == 0 || n < 2 { return 1.0; }
    let t = j as f32 / (n - 1) as f32;
    if dir > 0 { 0.4 + 0.6 * t } else { 1.0 - 0.6 * t }
}

/// A/B variation: an alternation group `<a/b/c>` picks one alternative per bar,
/// cycling by bar index (so the pattern changes bar to bar). Any other token is
/// returned unchanged. The chosen alternative may itself carry `?`/`*` mods.
fn pick_alt(tok: &str, bar: i64) -> &str {
    if tok.len() >= 2 && tok.starts_with('<') && tok.ends_with('>') {
        let alts: Vec<&str> = tok[1..tok.len() - 1].split('/').collect();
        if !alts.is_empty() {
            return alts[bar.rem_euclid(alts.len() as i64) as usize];
        }
    }
    tok
}

/// Read the optional `seed: N` directive (default 0 = the canonical performance).
pub fn parse_seed(src: &str) -> u64 {
    for raw in src.lines() {
        let line = decomment(raw).trim();
        let Some(colon) = line.find(':') else { continue };
        if line[..colon].trim().eq_ignore_ascii_case("seed") {
            if let Ok(n) = line[colon + 1..].trim().parse::<u64>() { return n; }
        }
    }
    0
}

/// Default bars for a section in `song:` when no `*count` is given.
const DEFAULT_SECTION_BARS: u32 = 4;

/// Read the `song:`/`arrange:` arrangement: a list of `(section, bars)`, e.g.
/// `song: intro*4 verse*8 chorus*8 outro*4`. A bare `name` lasts 4 bars.
pub fn parse_song(src: &str) -> Vec<(String, u32)> {
    for raw in src.lines() {
        let line = decomment(raw).trim();
        let Some(colon) = line.find(':') else { continue };
        let head = line[..colon].trim().to_lowercase();
        if head != "song" && head != "arrange" { continue; }
        return line[colon + 1..]
            .split_whitespace()
            .filter_map(|tok| {
                let (name, bars) = match tok.split_once('*') {
                    Some((n, b)) => (n, b.parse::<u32>().unwrap_or(DEFAULT_SECTION_BARS)),
                    None => (tok, DEFAULT_SECTION_BARS),
                };
                if name.is_empty() { None } else { Some((name.to_lowercase(), bars.max(1))) }
            })
            .collect();
    }
    Vec::new()
}

/// Which section is playing at `bar`, looping over the whole arrangement.
fn section_at_bar(song: &[(String, u32)], bar: i64) -> Option<&str> {
    let total: u32 = song.iter().map(|(_, n)| *n).sum();
    if total == 0 { return None; }
    let mut pos = bar.rem_euclid(total as i64) as u32;
    for (name, n) in song {
        if pos < *n { return Some(name.as_str()); }
        pos -= *n;
    }
    None
}

/// Tempo in BPM from `bpm:`/`tempo: <n>`, default 120 (clamped 40..=300, as JS).
pub fn parse_bpm(src: &str) -> f64 {
    for raw in src.lines() {
        let line = raw.trim();
        let Some(colon) = line.find(':') else { continue };
        let h = line[..colon].trim().to_lowercase();
        if h == "bpm" || h == "tempo" {
            // first token must be a number (else it's a `tempo: name n …` map line)
            if let Ok(n) = line[colon + 1..].split_whitespace().next().unwrap_or("").parse::<f64>() {
                return n.clamp(40.0, 300.0);
            }
        }
    }
    120.0
}

/// Beats per bar from `meter:`. `meter: 7/8` -> 7*4/8 = 3.5 quarter-beats;
/// a bare `meter: 3` -> 3 beats. A **composed meter** may list its beat groups,
/// `meter: 3+2+2/8`, whose beats sum: 7/8 -> 3.5 quarter-beats (groups exposed
/// by [`parse_meter_groups`]). Default 4.
pub fn parse_meter(src: &str) -> f64 {
    parse_meter_groups(src).map(|(b, _)| b).unwrap_or(4.0)
}

/// `meter:` as beats-per-bar plus its beat groups, when written composed.
/// `meter: 3+2+2/8` -> `(3.5, Some(vec![3, 2, 2]))`; `meter: 7/8` -> `(3.5, None)`.
pub fn parse_meter_groups(src: &str) -> Option<(f64, Option<Vec<u32>>)> {
    for raw in src.lines() {
        let line = decomment(raw).trim();
        let Some(colon) = line.find(':') else { continue };
        if line[..colon].trim().to_lowercase() != "meter" { continue; }
        let v = line[colon + 1..].trim();
        if let Some((n, d)) = v.split_once('/') {
            let n = n.trim();
            let d = d.trim().parse::<f64>().ok()?;
            if d > 0.0 {
                // composed: sum of the `+`-separated groups -> beats = sum*4/d
                let groups: Vec<u32> = n.split('+').filter_map(|g| g.trim().parse::<u32>().ok().filter(|g| *g > 0)).collect();
                if !groups.is_empty() && groups.len() > 1 {
                    let beats = groups.iter().sum::<u32>() as f64 * 4.0 / d;
                    return Some((beats, Some(groups)));
                }
                if let Ok(n) = n.parse::<f64>() {
                    if n > 0.0 { return Some((n * 4.0 / d, None)); }
                }
            }
        } else if let Ok(n) = v.parse::<f64>() {
            if n > 0.0 { return Some((n, None)); }
        }
    }
    None
}

/// `accel: <from> <to> [bars]` — a linear tempo ramp. `bars` = 0 means "span the
/// whole arrangement (or 8 bars if there is none)".
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Accel { pub a: f64, pub b: f64, pub bars: u32 }

pub fn parse_accel(src: &str) -> Option<Accel> {
    for raw in src.lines() {
        let line = raw.trim();
        let Some(colon) = line.find(':') else { continue };
        if line[..colon].trim().to_lowercase() != "accel" { continue; }
        let t: Vec<f64> = line[colon + 1..].split_whitespace().filter_map(|w| w.parse::<f64>().ok()).collect();
        if t.len() >= 2 {
            return Some(Accel {
                a: t[0].clamp(40.0, 300.0),
                b: t[1].clamp(40.0, 300.0),
                bars: if t.len() >= 3 && t[2] > 1.0 { t[2].round() as u32 } else { 0 },
            });
        }
    }
    None
}

/// `tempo: <section> <bpm> …` — a per-section tempo map (name/bpm pairs).
pub fn parse_tempo_map(src: &str) -> Vec<(String, f64)> {
    for raw in src.lines() {
        let line = raw.trim();
        let Some(colon) = line.find(':') else { continue };
        if line[..colon].trim().to_lowercase() != "tempo" { continue; }
        let t: Vec<&str> = line[colon + 1..].split_whitespace().collect();
        let mut out = Vec::new();
        let mut k = 0;
        while k + 1 < t.len() {
            // name (non-numeric) followed by a number
            if t[k].parse::<f64>().is_err() {
                if let Ok(n) = t[k + 1].parse::<f64>() {
                    out.push((t[k].to_lowercase(), n.clamp(40.0, 300.0)));
                }
            }
            k += 2;
        }
        return out;
    }
    Vec::new()
}

/// Named groove templates: per-step microtiming offsets (fraction of a step),
/// mirroring the JS engine's GROOVES table exactly.
pub fn groove_offsets(name: &str) -> Option<Vec<f32>> {
    let v = match name {
        "dilla"    => vec![0.0, 0.11, -0.05, 0.07, 0.0, 0.09, -0.04, 0.06, 0.0, 0.11, -0.05, 0.07, 0.0, 0.09, -0.04, 0.06],
        "mpc"      => vec![0.0, 0.07, 0.0, 0.07, 0.0, 0.07, 0.0, 0.07],
        "swing16"  => vec![0.0, 0.13, 0.0, 0.13, 0.0, 0.13, 0.0, 0.13],
        "push"     => vec![-0.04],
        "laidback" => vec![0.05],
        "humanish" => vec![0.0, 0.03, -0.02, 0.02, 0.01, 0.03, -0.01, 0.02],
        _ => return None,
    };
    Some(v)
}

/// `groove: <name>` -> the resolved per-step offset table (empty if absent/unknown).
pub fn parse_groove(src: &str) -> Vec<f32> {
    for raw in src.lines() {
        let line = raw.trim();
        let Some(colon) = line.find(':') else { continue };
        if line[..colon].trim().to_lowercase() != "groove" { continue; }
        let w = line[colon + 1..].trim().to_lowercase();
        return groove_offsets(&w).unwrap_or_default();
    }
    Vec::new()
}

/// A whole parsed LOOM program: voices plus the optional key/seed/arrangement.
/// This is the convenient front door — `Program::parse` then `schedule_bar`.
#[derive(Debug, Clone)]
pub struct Program {
    pub voices: Vec<Voice>,
    pub key: Option<Key>,
    pub seed: u64,
    pub song: Vec<(String, u32)>,
    /// `key@section:` modulation map — a section plays in its own key.
    pub key_map: Vec<(String, Key)>,
    /// Base tempo in BPM (`bpm:`/`tempo:`).
    pub bpm: f64,
    /// Beats per bar (`meter:`), e.g. 3.5 for 7/8.
    pub meter_beats: f64,
    /// Optional tempo ramp (`accel:`).
    pub accel: Option<Accel>,
    /// Per-section tempo overrides (`tempo: name bpm …`).
    pub tempo_map: Vec<(String, f64)>,
    /// Resolved groove microtiming table (`groove: dilla`), empty if none.
    pub groove: Vec<f32>,
    /// Chord progression (`prog: Am F C G`), one chord per bar, followed by `^` tokens.
    pub prog: Vec<String>,
    /// Mix-wide settings: `duck`, `feel`, `warmth`, `room`, `bus`, `morph`, `loop`, `tune`.
    pub mix: MixSettings,
}

impl Program {
    pub fn parse(src: &str) -> Program {
        let expanded = expand_macros(src);
        let src = expanded.as_str();
        Program {
            voices: parse(src),
            key: parse_key(src),
            seed: parse_seed(src),
            song: parse_song(src),
            key_map: parse_key_map(src),
            bpm: parse_bpm(src),
            meter_beats: parse_meter(src),
            accel: parse_accel(src),
            tempo_map: parse_tempo_map(src),
            groove: parse_groove(src),
            prog: parse_prog(src),
            mix: MixSettings::parse(src),
        }
    }

    /// Total length of the arrangement in bars (0 if there is no `song:`).
    pub fn len_bars(&self) -> u32 { self.song.iter().map(|(_, n)| *n).sum() }

    /// Tempo in BPM at a given bar — honours `accel:` (linear ramp) and, when
    /// there is no ramp, per-section `tempo:` overrides. Mirrors the JS engine's
    /// `cycleLenForBar` exactly so both engines share one timeline.
    pub fn bar_bpm(&self, bar: i64) -> f64 {
        if let Some(ac) = self.accel {
            let tb = if ac.bars > 0 {
                ac.bars
            } else {
                let s = self.len_bars();
                if s > 0 { s } else { 8 }
            };
            let span = if tb > 1 { (tb - 1) as f64 } else { 1.0 };
            let f = (bar as f64 / span).clamp(0.0, 1.0);
            ac.a + (ac.b - ac.a) * f
        } else {
            section_at_bar(&self.song, bar)
                .and_then(|s| self.tempo_map.iter().find(|(n, _)| n == s).map(|(_, b)| *b))
                .unwrap_or(self.bpm)
        }
    }

    /// Duration in seconds of a given bar = (60 / bpm) * beats_per_bar.
    pub fn bar_seconds(&self, bar: i64) -> f64 {
        (60.0 / self.bar_bpm(bar)) * self.meter_beats
    }

    /// Section name playing at `bar` (None if no arrangement).
    pub fn section_at(&self, bar: i64) -> Option<&str> { section_at_bar(&self.song, bar) }

    /// Schedule one bar, honouring the arrangement: when a `song:` is present,
    /// a voice plays only if it has no `| in …` tag or its tag includes the
    /// current section. With no `song:`, every voice always plays.
    pub fn schedule_bar(&self, bar: i64) -> Vec<Event> {
        let active = self.active_voices(bar);
        // modulation: if this section has a key@section, resolve in that key
        let sec_now = section_at_bar(&self.song, bar);
        let cur_key = sec_now
            .and_then(|s| self.key_map.iter().find(|(n, _)| n == s).map(|(_, k)| k))
            .or(self.key.as_ref());
        sched_prog(&active, cur_key, &self.prog, bar, self.seed, &self.groove)
    }

    /// The voices that sound at `bar`: per-section variants (`name@section`)
    /// resolved and `| in …` applied (every base voice when there is no `song:`).
    /// The audio engines use it for the voices that are not notes (bytebeat).
    pub fn active_voices(&self, bar: i64) -> Vec<Voice> {
        if self.song.is_empty() {
            // no arrangement: play the base voices (section variants need a song)
            self.voices.iter().filter(|v| v.section.is_none()).cloned().collect()
        } else {
            let sec = section_at_bar(&self.song, bar);
            // For each base voice name, pick its variant for this section if one
            // exists (`name@section`), else the base voice; then apply `| in`.
            let mut seen: Vec<&str> = Vec::new();
            let mut out: Vec<Voice> = Vec::new();
            for v in &self.voices {
                if seen.contains(&v.name.as_str()) { continue; }
                seen.push(&v.name);
                let chosen = self.voices.iter()
                    .find(|w| w.name == v.name && w.section.as_deref() == sec)
                    .or_else(|| self.voices.iter().find(|w| w.name == v.name && w.section.is_none()));
                if let Some(c) = chosen {
                    let plays = match &c.sections {
                        None => true,
                        Some(set) => sec.is_some_and(|s| set.iter().any(|x| x == s)),
                    };
                    if plays { out.push(c.clone()); }
                }
            }
            out
        }
    }
}

/// Parse a chord progression directive (`prog: Am*2 F C`) -> one chord per bar
/// (cycling), expanding `*N` durations. Keeps original case (chords are case-sensitive).
pub fn parse_prog(src: &str) -> Vec<String> {
    for raw in src.lines() {
        let line = decomment(raw).trim();
        let Some(colon) = line.find(':') else { continue };
        if line[..colon].trim().to_lowercase() == "prog" {
            let mut out = Vec::new();
            for t in line[colon + 1..].split_whitespace() {
                let (name, n) = match t.split_once('*') {
                    Some((a, b)) => (a, b.parse::<usize>().unwrap_or(1).clamp(1, 64)),
                    None => (t, 1),
                };
                for _ in 0..n { out.push(name.to_string()); }
            }
            return out;
        }
    }
    Vec::new()
}

/// Resolve a `^` / `^N` token against the current bar's progression chord.
/// `^` -> the whole chord; `^N` -> the Nth chord-tone as a note (octave marks ok).
fn resolve_prog(tok: &str, prog: &[String], bar: i64, key: Option<&Key>, octave: i32) -> String {
    if prog.is_empty() || !tok.starts_with('^') { return tok.to_string(); }
    let cs = &prog[bar.rem_euclid(prog.len() as i64) as usize];
    let rest = &tok[1..];
    if rest.is_empty() || rest == "^" { return cs.clone(); }
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    let marks = &rest[digits.len()..];
    let Ok(nn) = digits.parse::<i32>() else { return cs.clone() };
    let notes: Vec<i32> = key
        .and_then(|k| harmony::roman_chord(cs, k))
        .or_else(|| parse_chord(cs, octave))
        .unwrap_or_default();
    if notes.is_empty() { return ".".to_string(); }
    let idx = nn - 1;
    let len = notes.len() as i32;
    let mut midi = notes[idx.rem_euclid(len) as usize] + 12 * idx.div_euclid(len);
    for c in marks.chars() { if c == '\'' { midi += 12; } else if c == ',' { midi -= 12; } }
    midi_to_name(midi)
}

/// Resolve one melodic/drum token into the note-name string(s) it sounds.
/// Order: scale degree → roman-numeral chord → chord symbol → plain note (the
/// first two need a key). Drums pass through. Chords return several notes.
fn resolve_token(tok: &str, is_drum: bool, octave: i32, key: Option<&Key>) -> Vec<String> {
    if is_drum { return vec![tok.to_string()]; }
    if let Some(k) = key {
        if is_degree(tok) {
            return vec![degree_to_midi(tok, k).map(|m| midi_to_name(m.round() as i32)).unwrap_or_else(|| tok.to_string())];
        }
        if let Some(notes) = harmony::roman_chord(tok, k) {
            return notes.into_iter().map(midi_to_name).collect();
        }
    }
    if let Some(notes) = parse_chord(tok, octave) {
        return notes.into_iter().map(midi_to_name).collect();
    }
    vec![tok.to_string()]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A named scale as f64 (Key.scale is now f64 to allow microtonal scales).
    fn fscale(name: &str) -> Vec<f64> {
        scale_intervals(name).unwrap().into_iter().map(|x| x as f64).collect()
    }

    #[test]
    fn parses_drum_and_melody() {
        let v = parse("kick: x . x .\nbass: c2 . g2 . | saw | gain 0.5");
        assert_eq!(v.len(), 2);
        assert!(v[0].is_drum);
        assert_eq!(v[0].tokens, ["x",".","x","."]);
        assert!(!v[1].is_drum);
        assert_eq!(v[1].wave, Wave::Saw);
        assert!((v[1].gain - 0.5).abs() < 1e-6);
    }

    #[test]
    fn rev_reverses() {
        let t = vec!["a".into(),"b".into(),"c".into()];
        assert_eq!(apply_verb(&t, "rev", 0, 0), vec!["c","b","a"]);
    }

    fn toks(s: &str) -> Vec<String> { s.split_whitespace().map(|x| x.to_string()).collect() }

    #[test]
    fn bytebeat_voice_parses_and_schedules_nothing() {
        let p = Program::parse("crunch: t: (t*5&t>>7)|(t*3&t>>10) | gain 0.4 | pan -0.5\nkick: x . x .");
        assert_eq!(p.voices.len(), 2);
        let v = &p.voices[0];
        let e = v.bytebeat.as_ref().expect("voix bytebeat");
        assert_eq!(e.src, "(t*5&t>>7)|(t*3&t>>10)");
        let t = 8001;
        assert_eq!(e.eval(&[t]), ((t * 5) & (t >> 7)) | ((t * 3) & (t >> 10)));
        assert_eq!((v.gain, v.pan), (0.4, Some(-0.5)));
        assert_eq!(v.tokens, vec!["~"]);
        assert!(v.verbs.is_empty());
        // aucun événement : seul le kick joue
        let ev = p.schedule_bar(0);
        assert!(ev.iter().all(|e| e.voice == "kick") && ev.len() == 2);
        // expression invalide : la voix est ignorée, le reste joue
        let p = Program::parse("bad: t: t*(2+\nok: t: t");
        assert_eq!(p.voices.iter().map(|v| v.name.as_str()).collect::<Vec<_>>(), vec!["ok"]);
        // une voix nommée t reste une voix normale
        assert!(parse("t: x . x .")[0].bytebeat.is_none());
    }

    /// (phase arrondie au millième, jeton) d'une voix sur une mesure.
    fn hits(p: &Program, voice: &str, bar: i64) -> Vec<(i32, String)> {
        p.schedule_bar(bar).into_iter().filter(|e| e.voice == voice)
            .map(|e| ((e.phase * 1000.0).round() as i32, e.token)).collect()
    }

    #[test]
    fn sync_restarts_a_poly_loop_on_every_bar_of_a_plain_leader() {
        // 3 jetons en poly 4 dérivent (a b c a | b c a b …) ; synchronisés sur
        // une voix simple (cycle d'une mesure), chaque mesure repart de « a ».
        let free = Program::parse("lead: c4 d4 e4 | poly 4\nkick: x . x .");
        assert_eq!(hits(&free, "lead", 1).iter().map(|h| h.1.as_str()).collect::<Vec<_>>(), vec!["d4", "e4", "c4", "d4"]);
        let p = Program::parse("lead: c4 d4 e4 | poly 4 | sync kick\nkick: x . x .");
        assert_eq!(p.voices[0].sync, Some(SyncTo { leader: "kick".into(), num: 1, den: 1 }));
        for bar in 0..4 {
            assert_eq!(hits(&p, "lead", bar).iter().map(|h| h.1.as_str()).collect::<Vec<_>>(), vec!["c4", "d4", "e4", "c4"], "bar {bar}");
        }
    }

    #[test]
    fn sync_follows_a_drifting_poly_leader() {
        // meneur : 5 jetons en poly 16 -> cycle de 5/16 de mesure, reprises aux
        // pas 0 5 10 15 20 25 30 ... ; le suiveur (3 jetons, poly 16) repart à chacune.
        let p = Program::parse("arp: c4 d4 e4 | poly 16 | sync lead\nlead: x . . . . | poly 16");
        assert_eq!(p.voices[0].sync, Some(SyncTo { leader: "lead".into(), num: 5, den: 16 }));
        let want = |bar: i64| -> Vec<String> {
            (0..16).map(|i| {
                let s = bar * 16 + i;
                let last = s / 5 * 5;
                ["c4", "d4", "e4"][((s - last) % 3) as usize].to_string()
            }).collect()
        };
        for bar in 0..3 {
            assert_eq!(hits(&p, "arp", bar).into_iter().map(|h| h.1).collect::<Vec<_>>(), want(bar), "bar {bar}");
        }
    }

    #[test]
    fn sync_rotates_a_plain_voice_mid_bar() {
        // meneur : 3 jetons en poly 4 -> cycle 3/4 de mesure ; à la mesure 1 il
        // repart au temps 1,5 mesure, soit au pas 2 d'une voix simple de 4 pas.
        let p = Program::parse("m: c4 d4 e4 f4 | sync l\nl: x . . | poly 4");
        assert_eq!(hits(&p, "m", 0).into_iter().map(|h| h.1).collect::<Vec<_>>(), vec!["c4", "d4", "e4", "c4"]);
        assert_eq!(hits(&p, "m", 1).into_iter().map(|h| h.1).collect::<Vec<_>>(), vec!["d4", "e4", "c4", "d4"]);
        // les positions ne bougent pas, seuls les jetons tournent
        assert_eq!(hits(&p, "m", 1).iter().map(|h| h.0).collect::<Vec<_>>(), vec![0, 250, 500, 750]);
    }

    #[test]
    fn sync_with_slow_leader_and_bad_names() {
        // meneur `slow 2` : cycle de 2 mesures, le suiveur ne repart qu'une mesure sur deux
        let p = Program::parse("a: c4 d4 e4 | poly 4 | sync b\nb: x . | slow 2");
        assert_eq!(p.voices[0].sync.as_ref().map(|s| (s.num, s.den)), Some((2, 1)));
        let toks = |bar| hits(&p, "a", bar).into_iter().map(|h| h.1).collect::<Vec<_>>();
        assert_eq!(toks(0), vec!["c4", "d4", "e4", "c4"]);
        assert_eq!(toks(1), vec!["d4", "e4", "c4", "d4"]);
        assert_eq!(toks(2), vec!["c4", "d4", "e4", "c4"]);
        // voix inconnue ou soi-même : ignoré, même résultat que sans sync
        let free = Program::parse("a: c4 d4 e4 | poly 4");
        for src in ["a: c4 d4 e4 | poly 4 | sync nobody", "a: c4 d4 e4 | poly 4 | sync a"] {
            let q = Program::parse(src);
            assert!(q.voices[0].sync.is_none());
            for bar in 0..3 { assert_eq!(hits(&q, "a", bar), hits(&free, "a", bar)); }
        }
        // un nombre reste le hard sync d'oscillateur
        let h = Program::parse("a: c4 | saw | sync 3\nb: x");
        assert!(h.voices[0].sync.is_none() && h.voices[0].synth.sync == Some(3.0));
        // meneur avec variante de section : la variante de base donne le cycle
        let v = Program::parse("song: x*1 y*1\nm: c4 d4 e4 | poly 4 | sync k\nk: x x | poly 4\nk@y: x | poly 4");
        assert_eq!(v.voices[0].sync.as_ref().map(|s| (s.num, s.den)), Some((2, 4)));
    }

    #[test]
    fn step_programs_compute_degrees_notes_and_hits() {
        // avec tonalité : un degré compté depuis 0 (0 = tonique, 7 = octave)
        let p = Program::parse("key: c major\nlead: =(i*2) | segment 4");
        assert_eq!(hits(&p, "lead", 0).into_iter().map(|h| h.1).collect::<Vec<_>>(), vec!["c3", "e3", "g3", "b3"]);
        // b = mesure, n = nombre de pas ; espaces et `|` permis dans les parenthèses
        let p = Program::parse("key: c major\nlead: =( (i + b) % n | 0 ) =(i) =(i) | saw");
        assert_eq!(hits(&p, "lead", 0).into_iter().map(|h| h.1).collect::<Vec<_>>(), vec!["c3", "d3", "e3"]);
        assert_eq!(hits(&p, "lead", 2).into_iter().map(|h| h.1).collect::<Vec<_>>(), vec!["e3", "d3", "e3"]);
        assert_eq!(p.voices[0].verbs, Vec::<String>::new());
        // sans tonalité : note MIDI ; octave négative et bornes
        let p = Program::parse("m: =(60+i*12) =(200) =(0-5)");
        assert_eq!(hits(&p, "m", 0).into_iter().map(|h| h.1).collect::<Vec<_>>(), vec!["c4", "g9", "c-1"]);
        let p = Program::parse("key: a minor\nm: =(0-1) =(7) =(99)");
        assert_eq!(hits(&p, "m", 0).into_iter().map(|h| h.1).collect::<Vec<_>>(), vec!["g3", "a4", "a8"]);
        // batterie : non nul = coup ; modificateurs après la parenthèse
        let p = Program::parse("kick: =(i%3==0) | segment 8\nhat: =(1)! =(1)*2 =(0)");
        assert_eq!(hits(&p, "kick", 0).iter().map(|h| h.0).collect::<Vec<_>>(), vec![0, 375, 750]);
        let hat: Vec<Event> = p.schedule_bar(0).into_iter().filter(|e| e.voice == "hat").collect();
        assert_eq!(hat.len(), 3);
        assert!(hat[0].vel > 1.0);
        // invalide ou non fermée : silence ; un jeton `=x` n'est pas un programme
        let p = Program::parse("key: c major\nm: =(i+) =(i 1 =(i)");
        assert_eq!(hits(&p, "m", 0).len(), 0);
        assert!(step_program("=x", false, true, 0, 1, 0).is_none());
        // dans un groupe [..] : i et n sont ceux du groupe ; en poly : l'indice de jeton
        let p = Program::parse("key: c major\nm: [=(i) =(i) =(n)] . | saw\nq: =(i) =(i+2) | poly 4");
        assert_eq!(hits(&p, "m", 0).into_iter().map(|h| h.1).collect::<Vec<_>>(), vec!["c3", "d3", "f3"]);
        assert_eq!(hits(&p, "q", 0).into_iter().map(|h| h.1).collect::<Vec<_>>(), vec!["c3", "f3", "c3", "f3"]);
        // les verbes s'appliquent avant : rev change la position donc la valeur
        let p = Program::parse("key: c major\nm: =(i) 5 | rev");
        assert_eq!(hits(&p, "m", 0).into_iter().map(|h| h.1).collect::<Vec<_>>(), vec!["g3", "d3"]);
    }

    #[test]
    fn bars_split_outside_parentheses_only() {
        assert_eq!(split_bars("a (b|c) | gain 1"), vec!["a (b|c) ", " gain 1"]);
        assert_eq!(split_bars("a ( | b"), vec!["a ( ", " b"]); // déséquilibré : coupe simple
        assert_eq!(tokenize_pat("=( i * 3 ) x e(3, 8)"), vec!["=( i * 3 )", "x", "e(3, 8)"]);
    }

    #[test]
    fn ply_repeats_each_event_inside_its_step() {
        assert_eq!(apply_verb(&toks("x . 3!"), "ply 3", 0, 0), vec!["[x x x]", ".", "[3! 3! 3!]"]);
        assert_eq!(apply_verb(&toks("x ."), "ply 1", 0, 0), vec!["x", "."]);
        assert_eq!(apply_verb(&toks("x"), "ply", 0, 0), vec!["[x x]"]); // default 2
        // onsets: 2 steps, ply 2 on the first -> hits at 0, 0.25 then rest
        let ev = schedule_bar(&parse("kick: x . | ply 2"), 0);
        let ph: Vec<f32> = ev.iter().map(|e| e.phase).collect();
        assert_eq!(ph, vec![0.0, 0.25]);
    }

    #[test]
    fn linger_loops_a_fraction_of_the_bar() {
        let t = toks("1 2 3 4 5 6 7 1'");
        assert_eq!(apply_verb(&t, "linger 0.25", 0, 0), toks("1 2 1 2 1 2 1 2"));
        assert_eq!(apply_verb(&t, "linger 0.75", 0, 0), toks("1 2 3 4 5 6 1 2"));
        assert_eq!(apply_verb(&t, "linger -0.25", 0, 0), toks("7 1' 7 1' 7 1' 7 1'"));
        assert_eq!(apply_verb(&t, "linger 1", 0, 0), t);
        assert_eq!(apply_verb(&toks("a b c"), "linger 0.1", 0, 0), toks("a a a")); // at least one step
    }

    #[test]
    fn segment_resamples_the_line() {
        assert_eq!(apply_verb(&toks("1 3 5"), "segment 6", 0, 0), toks("1 1 3 3 5 5"));
        assert_eq!(apply_verb(&toks("1 2 3 4"), "segment 2", 0, 0), toks("1 3"));
        assert_eq!(apply_verb(&toks("1 . 5"), "segment 4", 0, 0), toks("1 1 . 5"));
        assert_eq!(apply_verb(&toks("1 2"), "segment", 0, 0), toks("1 2")); // default: same length
    }

    #[test]
    fn range_rescales_degrees() {
        // span 1..5 (index 0..4) onto 1..3 (index 0..2): rounding half up
        assert_eq!(apply_verb(&toks("1 2 3 4 5"), "range 1 3", 0, 0), toks("1 2 2 3 3"));
        // stretch onto two octaves, mods kept, rests/notes untouched
        assert_eq!(apply_verb(&toks("1 . 3! c4 5?"), "range 1 1''", 0, 0), toks("1 . 1'! c4 1''?"));
        // integer bound 8 = one octave up; inverted range flips the contour
        assert_eq!(apply_verb(&toks("1 3 5"), "range 8 1", 0, 0), toks("1' 5 1")); // 3.5 rounds up
        // single-degree line goes to A; low octave marks parsed
        assert_eq!(apply_verb(&toks("4 4"), "range 2, 5", 0, 0), toks("2, 2,"));
        assert_eq!(apply_verb(&toks("x x"), "range 1 5", 0, 0), toks("x x"));
        assert_eq!(apply_verb(&toks("1 5"), "range", 0, 0), toks("1 5"));
    }

    #[test]
    fn transpose_add_is_diatonic() {
        assert_eq!(transpose_deg("1", 2), "3");      // a third up
        assert_eq!(transpose_deg("7", 2), "2'");     // wraps to next octave
        assert_eq!(transpose_deg("1", 7), "1'");     // octave
        assert_eq!(transpose_deg("1", -2), "6,");    // down to lower octave
        assert_eq!(transpose_deg("1'", 2), "3'");    // keeps octave mark
        assert_eq!(transpose_deg("3?40", 2), "5?40");// keeps probability mod
        assert_eq!(transpose_deg("c3", 2), "c3");    // leaves notes alone
        // as a verb across a pattern
        let t: Vec<String> = ["1","3","5"].iter().map(|s| s.to_string()).collect();
        assert_eq!(apply_verb(&t, "add 2", 0, 0), vec!["3","5","7"]);
    }

    #[test]
    fn chunk_applies_to_rotating_slice() {
        let t: Vec<String> = ["a","b","c","d"].iter().map(|s| s.to_string()).collect();
        // chunk 2 rev: reverse the active half (whole half reversed), rotating each bar
        assert_eq!(apply_verb(&t, "chunk 2 rev", 0, 0), vec!["b","a","c","d"]); // first half
        assert_eq!(apply_verb(&t, "chunk 2 rev", 1, 0), vec!["a","b","d","c"]); // second half
    }

    #[test]
    fn accents_set_velocity() {
        let v = parse("d: x x! x!! x");
        let ev = schedule_bar(&v, 0);
        let vels: Vec<f32> = ev.iter().map(|e| e.vel).collect();
        assert_eq!(vels, vec![1.0, 1.4, 1.8, 1.0]);
    }

    #[test]
    fn crescendo_roll_ramps_velocity() {
        let v = parse("d: x*4>");
        let ev = schedule_bar(&v, 0);
        let vels: Vec<f32> = ev.iter().map(|e| (e.vel * 100.0).round() / 100.0).collect();
        assert_eq!(vels, vec![0.4, 0.6, 0.8, 1.0]);
        let v2 = parse("d: x*4<");
        let vels2: Vec<f32> = schedule_bar(&v2, 0).iter().map(|e| (e.vel * 100.0).round() / 100.0).collect();
        assert_eq!(vels2, vec![1.0, 0.8, 0.6, 0.4]);
    }

    #[test]
    fn stacks_play_simultaneous_lines() {
        let k = parse_key("key: c major");
        // two `;`-separated lines, each 2 notes -> 4 events; both lines fire at phase 0
        let ev = schedule_bar_seeded(&parse("keys: 1 3 ; 5 7"), k.as_ref(), 0, 0);
        assert_eq!(ev.len(), 4);
        let at0: Vec<&String> = ev.iter().filter(|e| e.phase == 0.0).map(|e| &e.token).collect();
        assert_eq!(at0.len(), 2, "both stacked lines start together");
        // a single line (no ;) still works
        assert_eq!(schedule_bar_seeded(&parse("k: 1 3"), k.as_ref(), 0, 0).len(), 2);
    }

    #[test]
    fn prog_durations_and_chord_follow() {
        assert_eq!(parse_prog("prog: Am*2 F G"), vec!["Am", "Am", "F", "G"]);
        let prog = vec!["Am".to_string()];
        // ^ = the whole Am triad (3 notes at phase 0)
        let ev = schedule_bar_prog(&parse("pad: ^ . . ."), None, &prog, 0, 0);
        assert_eq!(ev.len(), 3);
        assert!(ev.iter().all(|e| e.phase == 0.0));
        // ^1 = the chord root -> a single A note
        let eb = schedule_bar_prog(&parse("bass: ^1 . . ."), None, &prog, 0, 0);
        assert_eq!(eb.len(), 1);
        assert!(eb[0].token.starts_with('a'), "root of Am is A, got {}", eb[0].token);
    }

    #[test]
    fn sub_sequences_subdivide_the_step() {
        let v = parse("hat: x [x x] x x");
        let ph: Vec<f32> = schedule_bar(&v, 0).iter().map(|e| (e.phase * 1000.0).round() / 1000.0).collect();
        assert_eq!(ph, vec![0.0, 0.25, 0.375, 0.5, 0.75]);
    }

    #[test]
    fn nested_sub_sequences() {
        let v = parse("hat: [x [x x]] x");
        let ph: Vec<f32> = schedule_bar(&v, 0).iter().map(|e| (e.phase * 1000.0).round() / 1000.0).collect();
        assert_eq!(ph, vec![0.0, 0.25, 0.375, 0.5]);
    }

    #[test]
    fn random_choice_is_deterministic_and_in_set() {
        let v = parse("hat: {x .} {x .} {x .} {x .}");
        let a: Vec<f32> = schedule_bar_seeded(&v, None, 0, 42).iter().map(|e| e.phase).collect();
        let b: Vec<f32> = schedule_bar_seeded(&v, None, 0, 42).iter().map(|e| e.phase).collect();
        assert_eq!(a, b, "same seed -> identical");
        // every emitted event sits on one of the 4 step phases (chosen 'x', never '.')
        for p in &a { assert!([0.0, 0.25, 0.5, 0.75].iter().any(|q| (q - p).abs() < 1e-4)); }
    }

    #[test]
    fn alternation_not_misread_as_roll() {
        // `<1/3>` must keep its closing `>` — not parsed as a crescendo
        let (base, _, _, _, dir) = token_mods("<1/3>");
        assert_eq!(base, "<1/3>");
        assert_eq!(dir, 0);
    }

    #[test]
    fn microtuning_matches_web_engine() {
        // major third (5/4) is ~13.7 cents flat in JI
        assert!((tune_cents(4, Tuning::Just) - (-13.69)).abs() < 0.01);
        // perfect fifth (3/2) is ~2 cents sharp in both JI and Pythagorean
        assert!((tune_cents(7, Tuning::Just) - 1.96).abs() < 0.01);
        assert!((tune_cents(7, Tuning::Pyth) - 1.96).abs() < 0.01);
        // historical temperaments match the JS tables exactly
        assert_eq!(tune_cents(4, Tuning::Werck), -10.0);
        assert_eq!(tune_cents(1, Tuning::Kirn), -10.0);
        assert_eq!(tune_cents(4, Tuning::Meantone), -14.0);
        assert_eq!(tune_cents(0, Tuning::Werck), 0.0);
        // equal temperament: no deviation
        assert_eq!(tune_cents(4, Tuning::Equal), 0.0);
        // 24-EDO (quarter tones): a 12-TET semitone is an exact EDO step -> 0 delta
        assert!((tune_cents(1, Tuning::Edo(24))).abs() < 0.01);
        // 19-EDO: the major third lands flat of 12-TET
        assert!(tune_cents(4, Tuning::Edo(19)) < 0.0);
        // octave stays pure under just intonation (a degree at the octave above the tonic)
        let f = degree_freq(60, 0, Tuning::Just);
        assert!((f - midi_to_freq(60)).abs() < 1e-3);
    }

    #[test]
    fn new_scales_resolve() {
        assert_eq!(scale_intervals("overtone"), Some(vec![0,2,4,6,7,9,10]));
        assert_eq!(scale_intervals("altered"), Some(vec![0,1,3,4,6,8,10]));
        assert_eq!(scale_intervals("messiaen3"), Some(vec![0,2,3,4,6,7,8,10,11]));
        assert_eq!(scale_intervals("ritusen"), Some(vec![0,2,5,7,9]));
        assert_eq!(scale_intervals("chinese"), Some(vec![0,4,6,7,11]));
        assert!(scale_intervals("not_a_scale").is_none());
    }

    #[test]
    fn fast_repeats() {
        let t = vec!["x".into(),".".into()];
        assert_eq!(apply_verb(&t, "fast 2", 0, 0).len(), 4);
    }

    #[test]
    fn palindrome_rot_iter_transforms() {
        let t = vec!["a".into(),"b".into(),"c".into(),"d".into()];
        // palindrome: forward on even bars, reversed on odd
        assert_eq!(apply_verb(&t, "palindrome", 0, 0), vec!["a","b","c","d"]);
        assert_eq!(apply_verb(&t, "palindrome", 1, 0), vec!["d","c","b","a"]);
        // rot 1: static left rotation
        assert_eq!(apply_verb(&t, "rot 1", 0, 0), vec!["b","c","d","a"]);
        assert_eq!(apply_verb(&t, "rot -1", 0, 0), vec!["d","a","b","c"]);
        // iter 4: shifts one step further each bar
        assert_eq!(apply_verb(&t, "iter 4", 0, 0), vec!["a","b","c","d"]);
        assert_eq!(apply_verb(&t, "iter 4", 1, 0), vec!["b","c","d","a"]);
        assert_eq!(apply_verb(&t, "iter 4", 2, 0), vec!["c","d","a","b"]);
    }

    #[test]
    fn probabilistic_verbs_are_deterministic() {
        let t = vec!["a".into(),"b".into()];
        // someby 0 never applies, someby 100 always applies (independent of seed)
        assert_eq!(apply_verb(&t, "someby 0 rev", 5, 7), vec!["a","b"]);
        assert_eq!(apply_verb(&t, "someby 100 rev", 5, 7), vec!["b","a"]);
        // same (bar,seed) -> same result, every time
        let x = apply_verb(&t, "sometimes rev", 3, 42);
        assert_eq!(x, apply_verb(&t, "sometimes rev", 3, 42));
    }

    #[test]
    fn every_applies_on_nth_bar() {
        let t = vec!["a".into(),"b".into()];
        // every 4 rev -> reversed only when bar % 4 == 3
        assert_eq!(apply_verb(&t, "every 4 rev", 0, 0), vec!["a","b"]);
        assert_eq!(apply_verb(&t, "every 4 rev", 3, 0), vec!["b","a"]);
    }

    #[test]
    fn schedules_only_hits() {
        let v = parse("kick: x . . x");
        let ev = schedule_bar(&v, 0);
        assert_eq!(ev.len(), 2);
        assert!((ev[0].phase - 0.0).abs() < 1e-6);
        assert!((ev[1].phase - 0.75).abs() < 1e-6);
    }

    #[test]
    fn middle_a_is_440() {
        assert!((note_to_freq("a4").unwrap() - 440.0).abs() < 0.01);
    }

    // --- music theory layer ---------------------------------------------

    fn close(a: Option<f32>, b: Option<f32>) -> bool {
        match (a, b) { (Some(x), Some(y)) => (x - y).abs() < 0.01, _ => false }
    }

    #[test]
    fn key_line_is_not_a_voice() {
        let v = parse("key: c minor\nkick: x . x .");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].name, "kick");
    }

    #[test]
    fn parses_key_with_mode() {
        let k = parse_key("key: c minor").unwrap();
        assert_eq!(k.tonic, 0);
        assert_eq!(k.scale, fscale("minor"));
    }

    #[test]
    fn key_defaults_to_major_and_scale_directive_overrides() {
        let k = parse_key("key: a").unwrap();
        assert_eq!(k.tonic, 9);
        assert_eq!(k.scale, fscale("major"));
        let k2 = parse_key("key: d\nscale: dorian").unwrap();
        assert_eq!(k2.tonic, 2);
        assert_eq!(k2.scale, fscale("dorian"));
    }

    #[test]
    fn parses_custom_microtonal_scale() {
        // The AFX∞ scale: quarter-tones in d. tonic=d, 8 fractional steps.
        let k = parse_key("key: d 0 1.5 3 4 6 7.5 9 10").unwrap();
        assert_eq!(k.tonic, 2);
        assert_eq!(k.scale, vec![0.0, 1.5, 3.0, 4.0, 6.0, 7.5, 9.0, 10.0]);
        // Degree 1 lands on the tonic exactly (d3 = midi 50).
        let f1 = token_to_freq("1", Some(&k)).unwrap();
        assert!((f1 - midi_f_to_freq(50.0)).abs() < 1e-3);
        // Degree 2 is a QUARTER-TONE up (step 1.5): midi 51.5, between e and eb.
        let f2 = token_to_freq("2", Some(&k)).unwrap();
        assert!((f2 - midi_f_to_freq(51.5)).abs() < 1e-3);
        // It really is microtonal: not equal to any 12-TET semitone.
        assert!((f2 - midi_to_freq(51)).abs() > 1.0);
        assert!((f2 - midi_to_freq(52)).abs() > 1.0);
        // `scale:`-only form also accepts numbers (tonic defaults to c).
        let k3 = parse_key("scale: 0 1 4 6 7.5 9 11").unwrap();
        assert_eq!(k3.tonic, 0);
        assert_eq!(k3.scale.len(), 7);
    }

    #[test]
    fn event_carries_exact_microtonal_freq() {
        // A degree on a quarter-tone scale: the token name rounds, but Event.freq
        // must keep the exact pitch so the audio backend plays the quarter-tone.
        let p = Program::parse("key: d 0 1.5 3 4 6 7.5 9 10\nlead: 1 2");
        let ev = p.schedule_bar(0);
        assert!((ev[0].freq.unwrap() - midi_f_to_freq(50.0)).abs() < 1e-3);   // d3
        assert!((ev[1].freq.unwrap() - midi_f_to_freq(51.5)).abs() < 1e-3);   // quarter-tone up
        // it is NOT a 12-TET semitone, proving the rounded token would be wrong
        assert!((ev[1].freq.unwrap() - midi_to_freq(51)).abs() > 1.0);
        assert!((ev[1].freq.unwrap() - midi_to_freq(52)).abs() > 1.0);
        // a drum has no pitch
        let pd = Program::parse("kick: x .");
        assert!(pd.schedule_bar(0)[0].freq.is_none());
    }

    #[test]
    fn no_directive_means_no_key() {
        assert!(parse_key("kick: x . x .").is_none());
    }

    #[test]
    fn degrees_resolve_against_c_major() {
        let k = parse_key("key: c major").unwrap();
        // degree 1 = c3, 5 = g3, 1' = c4 (one octave up)
        assert!(close(token_to_freq("1", Some(&k)), note_to_freq("c3")));
        assert!(close(token_to_freq("5", Some(&k)), note_to_freq("g3")));
        assert!(close(token_to_freq("1'", Some(&k)), note_to_freq("c4")));
        assert!(close(token_to_freq("1,", Some(&k)), note_to_freq("c2")));
    }

    #[test]
    fn minor_scale_supplies_flats() {
        let k = parse_key("key: c minor").unwrap();
        // 3rd of C minor is eb, 7th is bb — straight from the scale, no b syntax
        assert!(close(token_to_freq("3", Some(&k)), note_to_freq("eb3")));
        assert!(close(token_to_freq("7", Some(&k)), note_to_freq("bb3")));
    }

    #[test]
    fn cellular_automaton_evolves_per_bar() {
        let v = parse("ca1: ca(90,8)");
        // bar 0 = single centre seed -> exactly one hit
        let b0 = effective(&v[0], 0);
        assert_eq!(b0.len(), 8);
        assert_eq!(b0.iter().filter(|t| *t == "x").count(), 1);
        // it evolves: a later generation differs from the seed row
        let b1 = effective(&v[0], 1);
        assert_ne!(b0, b1);
    }

    #[test]
    fn euclidean_expands_to_tresillo() {
        let v = parse("kick: e(3,8)");
        assert_eq!(v[0].tokens, ["x",".",".","x",".",".","x","."]);
    }

    #[test]
    fn morse_and_fibonacci_generators() {
        // Thue-Morse: popcount parity of 0..8
        let m = parse("d: morse(8)");
        assert_eq!(m[0].tokens, [".","x","x",".","x",".",".","x"]);
        // Fibonacci word (0->01, 1->0), 0=x 1=.
        let f = parse("d: fib(8)");
        assert_eq!(f[0].tokens, ["x",".","x","x",".","x",".","x"]);
    }

    #[test]
    fn euclidean_rotation_and_mixing() {
        // rotation shifts the pattern; euclid tokens can sit next to plain ones
        let v = parse("hat: e(2,4,1) x");
        // E(2,4) = x . x . ; rotate left 1 = . x . x ; then a trailing x
        assert_eq!(v[0].tokens, [".","x",".","x","x"]);
    }

    #[test]
    fn world_scales_resolve() {
        // harmonic minor has a natural 7th (leading tone): C..B
        let hm = parse_key("key: c harmonic_minor").unwrap();
        assert!(close(token_to_freq("7", Some(&hm)), note_to_freq("b3")));
        // whole-tone: C D E F# ... degree 4 = f#
        let wt = parse_key("key: c whole_tone").unwrap();
        assert!(close(token_to_freq("4", Some(&wt)), note_to_freq("f#3")));
        // hijaz: C Db E F G Ab Bb — the b2 then major-3rd gap is its signature
        let hj = parse_key("key: c hijaz").unwrap();
        assert!(close(token_to_freq("2", Some(&hj)), note_to_freq("db3")));
        assert!(close(token_to_freq("3", Some(&hj)), note_to_freq("e3")));
        // blues scale has the b5 "blue note" as degree 4
        let bl = parse_key("key: c blues").unwrap();
        assert!(close(token_to_freq("4", Some(&bl)), note_to_freq("gb3")));
    }

    #[test]
    fn pentatonic_degree_wraps_octave() {
        let k = parse_key("key: c\nscale: pentatonic").unwrap();
        // 5-note scale [0,2,4,7,9]; degree 6 wraps to degree 1 an octave up
        assert!(close(token_to_freq("6", Some(&k)), note_to_freq("c4")));
    }

    #[test]
    fn absolute_notes_still_work_with_a_key() {
        let k = parse_key("key: c minor").unwrap();
        assert!(close(token_to_freq("g4", Some(&k)), note_to_freq("g4")));
    }

    #[test]
    fn degrees_dont_resolve_without_a_key() {
        // No key: a degree token is meaningless, so it yields no frequency.
        assert!(token_to_freq("1", None).is_none());
    }

    #[test]
    fn chromatic_alteration_raises_and_lowers_a_degree() {
        let cm = parse_key("key: c minor").unwrap();
        // 7th of C minor is bb; 7# is the raised leading tone = b natural.
        assert!(close(token_to_freq("7#", Some(&cm)), note_to_freq("b3")));
        let cmaj = parse_key("key: c major").unwrap();
        // major 3rd is e; 3b = eb. major 5th is g; 5b = gb (blue note).
        assert!(close(token_to_freq("3b", Some(&cmaj)), note_to_freq("eb3")));
        assert!(close(token_to_freq("5b", Some(&cmaj)), note_to_freq("gb3")));
        // alteration combines with octave marks: 7#' = b in the next octave up.
        assert!(close(token_to_freq("7#'", Some(&cm)), note_to_freq("b4")));
    }

    #[test]
    fn letter_b_note_is_not_mistaken_for_a_degree() {
        let cm = parse_key("key: c minor").unwrap();
        // "b3" starts with a letter -> it is the note B3, never a degree.
        assert!(!is_degree("b3"));
        assert!(close(token_to_freq("b3", Some(&cm)), note_to_freq("b3")));
    }

    #[test]
    fn per_voice_octave_shifts_degrees() {
        let v = parse("key: c major\nbass: 1 . 5 . | oct 2");
        assert_eq!(v[0].octave, Some(2));
        let k = parse_key("key: c major");
        let ev = schedule_bar_keyed(&v, k.as_ref(), 0);
        // degree 1 at oct 2 = c2, degree 5 = g2 (the key's default would be oct 3)
        let toks: Vec<&str> = ev.iter().map(|e| e.token.as_str()).collect();
        assert_eq!(toks, ["c2", "g2"]);
    }

    #[test]
    fn oct_does_not_affect_absolute_notes() {
        let v = parse("key: c major\nlead: c4 . e4 . | oct 2");
        let k = parse_key("key: c major");
        let ev = schedule_bar_keyed(&v, k.as_ref(), 0);
        // absolute note names carry their own octave; oct must not touch them
        let toks: Vec<&str> = ev.iter().map(|e| e.token.as_str()).collect();
        assert_eq!(toks, ["c4", "e4"]);
    }

    #[test]
    fn ratchet_subdivides_a_step() {
        // x*2 in a 2-step bar: step width 0.5, so sub-hits at 0.0 and 0.25.
        let v = parse("kick: x*2 .");
        let ev = schedule_bar_seeded(&v, None, 0, 0);
        assert_eq!(ev.len(), 2);
        assert!((ev[0].phase - 0.0).abs() < 1e-6);
        assert!((ev[1].phase - 0.25).abs() < 1e-6);
    }

    #[test]
    fn ratchet_works_on_notes_and_combines_with_probability() {
        // a single-step bar with a triple ratchet on degree 1 = three c3 hits
        let v = parse("key: c major\nlead: 1*3");
        let k = parse_key("key: c major");
        let ev = schedule_bar_seeded(&v, k.as_ref(), 0, 0);
        assert_eq!(ev.len(), 3);
        assert!(ev.iter().all(|e| e.token == "c3"));
        // p=0 ratchet never fires (probability is checked before the ratchet)
        let z = parse("kick: x?0*4");
        assert_eq!(schedule_bar_seeded(&z, None, 0, 0).len(), 0);
    }

    #[test]
    fn probability_extremes_are_deterministic() {
        // ?0 never fires, ?100 always fires — independent of the RNG.
        let never = parse("kick: x?0 x?0 x?0 x?0");
        assert_eq!(schedule_bar_seeded(&never, None, 0, 0).len(), 0);
        let always = parse("kick: x?100 x?100 x?100 x?100");
        assert_eq!(schedule_bar_seeded(&always, None, 0, 0).len(), 4);
    }

    #[test]
    fn probability_is_reproducible_per_seed() {
        let v = parse("hat: x?50 x?50 x?50 x?50 x?50 x?50 x?50 x?50");
        let a = schedule_bar_seeded(&v, None, 3, 42);
        let b = schedule_bar_seeded(&v, None, 3, 42);
        assert_eq!(a.len(), b.len()); // same seed+bar => identical
        // a different seed generally changes which hits survive (not all 8 / 0)
        let c = schedule_bar_seeded(&v, None, 3, 99);
        assert!(a.len() <= 8 && c.len() <= 8);
    }

    #[test]
    fn parses_seed_directive() {
        assert_eq!(parse_seed("seed: 7\nkick: x"), 7);
        assert_eq!(parse_seed("kick: x"), 0);
        // seed line is a directive, not a voice
        assert_eq!(parse("seed: 7\nkick: x").len(), 1);
    }

    #[test]
    fn audio_only_directives_are_not_voices() {
        // duck: is an audio-engine directive (sidechain), never a playable voice
        let v = parse("duck: 60\nkick: x x x x");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].name, "kick");
    }

    #[test]
    fn send_directive_is_not_a_voice() {
        // a `send:` return (mix directive) must not be scheduleable as a voice
        let v = parse("send: tails echo 4 0.5\nsend: airy reverb hall 0.85 4500\nkick: x . x . | send tails 0.6\npad: 1 5 3 | send airy 0.2");
        assert_eq!(v.len(), 2);
        assert!(v.iter().all(|x| x.name != "send" && x.name != "airy" && x.name != "tails"));
        // the `| send` verb is a sound verb, not a pattern verb: onsets are untouched
        let with = schedule_bar(&parse("k: 1 3 5 1 | send tails 0.5"), 0);
        let without = schedule_bar(&parse("k: 1 3 5 1"), 0);
        assert_eq!(with.iter().map(|e| e.phase).collect::<Vec<_>>(), without.iter().map(|e| e.phase).collect::<Vec<_>>());
    }

    #[test]
    fn song_arrangement_selects_sections() {
        let song = parse_song("song: intro*2 chorus*4 outro*2");
        assert_eq!(song, vec![("intro".into(),2),("chorus".into(),4),("outro".into(),2)]);
        assert_eq!(section_at_bar(&song, 0), Some("intro"));
        assert_eq!(section_at_bar(&song, 2), Some("chorus"));
        assert_eq!(section_at_bar(&song, 5), Some("chorus"));
        assert_eq!(section_at_bar(&song, 6), Some("outro"));
        // loops: bar 8 == bar 0
        assert_eq!(section_at_bar(&song, 8), Some("intro"));
    }

    #[test]
    fn per_section_variant_overrides_base() {
        let src = "song: intro*1 drop*1\nbass: 1 . . .\nbass@drop: 1 1 1 1";
        let prog = Program::parse(src);
        // intro -> base bass (1 hit), drop -> the @drop variant (4 hits)
        assert_eq!(prog.schedule_bar(0).iter().filter(|e| e.voice=="bass").count(), 1);
        assert_eq!(prog.schedule_bar(1).iter().filter(|e| e.voice=="bass").count(), 4);
    }

    #[test]
    fn program_mutes_voices_outside_their_sections() {
        let src = "song: intro*1 drop*1\nkick: x x x x\nlead: c4 c4 c4 c4 | in drop";
        let prog = Program::parse(src);
        assert_eq!(prog.len_bars(), 2);
        // bar 0 = intro: only kick (lead is `in drop`)
        let b0 = prog.schedule_bar(0);
        assert!(b0.iter().all(|e| e.voice == "kick"));
        // bar 1 = drop: kick (untagged, always) + lead
        let b1 = prog.schedule_bar(1);
        assert!(b1.iter().any(|e| e.voice == "kick"));
        assert!(b1.iter().any(|e| e.voice == "lead"));
    }

    #[test]
    fn no_song_means_every_voice_plays() {
        let prog = Program::parse("kick: x x\nlead: c4 c4 | in chorus");
        // no arrangement -> section tags are ignored, both voices play
        let ev = prog.schedule_bar(0);
        assert!(ev.iter().any(|e| e.voice == "lead"));
        assert!(ev.iter().any(|e| e.voice == "kick"));
    }

    #[test]
    fn arp_spreads_chord_across_the_step() {
        let v = parse("keys: Cm7 | arp up");
        assert_eq!(v[0].arp, Some(ArpMode::Up));
        let ev = schedule_bar_seeded(&v, None, 0, 0);
        // one-step bar, 4 chord notes -> phases 0, .25, .5, .75 in order
        assert_eq!(ev.iter().map(|e| e.token.clone()).collect::<Vec<_>>(), ["c3","eb3","g3","bb3"]);
        let phases: Vec<f32> = ev.iter().map(|e| e.phase).collect();
        assert!((phases[1] - 0.25).abs() < 1e-6 && (phases[3] - 0.75).abs() < 1e-6);
    }

    #[test]
    fn arp_down_and_updown_orders() {
        let down = parse("keys: C | arp down");
        let ev = schedule_bar_seeded(&down, None, 0, 0);
        assert_eq!(ev.iter().map(|e| e.token.clone()).collect::<Vec<_>>(), ["g3","e3","c3"]);
        let ud = parse("keys: Cm7 | arp updown");
        let ev2 = schedule_bar_seeded(&ud, None, 0, 0);
        // c eb g bb then back down without repeating ends: g eb
        assert_eq!(ev2.iter().map(|e| e.token.clone()).collect::<Vec<_>>(), ["c3","eb3","g3","bb3","g3","eb3"]);
    }

    #[test]
    fn ab_variation_alternates_per_bar() {
        // <x/.> is "x" on even bars, "." on odd bars.
        let v = parse("kick: <x/.> x");
        assert_eq!(schedule_bar_seeded(&v, None, 0, 0).len(), 2); // bar0: x x
        assert_eq!(schedule_bar_seeded(&v, None, 1, 0).len(), 1); // bar1: . x
        assert_eq!(schedule_bar_seeded(&v, None, 2, 0).len(), 2); // loops
        // alternatives can be degrees and cycle through 3 bars
        let m = parse("key: c major\nlead: <1/3/5>");
        let k = parse_key("key: c major");
        assert_eq!(schedule_bar_seeded(&m, k.as_ref(), 0, 0)[0].token, "c3");
        assert_eq!(schedule_bar_seeded(&m, k.as_ref(), 1, 0)[0].token, "e3");
        assert_eq!(schedule_bar_seeded(&m, k.as_ref(), 2, 0)[0].token, "g3");
    }

    #[test]
    fn polyrhythm_voices_subdivide_independently() {
        // a 3-step voice and a 4-step voice each fill the bar -> 3-against-4.
        let three = parse("a: x x x");
        let four = parse("b: x x x x");
        let e3 = schedule_bar_seeded(&three, None, 0, 0);
        let e4 = schedule_bar_seeded(&four, None, 0, 0);
        assert_eq!(e3.len(), 3);
        assert_eq!(e4.len(), 4);
        // their phases line up only at 0.0 (the downbeat)
        assert!((e3[1].phase - 1.0/3.0).abs() < 1e-6);
        assert!((e4[1].phase - 0.25).abs() < 1e-6);
    }

    #[test]
    fn poly_runs_own_loop_and_drifts() {
        // `| poly 13` over a 6-token list: 13 steps/bar, indexed by the GLOBAL
        // step (bar*13 + i), so the start drifts each bar (Reich phasing).
        let v = parse("fil: 1 4 6 3 5 1' | poly 13");
        assert_eq!(v[0].poly, Some(Poly::Meter(13)));
        let k = parse_key("key: c major");
        let e0 = schedule_bar_seeded(&v, k.as_ref(), 0, 0);
        assert_eq!(e0.len(), 13);                                  // N steps, not 6
        assert!((e0[1].phase - 1.0 / 13.0).abs() < 1e-6);         // even N-grid
        let toks0: Vec<&str> = e0.iter().map(|e| e.token.as_str()).collect();
        assert_eq!(&toks0[..6], &["c3", "f3", "a3", "e3", "g3", "c4"]);
        assert_eq!(toks0[6], "c3");                                // loop wraps at 6
        // bar 1 starts at global step 13 -> idx 13%6 = 1 -> "4" = f3 (it drifted)
        let e1 = schedule_bar_seeded(&v, k.as_ref(), 1, 0);
        assert_eq!(e1.len(), 13);
        assert_eq!(e1[0].token, "f3");
    }

    #[test]
    fn explicit_tuplet_polyrhythm_groups_m_in_n() {
        // `poly 3:2` = explicit 3-over-2: the bar is two 2-grid windows, each
        // holding a triplet, so six equally-spaced steps total but the three-
        // token phrase lands on every window (3 against the duple grid).
        let v = parse("car: 1 2 3 | poly 3:2");
        assert_eq!(v[0].poly, Some(Poly::Grid(3, 2)));
        let k = parse_key("key: c major");
        let e0 = schedule_bar_seeded(&v, k.as_ref(), 0, 0);
        assert_eq!(e0.len(), 6);                                  // n*m steps per bar
        // six step positions, evenly spread (1/6 apart): 1 2 3 1 2 3 — the
        // 3-token phrase restarts at each 2-grid window = three-against-two.
        let toks: Vec<&str> = e0.iter().map(|e| e.token.as_str()).collect();
        assert_eq!(&toks[..3], &["c3", "d3", "e3"]);
        assert_eq!(&toks[3..], &["c3", "d3", "e3"]);
        for (i, e) in e0.iter().enumerate() {
            assert!((e.phase - i as f32 / 6.0).abs() < 1e-6, "step {i}: {}", e.phase);
        }
        // a phrase of length != m still drifts against the window grid (bar 1
        // starts at global step 6 -> idx 6%4 = 2), Reich-style.
        let v2 = parse("car: 1 2 3 4 | poly 3:2");
        let k2 = parse_key("key: c major");
        let e1 = schedule_bar_seeded(&v2, k2.as_ref(), 1, 0);
        assert_eq!(e1.len(), 6);
        assert_eq!(e1[0].token, "e3");        // c3 d3 e3 f3 c3 d3 | f3 ...
    }

    #[test]
    fn explicit_tuplet_and_meter_produce_identical_spacing() {
        // `poly m:n` has the same global grid as `poly (n*m)`; what it changes is
        // the wording (and thus the phrase grouping against the bar).
        let a = parse("bell: a b c | poly 3:2");
        let b = parse("bell: a b c | poly 6");
        let k = parse_key("key: c major");
        let ea = schedule_bar_seeded(&a, k.as_ref(), 0, 0);
        let eb = schedule_bar_seeded(&b, k.as_ref(), 0, 0);
        assert_eq!(ea.len(), eb.len());
        assert_eq!(ea[1].phase, eb[1].phase);
        assert_eq!(a[0].poly, Some(Poly::Grid(3, 2)));
        assert_eq!(b[0].poly, Some(Poly::Meter(6)));
    }

    #[test]
    fn jux_splits_into_two_panned_layers() {
        // jux rev: original hard-left + reversed copy hard-right.
        let v = parse("m: 1 2 | jux rev");
        assert_eq!(v[0].jux.as_deref(), Some("rev"));
        let k = parse_key("key: c major");
        let ev = schedule_bar_seeded(&v, k.as_ref(), 0, 0);
        assert_eq!(ev.len(), 4);
        let left: Vec<&str> = ev.iter().filter(|e| e.pan.is_some_and(|p| (p + 0.82).abs() < 1e-6)).map(|e| e.token.as_str()).collect();
        let right: Vec<&str> = ev.iter().filter(|e| e.pan.is_some_and(|p| (p - 0.82).abs() < 1e-6)).map(|e| e.token.as_str()).collect();
        assert_eq!(left, ["c3", "d3"]);   // original, hard left
        assert_eq!(right, ["d3", "c3"]);  // reversed, hard right
    }

    #[test]
    fn off_adds_shifted_quieter_copy() {
        // off 1: a copy one step later, at 0.6x gain.
        let v = parse("m: 1 2 | off 1");
        assert!(matches!(v[0].off, Some((s, None)) if (s - 1.0).abs() < 1e-6));
        let k = parse_key("key: c major");
        let ev = schedule_bar_seeded(&v, k.as_ref(), 0, 0);
        assert_eq!(ev.len(), 4);
        let base: Vec<&Event> = ev.iter().filter(|e| (e.gain - 0.22).abs() < 1e-6).collect();
        let copy: Vec<&Event> = ev.iter().filter(|e| (e.gain - 0.22 * 0.6).abs() < 1e-4).collect();
        assert_eq!(base.len(), 2);
        assert_eq!(copy.len(), 2);
        assert!((base[0].phase - 0.0).abs() < 1e-6);
        assert!((copy[0].phase - 0.5).abs() < 1e-6);  // shifted +1 step (= +1/2 bar)
    }

    #[test]
    fn degrade_and_chance_drop_seeded() {
        // degrade thins hits, deterministically (same seed/bar -> same result)
        let full = schedule_bar_seeded(&parse("h: x x x x x x x x"), None, 0, 7);
        assert_eq!(full.len(), 8);
        let thin = schedule_bar_seeded(&parse("h: x x x x x x x x | degrade 50"), None, 0, 7);
        assert!(thin.len() < 8 && !thin.is_empty());
        let thin2 = schedule_bar_seeded(&parse("h: x x x x x x x x | degrade 50"), None, 0, 7);
        assert_eq!(thin.len(), thin2.len());
        // chance 0 = never plays, chance 1 = always plays (per bar)
        assert_eq!(schedule_bar_seeded(&parse("h: x x | chance 0"), None, 0, 7).len(), 0);
        assert_eq!(schedule_bar_seeded(&parse("h: x x | chance 1"), None, 5, 7).len(), 2);
    }

    #[test]
    fn roman_numerals_build_diatonic_chords() {
        let k = parse_key("key: c major");
        // I = C E G
        let i = schedule_bar_seeded(&parse("key: c major\nx: I"), k.as_ref(), 0, 0);
        assert_eq!(i.iter().map(|e| e.token.clone()).collect::<Vec<_>>(), ["c3","e3","g3"]);
        // V7 = G B D F (diatonic seventh)
        let v7 = schedule_bar_seeded(&parse("key: c major\nx: V7"), k.as_ref(), 0, 0);
        assert_eq!(v7.iter().map(|e| e.token.clone()).collect::<Vec<_>>(), ["g3","b3","d4","f4"]);
        // vi = A C E (minor, straight from the scale)
        let vi = schedule_bar_seeded(&parse("key: c major\nx: vi"), k.as_ref(), 0, 0);
        assert_eq!(vi.iter().map(|e| e.token.clone()).collect::<Vec<_>>(), ["a3","c4","e4"]);
    }

    #[test]
    fn roman_numerals_like_a_theory_class() {
        let k = parse_key("key: c major");
        let toks = |src: &str| schedule_bar_seeded(&parse(&format!("key: c major\nx: {src}")), k.as_ref(), 0, 0)
            .iter().map(|e| e.token.clone()).collect::<Vec<_>>();
        assert_eq!(toks("V7/V"), ["d3", "gb3", "a3", "c4"]);
        assert_eq!(toks("bVII"), ["bb3", "d4", "f4"]);
        assert_eq!(toks("iv"), ["f3", "ab3", "c4"]);
        assert_eq!(toks("N6"), ["f3", "ab3", "db4"]);
        // prog: with a secondary dominant, played with ^
        let src = "key: c major\nprog: I V7/ii ii V7\nx: ^";
        let p = parse(src);
        let prog = parse_prog(src);
        let bar1 = schedule_bar_prog(&p, k.as_ref(), &prog, 1, 0).iter().map(|e| e.token.clone()).collect::<Vec<_>>();
        assert_eq!(bar1, ["a3", "db4", "e4", "g4"]);
    }

    #[test]
    fn roman_needs_a_key_and_adapts_to_the_scale() {
        // no key: "i" is not a note, yields nothing playable
        assert!(parse_chord("i", 3).is_none());
        // in c minor, i = C Eb G (minor tonic)
        let k = parse_key("key: c minor");
        let i = schedule_bar_seeded(&parse("key: c minor\nx: i"), k.as_ref(), 0, 0);
        assert_eq!(i.iter().map(|e| e.token.clone()).collect::<Vec<_>>(), ["c3","eb3","g3"]);
    }

    #[test]
    fn chord_expands_to_simultaneous_notes() {
        // No key needed: Cm7 (uppercase) -> c eb g bb, all at the same phase.
        let v = parse("pad: Cm7 . | sine");
        let ev = schedule_bar_keyed(&v, None, 0);
        let toks: Vec<&str> = ev.iter().map(|e| e.token.as_str()).collect();
        assert_eq!(toks, ["c3", "eb3", "g3", "bb3"]);
        assert!(ev.iter().all(|e| e.phase == 0.0));
    }

    #[test]
    fn extended_chords_and_inversions() {
        // 13th chord stacks the extensions
        let v = parse("k: C13");
        let ev = schedule_bar_seeded(&v, None, 0, 0);
        assert_eq!(ev.iter().map(|e| e.token.clone()).collect::<Vec<_>>(), ["c3","e3","g3","bb3","d4","a4"]);
        // power chord
        let p = parse("k: G5");
        assert_eq!(schedule_bar_seeded(&p, None, 0, 0).iter().map(|e| e.token.clone()).collect::<Vec<_>>(), ["g3","d4"]);
        // slash chord puts the named bass below: C/E -> e2 c3 e3 g3
        let s = parse("k: C/E");
        assert_eq!(schedule_bar_seeded(&s, None, 0, 0).iter().map(|e| e.token.clone()).collect::<Vec<_>>(), ["e2","c3","e3","g3"]);
    }

    #[test]
    fn new_scales_exist() {
        assert!(scale_intervals("octatonic").is_some());
        assert!(scale_intervals("hungarian_minor").is_some());
        assert!(scale_intervals("phrygian_dominant").is_some());
        assert_eq!(scale_intervals("augmented").unwrap(), vec![0,3,4,7,8,11]);
    }

    #[test]
    fn bare_uppercase_is_a_major_triad() {
        let v = parse("pad: G .");
        let ev = schedule_bar_keyed(&v, None, 0);
        assert_eq!(ev.iter().map(|e| e.token.clone()).collect::<Vec<_>>(), ["g3","b3","d4"]);
    }

    #[test]
    fn uppercase_note_with_octave_stays_a_note() {
        // "C3" carries an octave so it is NOT a chord — it is the note C3.
        let v = parse("lead: C3 .");
        let ev = schedule_bar_keyed(&v, None, 0);
        assert_eq!(ev.len(), 1);
        assert!(close(note_to_freq(&ev[0].token), note_to_freq("c3")));
    }

    #[test]
    fn chord_respects_voice_octave() {
        let v = parse("pad: Am . | oct 2");
        let ev = schedule_bar_keyed(&v, None, 0);
        assert_eq!(ev.iter().map(|e| e.token.clone()).collect::<Vec<_>>(), ["a2","c3","e3"]);
    }

    #[test]
    fn schedule_resolves_degrees_into_note_names() {
        let v = parse("key: c minor\nbass: 1 . 3 . 5 . 3 .");
        let k = parse_key("key: c minor\nbass: 1 . 3 . 5 . 3 .");
        let ev = schedule_bar_keyed(&v, k.as_ref(), 0);
        // hits at degrees 1,3,5,3 → c3, eb3, g3, eb3
        let toks: Vec<&str> = ev.iter().map(|e| e.token.as_str()).collect();
        assert_eq!(toks, ["c3", "eb3", "g3", "eb3"]);
    }

    #[test]
    fn maths_generators() {
        let g = |s: &str| expand_euclid(vec![s.to_string()]).join("");
        assert_eq!(g("prime(8)"), "..xx.x.x");      // primes: 2,3,5,7
        assert_eq!(g("cantor(9)"), "x.x...x.x");     // Cantor set (no base-3 digit '1')
        assert_eq!(g("collatz(1)"), "x");
        assert_eq!(g("golden(8)").len(), 8);
    }

    #[test]
    fn lsys_is_backwards_compatible_and_custom() {
        let g = |s: &str| expand_euclid(vec![s.to_string()]).join(" ");
        // bare form unchanged: bit-identical sequence to the fixed table
        assert_eq!(g("lsys(8)"), "1 2 3 1 5 2 1 2");
        assert_eq!(g("l-sys(8)"), g("lsys(8)"));
        // custom deterministic grammar: 1>1.2.5
        assert_eq!(g("l-sys(6,1>1.2.5)"), "1 2 5 2 5 2");
        assert_eq!(g("l-sys(6,1>1.2,2>1.3)"), "1 2 1 3 1 2");
        // custom axiom: 5 grows from its own seed
        assert_eq!(g("l-sys(5,5,5>1.5,1>5)"), "1 5 5 1 5");
    }

    #[test]
    fn xys_is_seeded_and_weighted() {
        let a = expand_xys_token("xys(12,1,1>2:2|3:1,2>1:1,3>2:1)", 3, 99).unwrap();
        let b = expand_xys_token("xys(12,1,1>2:2|3:1,2>1:1,3>2:1)", 3, 99).unwrap();
        let c = expand_xys_token("xys(12,1,1>2:2|3:1,2>1:1,3>2:1)", 3, 7).unwrap();
        assert_eq!(a, b);                       // deterministic per seed
        assert_eq!(a.len(), 12);
        assert_ne!(a, c);                       // seed changes the walk
        assert!(a.iter().all(|s| ["1", "2", "3"].contains(&s.as_str())));
        // a symbol with no exit falls back to the terminal: stays
        let d = expand_xys_token("xys(6,9,9>1:1)", 0, 1).unwrap();
        assert_eq!(d, vec!["9", "1", "1", "1", "1", "1"]);
    }

    #[test]
    fn evolve_is_seeded_and_improves() {
        let a = expand_evolve_token("evolve(16)", 2, 42).unwrap();
        let b = expand_evolve_token("evolve(16)", 2, 42).unwrap();
        let c = expand_evolve_token("evolve(16)", 2, 9).unwrap();
        assert_eq!(a, b);                       // deterministic per seed
        assert_eq!(a.len(), 16);
        assert_ne!(a, c);                       // seed changes the lineage
        // earlier evaluation of `evolve` without generations is smoother than raw
        // random: assert all degrees stay in octave-range (1..=7)
        assert!(a.iter().all(|s| (1..=7).contains(&s.parse::<i64>().unwrap())));
        // fitness of the evolved champion >= fitness of a plain random sequence
        let n = 16; let maxd = 7i64;
        let fit = |v: &Vec<String>| -> i64 {
            let deg: Vec<i64> = v.iter().map(|s| s.parse().unwrap()).collect();
            let mut f = 0i64; let mut prev = deg[0];
            for &x in &deg[1..] { let d = (x - prev).abs(); f += match d { 0 => 2, 1 => 3, 2 => 2, _ => 0 } - d; prev = x; }
            let mut seen = [false; 7];
            for w in deg.windows(2) { let d = ((w[1] - w[0]).abs() % 7) as usize; seen[d] = true; }
            f + seen.iter().filter(|b| **b).count() as i64 * 2
        };
        let champ = fit(&a);
        let mut rand_fit = 0i64;
        for s in 0..10u64 {
            let mut v = Vec::new();
            for i in 0..n { v.push(((js_rng(s as u32, 2, "evolve", i as i64) * maxd as f64) as u8) + 1); }
            rand_fit = rand_fit.max(fit(&v.iter().map(|d| d.to_string()).collect()));
        }
        assert!(champ >= rand_fit);
    }

    #[test]
    fn generators_match_js_engine() {
        let g = |s: &str| expand_euclid(vec![s.to_string()]).join(" ");
        assert_eq!(g("lsys(8)"), "1 2 3 1 5 2 1 2");
        assert_eq!(g("pi(16)"), "3 1 4 1 5 2 2 6 5 3 5 1 2 7 2 3");
        assert_eq!(g("recaman(12)"), "1 2 4 7 3 1 7 7 6 1 5 2");
        assert_eq!(g("logistic(12)"), "7 1 3 6 4 7 1 3 7 2 5 6");
        assert_eq!(expand_euclid(vec!["trib(16)".into()]).join(""), "x.x.x.xx.x.x.x.x");
        assert_eq!(expand_euclid(vec!["bin(173)".into()]).join(""), "x.x.xx.x");
        // perlin uses sin() -> assert length only (degree may differ by a ULP across runtimes)
        assert_eq!(expand_perlin_token("perlin(12)", 5).unwrap().len(), 12);
    }

    #[test]
    fn modulation_key_at_section() {
        // verse in A minor, chorus modulates to C minor — degree 1 follows the section's key
        let p = Program::parse("song: verse*1 chorus*1\nkey: a minor\nkey@chorus: c minor\nlead: 1 . . .");
        let v0 = p.schedule_bar(0); // verse -> A
        let v1 = p.schedule_bar(1); // chorus -> C
        assert_eq!(v0[0].token, "a3");
        assert_eq!(v1[0].token, "c3");
    }

    #[test]
    fn meter_parse() {
        assert_eq!(parse_meter("meter: 7/8"), 3.5);   // 7 eighths = 3.5 quarters
        assert_eq!(parse_meter("meter: 5/4"), 5.0);
        assert_eq!(parse_meter("meter: 3"), 3.0);     // bare beat count
        assert_eq!(parse_meter("bpm: 120"), 4.0);     // default
        // composed meter: beats sum, groups are kept
        assert_eq!(parse_meter("meter: 3+2+2/8"), 3.5);   // 7/8 in 3+2+2
        assert_eq!(parse_meter_groups("meter: 3+2+2/8"), Some((3.5, Some(vec![3, 2, 2]))));
        assert_eq!(parse_meter_groups("meter: 7/8"), Some((3.5, None)));
        assert_eq!(parse_meter_groups("meter: 3"), Some((3.0, None)));
        assert_eq!(parse_meter_groups("bpm: 120"), None);
    }

    #[test]
    fn tempo_bpm_and_per_section() {
        // per-section tempo map, no ramp
        let p = Program::parse("bpm: 120\ntempo: verse 90 chorus 160\nsong: verse*2 chorus*2\nx: x");
        assert_eq!(p.bpm, 120.0);
        assert_eq!(p.bar_bpm(0), 90.0);   // verse
        assert_eq!(p.bar_bpm(2), 160.0);  // chorus
        assert!((p.bar_seconds(0) - (60.0 / 90.0) * 4.0).abs() < 1e-9);
    }

    #[test]
    fn accel_ramp_matches_js() {
        // accel over an explicit 11-bar span: 100 -> 200 linearly, meter 7/8.
        let p = Program::parse("bpm: 120\nmeter: 7/8\naccel: 100 200 11\nsong: a*11\nx: x");
        assert_eq!(p.meter_beats, 3.5);
        assert_eq!(p.bar_bpm(0), 100.0);                 // f=0
        assert_eq!(p.bar_bpm(10), 200.0);                // f=1 (span=10)
        assert!((p.bar_bpm(5) - 150.0).abs() < 1e-9);    // halfway
        assert!((p.bar_seconds(0) - (60.0 / 100.0) * 3.5).abs() < 1e-9);
        // bars=0 -> span the whole arrangement (here 11 bars too via len_bars fallback)
        let p2 = Program::parse("accel: 132 150\nmeter: 7/8\nsong: emerge*4 drift*6 fracture*6 bloom*6 collapse*4 swarm*6 dissolve*6 ghost*4\nx: x");
        assert_eq!(p2.len_bars(), 42);
        assert_eq!(p2.bar_bpm(0), 132.0);
        assert_eq!(p2.bar_bpm(41), 150.0);
    }

    #[test]
    fn swing_offsets_odd_steps() {
        // swing 50 -> odd 16ths land a half-step late; even steps stay put.
        let v = parse("hat: x x x x | swing 50");
        assert_eq!(v[0].swing, 0.5);
        let ph: Vec<f32> = schedule_bar(&v, 0).iter().map(|e| e.phase).collect();
        let want = [0.0, 0.375, 0.5, 0.875];
        for (a, b) in ph.iter().zip(want.iter()) { assert!((a - b).abs() < 1e-6, "{a} vs {b}"); }
    }

    #[test]
    fn nudge_shifts_whole_voice() {
        // shift 0.2 -> every hit slides 0.2 of a step later (0.2/n of the bar).
        let v = parse("bass: x x | shift 0.2");
        assert_eq!(v[0].nudge, 0.2);
        let ph: Vec<f32> = schedule_bar(&v, 0).iter().map(|e| e.phase).collect();
        assert!((ph[0] - 0.1).abs() < 1e-6);
        assert!((ph[1] - 0.6).abs() < 1e-6);
        // a bare voice (no shift) is untouched
        let v2 = parse("bass: x x");
        let ph2: Vec<f32> = schedule_bar(&v2, 0).iter().map(|e| e.phase).collect();
        assert!((ph2[0] - 0.0).abs() < 1e-6 && (ph2[1] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn groove_template_applies_per_step() {
        assert_eq!(parse_groove("groove: mpc"), vec![0.0, 0.07, 0.0, 0.07, 0.0, 0.07, 0.0, 0.07]);
        assert!(parse_groove("groove: nope").is_empty());
        let p = Program::parse("groove: mpc\nhat: x x x x");
        assert_eq!(p.groove, vec![0.0, 0.07, 0.0, 0.07, 0.0, 0.07, 0.0, 0.07]);
        let ph: Vec<f32> = p.schedule_bar(0).iter().map(|e| e.phase).collect();
        let want = [0.0, 0.25 + 0.07 / 4.0, 0.5, 0.75 + 0.07 / 4.0];
        for (a, b) in ph.iter().zip(want.iter()) { assert!((a - b).abs() < 1e-6, "{a} vs {b}"); }
    }

    #[test]
    fn seeded_verbs_match_js_engine() {
        let t: Vec<String> = "1 2 3 5 6 5 3 2".split_whitespace().map(|s| s.to_string()).collect();
        assert_eq!(apply_verb(&t, "mutate 30", 3, 5).join(" "), "1 2 2 5 5 3 3 2");
        assert_eq!(apply_verb(&t, "shuffle", 3, 5).join(" "), "1 2 5 6 5 3 3 2");
        assert_eq!(apply_verb(&t, "markov", 3, 5).join(" "), "1 2 1 2 1 2 3 5");
        let q: Vec<String> = "1 2 3 5".split_whitespace().map(|s| s.to_string()).collect();
        assert_eq!(apply_verb(&q, "invert", 0, 5).join(" "), "1 7, 6, 4,");
        let s3: Vec<String> = "1 2 3".split_whitespace().map(|s| s.to_string()).collect();
        assert_eq!(apply_verb(&s3, "sequence 2", 0, 5).join(" "), "1 2 3 3 4 5");
    }

    #[test]
    fn markov_order_n() {
        let t: Vec<String> = "1 2 3 1 2 4 1 2 5".split_whitespace().map(|s| s.to_string()).collect();
        let a = apply_verb(&t, "markov 2", 3, 5);
        assert_eq!(a, apply_verb(&t, "markov 2", 3, 5));  // deterministic
        assert_eq!(a.len(), t.len());
        // order-1 (no arg) unchanged — regression
        let q: Vec<String> = "1 2 3 5 6 5 3 2".split_whitespace().map(|s| s.to_string()).collect();
        assert_eq!(apply_verb(&q, "markov", 3, 5).join(" "), "1 2 1 2 1 2 3 5");
    }

    #[test]
    fn constraint_solver_honors_the_rules() {
        // solve(12,target:1,ladder:2,range:1-5) — un VRAI solveur : le dernier pas
        // est la cible, chaque enchaînement reste à 2 degrés au plus (marche), tout
        // reste dans l'ambitus 1-5 — et c'est déterministe.
        let get = || expand_solve_token("solve(12,target:1,ladder:2,range:1-5)", 3, 5).unwrap();
        let a = get();
        assert_eq!(a.len(), 12);
        assert_eq!(a.last().map(|s| s.as_str()), Some("1"));
        let d: Vec<i64> = a.iter().map(|s| s.parse().unwrap()).collect();
        for w in d.windows(2) {
            assert!((w[0] - w[1]).abs() <= 2, "{a:?} : saut {}>{}", w[0], w[1]);
        }
        for &x in &d {
            assert!((1..=5).contains(&x), "{a:?} : {x} hors 1-5");
        }
        assert_eq!(a, get(), "déterministe");
        // sans contrainte : les défauts résolvent quand même sur la tonique 1, en marches
        let b = expand_solve_token("solve(8)", 3, 5).unwrap();
        assert_eq!(b.len(), 8);
        assert_eq!(b.last().map(|s| s.as_str()), Some("1"));
        let db: Vec<i64> = b.iter().map(|s| s.parse().unwrap()).collect();
        for w in db.windows(2) {
            assert!((w[0] - w[1]).abs() <= 2);
        }
        // arc : un pic dans les 2/3 puis une descente qui finit sur la cible
        let arc = expand_solve_token("solve(16,target:1,ladder:2,range:1-7,arc)", 0, 9).unwrap();
        let da: Vec<i64> = arc.iter().map(|s| s.parse().unwrap()).collect();
        let peak_idx = da[..da.len() - 1].iter().enumerate().max_by_key(|(_, v)| *v).map(|(i, _)| i).unwrap();
        assert!(peak_idx * 3 < da.len() * 2, "arc : pic trop tard {arc:?}");
        assert_eq!(da.last(), Some(&1));
        for w in da.windows(2) {
            assert!((w[0] - w[1]).abs() <= 2);
        }
    }

    #[test]
    fn markov_learns_from_a_piece() {
        // « chaînes apprises d'un morceau » : le morceau = la phrase donnée, la
        // ligne générée garde la longueur de la voix et n'utilise que les paires
        // du morceau appris.
        let t: Vec<String> = "3 5 7".split_whitespace().map(|s| s.to_string()).collect();
        let phrase = "1 2 1 3 1 2 4";
        let a = apply_verb(&t, &format!("markov learn: {phrase}"), 3, 5);
        assert_eq!(a.len(), t.len());
        assert_eq!(a, apply_verb(&t, &format!("markov learn: {phrase}"), 3, 5)); // deterministic
        // chaque transition vient des paires du morceau appris (ou bien un simple chiffre tombé par hasard)
        let pairs: Vec<(String, String)> = "1 2 1 3 1 2 4".split_whitespace().map(|s| s.to_string()).collect::<Vec<_>>().windows(2).map(|w| (w[0].clone(), w[1].clone())).collect();
        for w in a.windows(2) {
            let pair = (w[0].clone(), w[1].clone());
            assert!(pairs.contains(&pair), "transition {pair:?} hors du morceau appris");
        }
        // une phrase trop courte ne tombe pas en boucle : la voix revient à elle-même
        let short = apply_verb(&t, "markov learn: 5", 3, 5);
        assert_eq!(short.len(), t.len());
        // sans `learn:`, comportement inchangé (régression)
        let q: Vec<String> = "1 2 3 5 6 5 3 2".split_whitespace().map(|s| s.to_string()).collect();
        assert_eq!(apply_verb(&q, "markov", 3, 5).join(" "), "1 2 1 2 1 2 3 5");
    }

    #[test]
    fn markov_order_n_learns_from_a_piece() {
        let t: Vec<String> = "3 5 7 1'".split_whitespace().map(|s| s.to_string()).collect();
        let a = apply_verb(&t, "markov 2 learn: 1 2 1 3 1 2 4 1 2 5", 3, 5);
        assert_eq!(a.len(), t.len());
        assert_eq!(a, apply_verb(&t, "markov 2 learn: 1 2 1 3 1 2 4 1 2 5", 3, 5));
        // l'ordre reste 2 (pas de régression sur markov 2 sans learn)
        let q: Vec<String> = "1 2 3 1 2 4 1 2 5".split_whitespace().map(|s| s.to_string()).collect();
        assert_eq!(apply_verb(&q, "markov 2", 3, 5).len(), q.len());
    }

    #[test]
    fn develop_evolves_each_bar() {
        // a section that develops: the material transforms bar by bar (an arc of
        // rot/add/rev/invert), so it never just repeats.
        let t: Vec<String> = "1 2 3".split_whitespace().map(|s| s.to_string()).collect();
        assert_eq!(apply_verb(&t, "develop", 0, 0).join(" "), "1 2 3");   // original
        assert_eq!(apply_verb(&t, "develop", 1, 0).join(" "), "2 3 1");   // rot 1
        assert_eq!(apply_verb(&t, "develop", 2, 0).join(" "), "3 4 5");   // add 2 (sequence up)
        assert_eq!(apply_verb(&t, "develop", 3, 0).join(" "), "3 2 1");   // rev (retrograde)
        // consecutive bars differ -> genuine evolution
        let bars: Vec<String> = (0..6).map(|b| apply_verb(&t, "develop", b, 0).join(" ")).collect();
        assert!(bars.iter().collect::<std::collections::HashSet<_>>().len() >= 5);
    }

    #[test]
    fn inline_comments_stripped() {
        // a # after whitespace is a comment; the voice keeps its real tokens
        let v = parse("lead: 1 2 3 5 # une ligne\nkick: x . x . # beat");
        assert_eq!(v[0].tokens.join(" "), "1 2 3 5");
        assert_eq!(v[1].tokens.join(" "), "x . x .");
        // 7# accidental is NOT a comment (no space before #)
        assert_eq!(parse("m: 1 7# 1")[0].tokens.join(" "), "1 7# 1");
        // song / meter ignore the trailing comment
        assert_eq!(parse_song("song: a*2 b*2 # forme").iter().map(|(n,_)|n.clone()).collect::<Vec<_>>(), ["a","b"]);
        assert_eq!(parse_meter("meter: 7/8 # boiteux"), 3.5);
    }

    #[test]
    fn tonnetz_walk() {
        // C with PLR: C -> P -> Cm -> L -> Ab -> R -> Fm
        assert_eq!(parse("h: tonnetz(4,C,PLR)")[0].tokens.join(" "), "C Cm Ab Fm");
        assert_eq!(parse("h: tonnetz(3,A,P)")[0].tokens.join(" "), "A Am A");  // P flips back and forth
        assert_eq!(parse("h: tonnetz(2,Eb)")[0].tokens.len(), 2);
    }

    #[test]
    fn hocket_complement() {
        // complement: hits<->rests. A voice + its | hocket copy tile the bar.
        let v = |s: &str| s.split_whitespace().map(|x| x.to_string()).collect::<Vec<_>>();
        assert_eq!(apply_verb(&v("x . x ."), "hocket", 0, 0).join(" "), ". x . x");
        assert_eq!(apply_verb(&v("x . . x . . x ."), "hocket", 0, 0).join(" "), ". x x . x x . x");
    }

    #[test]
    fn spectrum_overtones() {
        // the overtone series of c2 as nearest note names (spectral harmony)
        assert_eq!(parse("m: spectrum(8,c2)")[0].tokens.join(" "), "c2 c3 g3 c4 e4 g4 bb4 c5");
        assert_eq!(parse("m: spectrum(6,e2)")[0].tokens.join(" "), "e2 e3 b3 e4 ab4 b4");
        assert_eq!(parse("m: spectrum(8)")[0].tokens.len(), 8); // default fund c2
    }

    #[test]
    fn shape_contours() {
        // arc: rises to a peak and resolves back on the tonic
        assert_eq!(parse("m: arc(9)")[0].tokens.join(" "), "1 3 4 6 7 6 4 3 1");
        // rise ascends, fall descends to the tonic
        assert_eq!(parse("m: rise(7)")[0].tokens.join(" "), "1 2 3 4 5 6 7");
        assert_eq!(parse("m: fall(7)")[0].tokens.join(" "), "7 6 5 4 3 2 1");
        assert!(expand_shape_token("arc(0)").is_none());
    }

    #[test]
    fn sieve_expands() {
        // even positions
        assert_eq!(parse("k: sieve(8,2:0)")[0].tokens.join(" "), "x . x . x . x .");
        // union of (i%3==0) and (i%4==1) over 12 steps
        assert_eq!(parse("k: sieve(12,3:0,4:1)")[0].tokens.join(" "), "x x . x . x x . . x . .");
        assert!(expand_sieve_token("sieve(0)").is_none());
    }

    #[test]
    fn world_rhythms_expand() {
        // named timelines expand to x/. patterns at parse time
        let v = parse("k: tresillo");
        assert_eq!(v[0].tokens.join(" "), "x . . x . . x .");
        let s = parse("k: son");
        assert_eq!(s[0].tokens.len(), 16);
        assert_eq!(world_rhythm("bembe").unwrap().split_whitespace().count(), 12);
        assert!(world_rhythm("pas_un_rythme").is_none());
    }

    #[test]
    fn rng_walk_drunk_match_js_engine_bit_for_bit() {
        // reference values produced by the actual JS engine (seed=5)
        assert_eq!(expand_walk_token("walk(8)", 2, 5).unwrap().join(" "), "1 3 1 2 1 1 1 2");
        assert_eq!(expand_drunk_token("drunk(16)", 0, 5).unwrap().join(""), "x...xxxxx.......");
        // pink/1f reference values (computed from the JS engine's exact RNG)
        assert_eq!(expand_1f_token("pink(8)", 0, 5).unwrap().join(" "), "4 4 4 4 5 4 5 6");
        assert_eq!(expand_1f_token("1f(8)", 1, 5).unwrap().join(" "), "3 5 5 5 5 5 5 6");
        // gendy reference values (Xenakis bouncing walk)
        assert_eq!(expand_gendy_token("gendy(8)", 0, 5).unwrap().join(" "), "4 4 5 4 5 4 4 5");
        assert_eq!(expand_gendy_token("gendy(8)", 1, 5).unwrap().join(" "), "4 3 4 4 3 4 4 3");
    }

    #[test]
    fn new_verbs_mirror_thin_accent() {
        let v = |s: &str| -> Vec<String> { s.split_whitespace().map(|x| x.to_string()).collect() };
        assert_eq!(apply_verb(&v("1 2 3"), "mirror", 0, 0).join(" "), "1 2 3 3 2 1");
        assert_eq!(apply_verb(&v("x x x x"), "thin 2", 0, 0).join(" "), "x . x .");
        assert_eq!(apply_verb(&v("x x x x"), "accent 2", 0, 0).join(" "), "x! x x! x");
    }

    #[test]
    fn events_carry_step_length() {
        let ev = schedule_bar(&parse("kick: x . [x x] ."), 0);
        let durs: Vec<f32> = ev.iter().map(|e| e.dur).collect();
        assert_eq!(durs, vec![0.25, 0.125, 0.125]);
        // arpeggio: a chord step is shared between its notes
        let ev = schedule_bar(&parse("keys: Cm . | arp up"), 0);
        assert_eq!(ev.len(), 3, "{:?}", ev);
        assert!(ev.iter().all(|e| (e.dur - 0.5 / 3.0).abs() < 1e-6), "{:?}", ev);
    }

    #[test]
    fn program_follows_chord_progression() {
        // `^` plays the current chord of `prog:` — Program must pass the progression through
        let p = Program::parse("prog: Am F\npad: ^ . . .");
        let bar0: Vec<String> = p.schedule_bar(0).into_iter().map(|e| e.token).collect();
        let bar1: Vec<String> = p.schedule_bar(1).into_iter().map(|e| e.token).collect();
        assert!(!bar0.is_empty(), "the chord must sound");
        assert_ne!(bar0, bar1, "bar 1 moves to F");
    }

    #[test]
    fn stutter_arp_modes_and_voice_synth() {
        let v = |s: &str| s.split_whitespace().map(String::from).collect::<Vec<_>>();
        assert_eq!(apply_verb(&v("1 2 3 4 5 6 7 8"), "stutter 4", 0, 0).join(" "), "1 2 1 2 1 2 1 2");
        let names = |ev: Vec<Event>| ev.into_iter().map(|e| e.token).collect::<Vec<_>>().join(" ");
        assert_eq!(names(schedule_bar(&parse("k: Cmaj7 | arp converge"), 0)), "c3 b3 e3 g3");
        assert_eq!(names(schedule_bar(&parse("k: C | arp up 2"), 0)), "c3 e3 g3 c4 e4 g4");
        assert_eq!(names(schedule_bar(&parse("k: C | arp thumb"), 0)), "c3 e3 c3 g3");
        let voices = parse("def fx: lp 900 | reverb 0.4\nbass: c2 . | saw | $fx | every 2 rev");
        let s = &voices[0].synth;
        assert_eq!((s.cutoff, s.reverb), (Some(Param::Const(900.0)), Some(0.4)));
        assert_eq!(voices[0].verbs, vec!["every 2 rev".to_string()]);
        assert_eq!(schedule_bar(&voices, 0)[0].synth.reverb, Some(0.4));
    }

    #[test]
    fn seed_ignores_trailing_comment() {
        assert_eq!(parse_seed("seed: 41 # graine du refrain"), 41);
        assert_eq!(parse_seed("seed:  24          # graine"), 24);
    }
}
