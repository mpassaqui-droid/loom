//! LOOM core — the shared language engine.
//!
//! This is a faithful Rust port of the JavaScript prototype's logic. The same
//! crate is wrapped by `loom-wasm` (browser / phone) and `loom-tui` (terminal),
//! so the language behaves identically everywhere. No external dependencies.

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
    /// Polymeter (`| poly N`): this voice runs its OWN loop of N steps per bar and
    /// drifts against the bar grid when its token count != N (Reich-style phasing).
    pub poly: Option<u32>,
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wave { Sine, Square, Saw, Triangle }

/// Arpeggio direction for a chord voice (`| arp up|down|updown|downup`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArpMode { Up, Down, UpDown, DownUp }

fn arp_mode(w: &str) -> ArpMode {
    match w {
        "down" => ArpMode::Down,
        "updown" | "ud" => ArpMode::UpDown,
        "downup" | "du" => ArpMode::DownUp,
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
}

/// Names that introduce a *directive* (key/scale/seed/song/duck/feel/…), not a voice.
const DIRECTIVES: [&str; 19] = [
    "key", "scale", "seed", "song", "arrange", "duck", "feel", "scene", "prog", "tune",
    "bpm", "tempo", "meter", "accel", "groove", "title", "warmth", "film", "loop",
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
    let is_p = |k: usize| -> bool { if k < 2 { return false; } let mut i = 2; while i * i <= k { if k % i == 0 { return false; } i += 1; } true };
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

/// `lsys(n)` — a Lindenmayer system -> self-similar melodic sequence (matches JS).
fn expand_lsys_token(tok: &str) -> Option<Vec<String>> {
    let n: usize = tok.strip_prefix("lsys(")?.strip_suffix(')')?.trim().parse().ok()?;
    let rule = |c: &str| -> Option<Vec<&'static str>> { match c {
        "1" => Some(vec!["1","2"]), "2" => Some(vec!["3","1"]), "3" => Some(vec!["5","2"]),
        "5" => Some(vec!["6","3"]), "6" => Some(vec!["1'","5"]), "1'" => Some(vec!["3'","1"]), _ => None } };
    let mut s: Vec<String> = vec!["1".into()];
    for _ in 0..8 {
        if s.len() >= n { break; }
        let mut nx: Vec<String> = Vec::new();
        for c in &s { match rule(c) { Some(r) => nx.extend(r.iter().map(|x| x.to_string())), None => nx.push(c.clone()) } }
        if nx.len() == s.len() { break; }
        s = nx;
    }
    s.truncate(n); Some(s)
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
        else { out.push(t); }
    }
    out
}

/// Parse LOOM source text into voices.
pub fn parse(src: &str) -> Vec<Voice> {
    let mut out = Vec::new();
    for raw in src.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        let Some(colon) = line.find(':') else { continue };
        let full = line[..colon].trim().to_lowercase();
        if DIRECTIVES.contains(&full.as_str()) { continue; }
        // `name@section` = a per-section variant of the voice `name`
        let (name, section) = match full.split_once('@') {
            Some((b, s)) => (b.to_string(), Some(s.to_string())),
            None => (full, None),
        };
        // skip directive@section lines too (e.g. key@chorus, scene@drop) — not voices
        if DIRECTIVES.contains(&name.as_str()) { continue; }
        let rest = &line[colon + 1..];
        let mut segs = rest.split('|').map(|s| s.trim());

        let pattern = segs.next().unwrap_or("");
        // stacks: `;` splits the pattern into simultaneous lines
        let stacks: Vec<Vec<String>> = pattern.split(';')
            .map(|p| expand_euclid(tokenize_pat(p)))
            .filter(|t| !t.is_empty())
            .collect();
        if stacks.is_empty() { continue; }
        let tokens = stacks[0].clone();

        let is_drum = DRUMS.contains(&name.as_str());
        let mut wave = if is_drum { Wave::Square } else { Wave::Square };
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
                poly = Some(parts.next().and_then(|x| x.parse::<u32>().ok()).unwrap_or(16).max(1));
            } else if head == "pan" {
                pan = parts.next().and_then(|x| x.parse::<f32>().ok()).map(|p| p.clamp(-1.0, 1.0));
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
            } else if head == "in" {
                let secs: Vec<String> = parts.map(|s| s.to_lowercase()).collect();
                if !secs.is_empty() { sections = Some(secs); }
            } else {
                verbs.push(seg.to_string());
            }
        }
        out.push(Voice { name, tokens, stacks, verbs, wave, gain, is_drum, octave, arp, sections, section, swing, nudge, poly, pan, jux, off, degrade, chance });
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
        Some("shuffle") | Some("shuf") => {
            let mut a = tokens.to_vec();
            let mut i = a.len();
            while i > 1 { i -= 1; let j = (js_rng(seed as u32, bar, &format!("shuf:{}", verb), i as i64) * ((i + 1) as f64)).floor() as usize; a.swap(i, j); }
            a
        }
        Some("markov") => {
            if tokens.len() < 2 { return tokens.to_vec(); }
            let mut trans: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
            for i in 0..tokens.len() { let a = tokens[i].clone(); let b = tokens[(i + 1) % tokens.len()].clone(); trans.entry(a).or_default().push(b); }
            let mut out = Vec::with_capacity(tokens.len());
            let mut cur = tokens[0].clone();
            for i in 0..tokens.len() {
                out.push(cur.clone());
                let opts = trans.get(&cur).cloned().unwrap_or_else(|| tokens.to_vec());
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
        // polymeter (`| poly N`): run an independent N-step loop indexed by the
        // GLOBAL step count (bar*N + i), so a token list of length != N drifts
        // against the bar grid each bar (Reich phasing). Mirrors the JS engine.
        if let Some(spb) = v.poly {
            let spb = spb.max(1) as usize;
            let nn = v.tokens.len();
            if nn > 0 {
                for i in 0..spb {
                    let s = bar * spb as i64 + i as i64;
                    let idx = s.rem_euclid(nn as i64) as usize;
                    let off = step_offset(i, spb, v, groove);
                    let p0 = i as f32 / spb as f32 + off;
                    let p1 = (i + 1) as f32 / spb as f32 + off;
                    emit_token(&mut events, &v.tokens[idx], p0, p1, v, voice_key.as_ref(),
                               base_oct, bar, seed, s.max(0) as usize, prog);
                }
            }
            continue;
        }
        // stacks: each `;`-separated line plays simultaneously (a single line = one stack)
        let lines: &[Vec<String>] = if v.stacks.len() > 1 { &v.stacks } else { std::slice::from_ref(&v.tokens) };
        for line in lines {
            let eff = effective_of(line, v, bar, seed);
            if eff.is_empty() { continue; }
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
                    emit_token(&mut events, raw, p0, p1, lv, voice_key.as_ref(), base_oct, bar, seed, i, prog);
                }
            }
        }
    }
    events
}

/// Split a pattern on whitespace but keep `[a b]` and `{a b}` groups (nested, mixed) as one token.
pub fn tokenize_pat(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut cur = String::new();
    for ch in s.chars() {
        if ch == '[' || ch == '{' {
            if depth == 0 && !cur.is_empty() { out.push(std::mem::take(&mut cur)); }
            depth += 1; cur.push(ch);
        } else if ch == ']' || ch == '}' {
            if depth > 0 { depth -= 1; }
            cur.push(ch);
            if depth == 0 { out.push(std::mem::take(&mut cur)); }
        } else if ch.is_whitespace() {
            if depth == 0 { if !cur.is_empty() { out.push(std::mem::take(&mut cur)); } }
            else { cur.push(ch); }
        } else { cur.push(ch); }
    }
    if !cur.is_empty() { out.push(cur); }
    out.into_iter().filter(|t| !t.starts_with('@') && !t.starts_with('>')).collect()
}

/// Recursively emit events for one token within the phase range `[p0, p1)`.
/// Handles `{a b}` random choice (seeded), `<a/b>` alternation, `[a b]` sub-sequences
/// (recursive subdivision), `?`/`*` mods, `!` accents and `*N>`/`*N<` rolls, and arps.
#[allow(clippy::too_many_arguments)]
fn emit_token(events: &mut Vec<Event>, tok: &str, p0: f32, p1: f32, v: &Voice,
              key: Option<&Key>, base_oct: i32, bar: i64, seed: u64, salt: usize, prog: &[String]) {
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
    if t == "." || t == "~" { return; }
    // [a b ...]: subdivide this range among the children (recursive)
    if t.len() >= 2 && t.starts_with('[') && t.ends_with(']') {
        let inner = tokenize_pat(&t[1..t.len() - 1]);
        let m = inner.len();
        if m == 0 { return; }
        let span = (p1 - p0) / m as f32;
        for (k, child) in inner.iter().enumerate() {
            emit_token(events, child, p0 + k as f32 * span, p0 + (k + 1) as f32 * span,
                       v, key, base_oct, bar, seed, salt.wrapping_mul(31).wrapping_add(k), prog);
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
            let ord = arp_order(&notes, mode);
            let m = ord.len();
            for (j, note) in ord.iter().enumerate() {
                events.push(Event { phase: p0 + (j as f32 / m as f32) * span, voice: v.name.clone(),
                    token: note.clone(), wave: v.wave, gain: v.gain, is_drum: v.is_drum, vel: accent, freq: freq_for(note), pan: v.pan });
            }
            return;
        }
    }
    for j in 0..ratchet {
        let phase = p0 + (j as f32 / ratchet as f32) * span;
        let vel = accent * ramp_factor(j, ratchet, ramp_dir);
        for note in &notes {
            events.push(Event { phase, voice: v.name.clone(), token: note.clone(),
                wave: v.wave, gain: v.gain, is_drum: v.is_drum, vel, freq: freq_for(note), pan: v.pan });
        }
    }
}

/// Convert a note name like "c2", "eb3", "g#4" to a frequency in Hz.
pub fn note_to_freq(note: &str) -> Option<f32> {
    let b = note.as_bytes();
    if b.is_empty() { return None; }
    let letter = (b[0] as char).to_ascii_lowercase();
    let base = match letter { 'c'=>0,'d'=>2,'e'=>4,'f'=>5,'g'=>7,'a'=>9,'b'=>11, _=>return None };
    let mut idx = 1;
    let mut semi = base as i32;
    if idx < b.len() && (b[idx] == b'#' || b[idx] == b'b') {
        if b[idx] == b'#' { semi += 1; } else { semi -= 1; }
        idx += 1;
    }
    let octave: i32 = note[idx..].parse().ok()?;
    let midi = 12 * (octave + 1) + semi;
    Some(midi_to_freq(midi))
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
// Chromatic alterations on degrees (#/b) are intentionally NOT supported yet —
// the scale itself supplies the right accidentals.
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
    let mut semi = base as i32;
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

/// SplitMix64 finalizer — cheap, well-distributed integer hash.
fn hash_u64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// FNV-1a hash of a string (used to fold a voice name into the RNG).
fn fnv(s: &str) -> u64 {
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    for b in s.bytes() { h ^= b as u64; h = h.wrapping_mul(0x0000_0100_0000_01B3); }
    h
}

/// Deterministic pseudo-random value in [0,1) from the performance coordinates.
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
        let line = raw.trim();
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
        let line = raw.trim();
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
            if let Ok(n) = line[colon + 1..].trim().split_whitespace().next().unwrap_or("").parse::<f64>() {
                return n.clamp(40.0, 300.0);
            }
        }
    }
    120.0
}

/// Beats per bar from `meter:`. `meter: 7/8` -> 7*4/8 = 3.5 quarter-beats;
/// a bare `meter: 3` -> 3 beats. Default 4.
pub fn parse_meter(src: &str) -> f64 {
    for raw in src.lines() {
        let line = raw.trim();
        let Some(colon) = line.find(':') else { continue };
        if line[..colon].trim().to_lowercase() != "meter" { continue; }
        let v = line[colon + 1..].trim();
        if let Some((n, d)) = v.split_once('/') {
            if let (Ok(n), Ok(d)) = (n.trim().parse::<f64>(), d.trim().parse::<f64>()) {
                if n > 0.0 && d > 0.0 { return n * 4.0 / d; }
            }
        } else if let Ok(n) = v.parse::<f64>() {
            if n > 0.0 { return n; }
        }
    }
    4.0
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
}

impl Program {
    pub fn parse(src: &str) -> Program {
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
        let active: Vec<Voice> = if self.song.is_empty() {
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
                        Some(set) => sec.map_or(false, |s| set.iter().any(|x| x == s)),
                    };
                    if plays { out.push(c.clone()); }
                }
            }
            out
        };
        // modulation: if this section has a key@section, resolve in that key
        let sec_now = section_at_bar(&self.song, bar);
        let cur_key = sec_now
            .and_then(|s| self.key_map.iter().find(|(n, _)| n == s).map(|(_, k)| k))
            .or(self.key.as_ref());
        sched_prog(&active, cur_key, &[], bar, self.seed, &self.groove)
    }
}

/// Parse a roman-numeral chord token: `I`..`VII` / `i`..`vii`, optional trailing
/// `7` for a diatonic seventh. Returns (degree 1..7, seventh?). Case is allowed
/// either way — the chord quality comes from the scale, not the casing.
fn parse_roman(tok: &str) -> Option<(i32, bool)> {
    let (body, seventh) = match tok.strip_suffix('7') { Some(b) => (b, true), None => (tok, false) };
    let deg = match body {
        "I" | "i" => 1, "II" | "ii" => 2, "III" | "iii" => 3, "IV" | "iv" => 4,
        "V" | "v" => 5, "VI" | "vi" => 6, "VII" | "vii" => 7,
        _ => return None,
    };
    Some((deg, seventh))
}

/// Parse a chord progression directive (`prog: Am*2 F C`) -> one chord per bar
/// (cycling), expanding `*N` durations. Keeps original case (chords are case-sensitive).
pub fn parse_prog(src: &str) -> Vec<String> {
    for raw in src.lines() {
        let line = raw.trim();
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
        .and_then(|k| parse_roman(cs).map(|(d, s)| diatonic_chord(k, d, s)))
        .or_else(|| parse_chord(cs, octave))
        .unwrap_or_default();
    if notes.is_empty() { return ".".to_string(); }
    let idx = nn - 1;
    let len = notes.len() as i32;
    let mut midi = notes[idx.rem_euclid(len) as usize] + 12 * idx.div_euclid(len);
    for c in marks.chars() { if c == '\'' { midi += 12; } else if c == ',' { midi -= 12; } }
    midi_to_name(midi)
}

/// MIDI note for a 0-based scale index, wrapping up an octave past the scale end.
fn scale_note_midi(key: &Key, idx: i32) -> i32 {
    let len = key.scale.len() as i32;
    let wrap = idx.div_euclid(len);
    let step = key.scale[idx.rem_euclid(len) as usize].round() as i32; // chords snap to 12-TET
    12 * (key.octave + wrap + 1) + key.tonic + step
}

/// Build a diatonic chord on a scale degree by stacking scale-thirds, so the
/// quality (maj/min/dim…) is whatever the scale gives — works in any scale.
fn diatonic_chord(key: &Key, degree: i32, seventh: bool) -> Vec<i32> {
    let i0 = degree - 1;
    let mut n = vec![scale_note_midi(key, i0), scale_note_midi(key, i0 + 2), scale_note_midi(key, i0 + 4)];
    if seventh { n.push(scale_note_midi(key, i0 + 6)); }
    n
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
        if let Some((deg, seventh)) = parse_roman(tok) {
            return diatonic_chord(k, deg, seventh).into_iter().map(midi_to_name).collect();
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
        assert_eq!(v[0].poly, Some(13));
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
    fn jux_splits_into_two_panned_layers() {
        // jux rev: original hard-left + reversed copy hard-right.
        let v = parse("m: 1 2 | jux rev");
        assert_eq!(v[0].jux.as_deref(), Some("rev"));
        let k = parse_key("key: c major");
        let ev = schedule_bar_seeded(&v, k.as_ref(), 0, 0);
        assert_eq!(ev.len(), 4);
        let left: Vec<&str> = ev.iter().filter(|e| e.pan.map_or(false, |p| (p + 0.82).abs() < 1e-6)).map(|e| e.token.as_str()).collect();
        let right: Vec<&str> = ev.iter().filter(|e| e.pan.map_or(false, |p| (p - 0.82).abs() < 1e-6)).map(|e| e.token.as_str()).collect();
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
    }

    #[test]
    fn new_verbs_mirror_thin_accent() {
        let v = |s: &str| -> Vec<String> { s.split_whitespace().map(|x| x.to_string()).collect() };
        assert_eq!(apply_verb(&v("1 2 3"), "mirror", 0, 0).join(" "), "1 2 3 3 2 1");
        assert_eq!(apply_verb(&v("x x x x"), "thin 2", 0, 0).join(" "), "x . x .");
        assert_eq!(apply_verb(&v("x x x x"), "accent 2", 0, 0).join(" "), "x! x x! x");
    }
}
