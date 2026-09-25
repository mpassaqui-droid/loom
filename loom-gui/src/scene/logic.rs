//! The SCÈNE's pure logic, without any drawing: which scene to show (`scene:`,
//! `scene@section:`, N, VJ), the sound analysis (FFT, bass/mid/treble levels),
//! voice colours and `| viz` styles.
//!
//! It only uses `std`, `loom-core` and `loom-audio`, so the terminal app
//! (`loom-tui`) includes this very file too: one tested copy for both apps.

use loom_audio::clock::name_salt;
use loom_core::Program;

// ------------------------------------------------------------------ scenes

/// A scene drawn natively. The browser has more (50 + older ones); every other
/// name is mapped to the closest one of these by [`Scene::from_name`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scene {
    Codemap,
    Radial,
    Tunnel,
    Bars,
    Starfield,
    Kaleido,
    Terrain,
    Spiral,
    Aurora,
    Metaballs,
    Network,
    Lissajous,
    Plasma,
    Fire,
    Matrix,
    Orbits,
    Wave,
    Lorenz,
    Warp,
    Spectro,
    Scope,
    Dna,
    Mandala,
    Fireworks,
}

impl Scene {
    /// Every native scene, in the order N cycles through them.
    pub const ALL: [Scene; 24] = [
        Scene::Codemap, Scene::Radial, Scene::Tunnel, Scene::Bars, Scene::Starfield, Scene::Kaleido,
        Scene::Terrain, Scene::Spiral, Scene::Aurora, Scene::Metaballs, Scene::Network, Scene::Lissajous,
        Scene::Plasma, Scene::Fire, Scene::Matrix, Scene::Orbits, Scene::Wave, Scene::Lorenz,
        Scene::Warp, Scene::Spectro, Scene::Scope, Scene::Dna, Scene::Mandala, Scene::Fireworks,
    ];

    /// The name written in the code (`scene: tunnel`).
    pub fn name(self) -> &'static str {
        match self {
            Scene::Codemap => "codemap",
            Scene::Radial => "radial",
            Scene::Tunnel => "tunnel",
            Scene::Bars => "bars",
            Scene::Starfield => "starfield",
            Scene::Kaleido => "kaleido",
            Scene::Terrain => "terrain",
            Scene::Spiral => "spiral",
            Scene::Aurora => "aurora",
            Scene::Metaballs => "metaballs",
            Scene::Network => "network",
            Scene::Lissajous => "lissajous",
            Scene::Plasma => "plasma",
            Scene::Fire => "fire",
            Scene::Matrix => "matrix",
            Scene::Orbits => "orbits",
            Scene::Wave => "wave",
            Scene::Lorenz => "lorenz",
            Scene::Warp => "warp",
            Scene::Spectro => "spectro",
            Scene::Scope => "scope",
            Scene::Dna => "dna",
            Scene::Mandala => "mandala",
            Scene::Fireworks => "fireworks",
        }
    }

    /// A scene from its name, native or not (`gtunnel` → tunnel, `julia` →
    /// plasma…). Case-insensitive; `None` for a name the browser doesn't know.
    pub fn from_name(name: &str) -> Option<Scene> {
        let name = name.trim().to_lowercase();
        if let Some(scene) = Scene::ALL.iter().find(|s| s.name() == name) {
            return Some(*scene);
        }
        ALIASES.iter().find(|(alias, _)| *alias == name).map(|(_, scene)| *scene)
    }

    /// The next scene for the N key (wraps around).
    pub fn next(self) -> Scene {
        let i = Scene::ALL.iter().position(|s| *s == self).unwrap_or(0);
        Scene::ALL[(i + 1) % Scene::ALL.len()]
    }
}

/// Browser scene names without a native version, and the closest native scene.
/// Grouped by look: pixel fields → plasma, strange attractors → lorenz, etc.
const ALIASES: &[(&str, Scene)] = &[
    // colour fields / fractals (and the WebGL shaders)
    ("julia", Scene::Plasma), ("julia4", Scene::Plasma), ("mandelbrot", Scene::Plasma), ("ship", Scene::Plasma),
    ("cymatic", Scene::Plasma), ("chladni", Scene::Plasma), ("reaction", Scene::Plasma), ("moire", Scene::Plasma),
    ("faraday", Scene::Plasma), ("voronoi", Scene::Plasma), ("truchet", Scene::Plasma), ("honeycomb", Scene::Plasma),
    ("tiles", Scene::Plasma), ("checker", Scene::Plasma), ("nebula", Scene::Plasma),
    ("shader", Scene::Plasma), ("gfractal", Scene::Plasma), ("gwarp", Scene::Plasma), ("gcells", Scene::Plasma),
    ("gwater", Scene::Plasma), ("gmandel", Scene::Plasma),
    ("gtunnel", Scene::Tunnel), ("gtorus", Scene::Orbits), ("gkaleid", Scene::Kaleido), ("gaurora", Scene::Aurora),
    // tunnels and rings
    ("wormhole", Scene::Tunnel), ("vortex", Scene::Tunnel), ("rings", Scene::Tunnel), ("ripple", Scene::Tunnel),
    // flowing things
    ("flow", Scene::Aurora), ("ribbon", Scene::Wave), ("string", Scene::Wave),
    // spectra
    ("waterfall", Scene::Spectro), ("spectrum3d", Scene::Spectro), ("tracker", Scene::Bars), ("grid", Scene::Bars),
    ("radialbars", Scene::Radial), ("sunburst", Scene::Radial), ("petals", Scene::Radial),
    ("scan", Scene::Scope), ("lissa3d", Scene::Lissajous),
    // strange attractors and pendulums
    ("harmono", Scene::Lorenz), ("clifford", Scene::Lorenz), ("dejong", Scene::Lorenz), ("hopalong", Scene::Lorenz),
    ("gumowski", Scene::Lorenz), ("tinker", Scene::Lorenz), ("ikeda", Scene::Lorenz), ("henon", Scene::Lorenz),
    ("rossler", Scene::Lorenz), ("thomas", Scene::Lorenz), ("chen", Scene::Lorenz), ("dadras", Scene::Lorenz),
    ("halvorsen", Scene::Lorenz), ("aizawa", Scene::Lorenz), ("pendulum", Scene::Lorenz),
    // sacred geometry, roses, spirographs
    ("flower", Scene::Mandala), ("metatron", Scene::Mandala), ("rose", Scene::Mandala), ("superf", Scene::Mandala),
    ("maurer", Scene::Mandala), ("guilloche", Scene::Mandala), ("spiro", Scene::Mandala), ("epicycle", Scene::Mandala),
    ("phyllo", Scene::Spiral), ("sierpinski", Scene::Kaleido), ("fern", Scene::Kaleido),
    // 3D objects, planets
    ("torus", Scene::Orbits), ("globe", Scene::Orbits), ("starsphere", Scene::Orbits), ("cube", Scene::Orbits),
    ("tesseract", Scene::Orbits), ("knot", Scene::Lissajous), ("mobius", Scene::Lissajous), ("lattice", Scene::Network),
    ("helix", Scene::Dna), ("lightning", Scene::Network),
    // particles
    ("fountain", Scene::Fireworks), ("bubbles", Scene::Fireworks), ("comet", Scene::Starfield),
];

/// The browser's curated list (cycled with N there), to test that each one
/// shows something here.
#[cfg(test)]
const BROWSER_SCENES: [&str; 50] = [
    "codemap", "radial", "tunnel", "bars", "starfield", "kaleido", "terrain", "spiral", "aurora", "metaballs",
    "flow", "julia", "plasma", "mandelbrot", "network", "lissajous", "waterfall", "torus", "fire", "matrix",
    "orbits", "wave", "cymatic", "chladni", "lorenz", "reaction", "truchet", "warp", "phyllo", "voronoi",
    "sierpinski", "lightning", "tracker", "string", "spectro", "scope", "dna", "mandala", "fireworks", "fern",
    "shader", "gtunnel", "gfractal", "gwarp", "gtorus", "gkaleid", "gaurora", "gcells", "gwater", "gmandel",
];

// ------------------------------------------------------ directives in code

/// `key: value` on a line: the lower-cased key and the raw value.
fn directive(line: &str) -> Option<(String, &str)> {
    let line = line.trim();
    let colon = line.find(':')?;
    Some((line[..colon].trim().to_lowercase(), line[colon + 1..].trim()))
}

/// First word of a value, without a trailing `# comment`.
fn first_word(value: &str) -> &str {
    value.split('#').next().unwrap_or("").split_whitespace().next().unwrap_or("")
}

/// The start scene from `scene: NAME` (the first valid one wins).
pub fn parse_scene(src: &str) -> Option<Scene> {
    src.lines()
        .filter_map(directive)
        .filter(|(key, _)| key == "scene")
        .find_map(|(_, value)| Scene::from_name(first_word(value)))
}

/// `scene@section: NAME` lines, as `(section, scene)` (section lower-cased).
pub fn parse_scene_map(src: &str) -> Vec<(String, Scene)> {
    let mut map: Vec<(String, Scene)> = Vec::new();
    for (key, value) in src.lines().filter_map(directive) {
        let Some(section) = key.strip_prefix("scene@") else { continue };
        let (section, Some(scene)) = (section.trim().to_string(), Scene::from_name(first_word(value))) else { continue };
        // a later line for the same section wins, like the browser's object
        map.retain(|(s, _)| *s != section);
        map.push((section, scene));
    }
    map
}

/// The song title from `title:` (at most 40 characters, like the browser).
pub fn parse_title(src: &str) -> Option<String> {
    src.lines()
        .filter_map(directive)
        .find(|(key, _)| key == "title")
        .map(|(_, value)| value.chars().take(40).collect::<String>())
        .filter(|t| !t.is_empty())
}

/// Chooses the scene shown: `scene:` at RUN, `scene@section:` when the song
/// enters that section, N by hand, and VJ mode (a new scene every 4 bars).
#[derive(Debug, Clone)]
pub struct Director {
    pub scene: Scene,
    pub vj: bool,
    pub title: Option<String>,
    /// The `scene:` of the last loaded code, so a re-RUN keeps a scene picked with N.
    code_scene: Option<Scene>,
    map: Vec<(String, Scene)>,
    last_bar: i64,
    last_section: Option<String>,
}

impl Default for Director {
    fn default() -> Self {
        Director { scene: Scene::Radial, vj: false, title: None, code_scene: None, map: Vec::new(), last_bar: -1, last_section: None }
    }
}

impl Director {
    /// Read the scene directives of code that was just RUN (or opened).
    /// `scene:` applies only when it changed, so N choices survive live edits.
    pub fn load(&mut self, src: &str) {
        let scene = parse_scene(src);
        if scene != self.code_scene {
            self.code_scene = scene;
            if let Some(scene) = scene {
                self.scene = scene;
            }
        }
        self.map = parse_scene_map(src);
        self.title = parse_title(src);
        self.last_section = None; // re-apply the section's scene
    }

    /// Follow the transport. `bar` counts bars since RUN, `section` is the
    /// arrangement's current section. Returns true if the scene changed.
    pub fn follow(&mut self, playing: bool, bar: i64, section: Option<&str>) -> bool {
        let before = self.scene;
        if !playing {
            self.last_bar = -1;
            self.last_section = None;
            return false;
        }
        if bar != self.last_bar {
            self.last_bar = bar;
            if self.vj && bar > 0 && bar % 4 == 0 {
                self.scene = vj_pick(bar, self.scene);
            }
        }
        let section = section.map(str::to_lowercase);
        if section != self.last_section {
            if let Some(scene) = section.as_ref().and_then(|s| self.map.iter().find(|(name, _)| name == s)).map(|(_, sc)| *sc) {
                self.scene = scene;
            }
            self.last_section = section;
        }
        self.scene != before
    }

    /// N: the next scene.
    pub fn cycle(&mut self) -> Scene {
        self.scene = self.scene.next();
        self.scene
    }
}

/// VJ mode's "random" scene for a bar: always a different one than `current`.
pub fn vj_pick(bar: i64, current: Scene) -> Scene {
    let n = Scene::ALL.len() as u64;
    let hash = (bar as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 33;
    let mut i = (hash % n) as usize;
    if Scene::ALL[i] == current {
        i = (i + 1 + (hash as usize / 7) % (Scene::ALL.len() - 1)) % Scene::ALL.len();
    }
    Scene::ALL[i]
}

// ----------------------------------------------------------------- colours

/// HSL (hue 0..360, saturation and lightness 0..1) to RGB bytes.
pub fn hsl_to_rgb(hue: f32, saturation: f32, lightness: f32) -> (u8, u8, u8) {
    let h = hue.rem_euclid(360.0) / 60.0;
    let (s, l) = (saturation.clamp(0.0, 1.0), lightness.clamp(0.0, 1.0));
    let chroma = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = chroma * (1.0 - (h % 2.0 - 1.0).abs());
    let (r, g, b) = match h as u32 {
        0 => (chroma, x, 0.0),
        1 => (x, chroma, 0.0),
        2 => (0.0, chroma, x),
        3 => (0.0, x, chroma),
        4 => (x, 0.0, chroma),
        _ => (chroma, 0.0, x),
    };
    let m = l - chroma / 2.0;
    let byte = |v: f32| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    (byte(r), byte(g), byte(b))
}

/// RGB bytes to HSL (hue 0..360, saturation and lightness 0..1).
pub fn rgb_to_hsl(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let (r, g, b) = (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let l = (max + min) / 2.0;
    let d = max - min;
    if d < 1e-6 {
        return (0.0, 0.0, l);
    }
    let s = d / (1.0 - (2.0 * l - 1.0).abs());
    let h = if max == r {
        ((g - b) / d).rem_euclid(6.0)
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    (h * 60.0, s.min(1.0), l)
}

/// HSV (hue 0..360, saturation and value 0..1) to RGB bytes, for the pixel
/// scenes that the browser colours with `hsv2rgb`.
pub fn hsv_to_rgb(hue: f32, saturation: f32, value: f32) -> (u8, u8, u8) {
    let (s, v) = (saturation.clamp(0.0, 1.0), value.clamp(0.0, 1.0));
    let l = v * (1.0 - s / 2.0);
    let sl = if l <= 0.0 || l >= 1.0 { 0.0 } else { (v - l) / l.min(1.0 - l) };
    hsl_to_rgb(hue, sl, l)
}

/// A theme colour made bright enough for the stage: same hue, more saturation
/// and light (greys stay grey, just lighter).
pub fn vivid(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
    let (h, s, l) = rgb_to_hsl(r, g, b);
    if s < 0.08 {
        return hsl_to_rgb(h, s, l.max(0.8));
    }
    hsl_to_rgb(h, s.max(0.75), l.clamp(0.55, 0.72))
}

/// How a voice's notes look in the scene (`| viz STYLE`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Viz {
    /// Particles shot in the voice's own direction (the default).
    Burst,
    /// A ring growing from the centre.
    Ring,
    /// A ray of light from the centre.
    Beam,
    /// The whole stage flashes in the voice's colour.
    Flash,
    /// A growing polygon.
    Shape,
}

impl Viz {
    pub fn from_name(name: Option<&str>) -> Viz {
        match name.map(|n| n.trim().to_lowercase()).as_deref() {
            Some("ring") => Viz::Ring,
            Some("beam") => Viz::Beam,
            Some("flash") => Viz::Flash,
            Some("shape") => Viz::Shape,
            _ => Viz::Burst,
        }
    }
}

/// What the scene needs to know about one voice of the code.
#[derive(Clone, Debug, PartialEq)]
pub struct VoiceLook {
    pub name: String,
    /// `name_salt(name)`, the id carried by the engine's hits.
    pub id: u32,
    pub hue: f32,
    pub drum: bool,
    pub gain: f32,
    pub viz: Viz,
}

/// A voice's colour: its `| hue`, else one derived from its name (like the browser).
pub fn voice_hue(name: &str, hue: Option<f32>) -> f32 {
    hue.unwrap_or((name_salt(name) % 360) as f32)
}

/// One entry per voice name (base voices, not muted), in the code's order.
pub fn voice_looks(program: &Program) -> Vec<VoiceLook> {
    let mut looks: Vec<VoiceLook> = Vec::new();
    for v in program.voices.iter().filter(|v| v.section.is_none() && !v.synth.mute) {
        if looks.iter().any(|l| l.name == v.name) {
            continue;
        }
        looks.push(VoiceLook {
            name: v.name.clone(),
            id: name_salt(&v.name),
            hue: voice_hue(&v.name, v.synth.hue),
            drum: v.is_drum,
            gain: v.gain,
            viz: Viz::from_name(v.synth.viz.as_deref()),
        });
    }
    looks
}

// ---------------------------------------------------------------- analysis

/// Samples analysed per frame (the browser's `fftSize`).
pub const FFT_SIZE: usize = loom_audio::SCOPE_LEN;

/// In-place fast Fourier transform (iterative radix, length a power of two).
pub fn fft(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    assert!(n.is_power_of_two() && im.len() == n, "fft: power-of-two buffers of equal length");
    // bit-reversal permutation
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    // combine halves of growing size
    let mut len = 2;
    while len <= n {
        let angle = -2.0 * std::f64::consts::PI / len as f64;
        let (w_re, w_im) = (angle.cos() as f32, angle.sin() as f32);
        for start in (0..n).step_by(len) {
            let (mut u_re, mut u_im) = (1.0f32, 0.0f32);
            for k in 0..len / 2 {
                let (a, b) = (start + k, start + k + len / 2);
                let t_re = re[b] * u_re - im[b] * u_im;
                let t_im = re[b] * u_im + im[b] * u_re;
                re[b] = re[a] - t_re;
                im[b] = im[a] - t_im;
                re[a] += t_re;
                im[a] += t_im;
                let next = u_re * w_re - u_im * w_im;
                u_im = u_re * w_im + u_im * w_re;
                u_re = next;
            }
        }
        len <<= 1;
    }
}

/// How loud the low, middle and high frequencies are, 0..1 (the browser's formulas).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Levels {
    pub bass: f32,
    pub mid: f32,
    pub treble: f32,
    /// Waveform peak (the browser's `lvl0`).
    pub level: f32,
}

/// Levels from a spectrum (0..1 per bin, like `getByteFrequencyData`/255) and a waveform.
pub fn band_levels(spectrum: &[f32], wave: &[f32]) -> Levels {
    let mean = |from: usize, to: usize| {
        let to = to.min(spectrum.len());
        if from >= to { 0.0 } else { spectrum[from..to].iter().sum::<f32>() / (to - from) as f32 }
    };
    let peak = wave.iter().step_by(12).fold(0.0f32, |m, x| m.max(x.abs()));
    Levels {
        bass: (mean(0, 10) * 1.5).min(1.0),
        mid: (mean(10, 48) * 1.6).min(1.0),
        treble: (mean(48, 128) * 2.0).min(1.0),
        level: (peak * 128.0 / 110.0).min(1.0),
    }
}

/// Turns the engine's last output samples into a smoothed spectrum, like the
/// browser's AnalyserNode (Hann window, smoothing 0.7, −100..−30 dB → 0..1).
pub struct Analyser {
    /// The waveform, oldest first, -1..1. Fill it (e.g. `status.scope(&mut a.wave)`), then [`Analyser::process`].
    pub wave: Vec<f32>,
    /// Loudness per frequency bin, 0..1 (`FFT_SIZE / 2` bins).
    pub spectrum: Vec<f32>,
    pub levels: Levels,
    window: Vec<f32>,
    re: Vec<f32>,
    im: Vec<f32>,
    smoothed: Vec<f32>,
}

impl Default for Analyser {
    fn default() -> Self {
        let window = (0..FFT_SIZE)
            .map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / FFT_SIZE as f32).cos())
            .collect();
        Analyser {
            wave: vec![0.0; FFT_SIZE],
            spectrum: vec![0.0; FFT_SIZE / 2],
            levels: Levels::default(),
            window,
            re: vec![0.0; FFT_SIZE],
            im: vec![0.0; FFT_SIZE],
            smoothed: vec![0.0; FFT_SIZE / 2],
        }
    }
}

impl Analyser {
    const SMOOTHING: f32 = 0.7;
    const MIN_DB: f32 = -100.0;
    const MAX_DB: f32 = -30.0;

    /// Analyse `wave`: updates `spectrum` and `levels`. No allocation.
    pub fn process(&mut self) {
        for i in 0..FFT_SIZE {
            self.re[i] = self.wave[i] * self.window[i];
            self.im[i] = 0.0;
        }
        fft(&mut self.re, &mut self.im);
        for k in 0..FFT_SIZE / 2 {
            let magnitude = (self.re[k] * self.re[k] + self.im[k] * self.im[k]).sqrt() / FFT_SIZE as f32;
            let s = Self::SMOOTHING * self.smoothed[k] + (1.0 - Self::SMOOTHING) * magnitude;
            self.smoothed[k] = if s.is_finite() { s } else { 0.0 };
            let db = 20.0 * self.smoothed[k].max(1e-12).log10();
            self.spectrum[k] = ((db - Self::MIN_DB) / (Self::MAX_DB - Self::MIN_DB)).clamp(0.0, 1.0);
        }
        self.levels = band_levels(&self.spectrum, &self.wave);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fft_finds_the_frequency_of_a_sine() {
        let bin = 37;
        let mut re: Vec<f32> = (0..1024).map(|i| (2.0 * std::f32::consts::PI * bin as f32 * i as f32 / 1024.0).sin()).collect();
        let mut im = vec![0.0; 1024];
        fft(&mut re, &mut im);
        let mags: Vec<f32> = (0..512).map(|k| (re[k] * re[k] + im[k] * im[k]).sqrt()).collect();
        let peak = (0..512).max_by(|a, b| mags[*a].total_cmp(&mags[*b])).unwrap();
        assert_eq!(peak, bin);
        assert!((mags[bin] - 512.0).abs() < 0.5, "a unit sine gives N/2, got {}", mags[bin]);
    }

    #[test]
    fn analyser_hears_bass_and_treble() {
        let sine = |freq: f32, amp: f32| -> Vec<f32> {
            (0..FFT_SIZE).map(|i| amp * (2.0 * std::f32::consts::PI * freq * i as f32 / 48_000.0).sin()).collect()
        };
        let mut a = Analyser { wave: sine(70.0, 0.5), ..Analyser::default() };
        for _ in 0..10 { a.process(); }
        let low = a.levels;
        assert!(low.bass > 0.3 && low.treble < 0.05, "70 Hz is bass: {low:?}");

        // quiet enough not to saturate: the loudest bin is the sine's
        let mut a = Analyser { wave: sine(1_000.0, 0.01), ..Analyser::default() };
        for _ in 0..10 { a.process(); }
        let loudest = (0..a.spectrum.len()).max_by(|x, y| a.spectrum[*x].total_cmp(&a.spectrum[*y])).unwrap();
        assert_eq!(loudest, (1_000.0 / (48_000.0 / FFT_SIZE as f32)).round() as usize);
        assert!(a.spectrum[loudest] < 1.0);

        let mut a = Analyser { wave: sine(3_000.0, 0.5), ..Analyser::default() };
        for _ in 0..10 { a.process(); }
        assert!(a.levels.treble > 0.02 && a.levels.bass < 0.05, "3 kHz is treble: {:?}", a.levels);
        assert!((a.levels.level - 0.5 * 128.0 / 110.0).abs() < 0.02);

        let mut silent = Analyser::default();
        silent.process();
        assert_eq!(silent.levels, Levels::default());
    }

    #[test]
    fn band_levels_follow_the_browser_formulas() {
        let mut spectrum = vec![0.0; 1024];
        spectrum[..10].fill(0.5);
        spectrum[10..48].fill(0.25);
        spectrum[48..128].fill(1.0);
        let l = band_levels(&spectrum, &[0.0, 0.2]);
        assert_eq!((l.bass, l.mid, l.treble), (0.75, 0.4, 1.0));
        assert_eq!(l.level, 0.0, "only every 12th sample counts");
        assert_eq!(band_levels(&[], &[]), Levels::default());
    }

    #[test]
    fn hsl_colours() {
        assert_eq!(hsl_to_rgb(0.0, 1.0, 0.5), (255, 0, 0));
        assert_eq!(hsl_to_rgb(120.0, 1.0, 0.5), (0, 255, 0));
        assert_eq!(hsl_to_rgb(240.0, 1.0, 0.5), (0, 0, 255));
        assert_eq!(hsl_to_rgb(-120.0, 1.0, 0.5), (0, 0, 255), "hue wraps");
        assert_eq!(hsl_to_rgb(60.0, 0.0, 0.5), (128, 128, 128));
        assert_eq!(hsl_to_rgb(200.0, 0.9, 1.0), (255, 255, 255));
    }

    #[test]
    fn colour_conversions_round_trip() {
        for (r, g, b) in [(255, 0, 0), (12, 200, 90), (57, 255, 20), (0, 229, 255), (40, 40, 40)] {
            let (h, s, l) = rgb_to_hsl(r, g, b);
            let (r2, g2, b2) = hsl_to_rgb(h, s, l);
            assert!((r as i32 - r2 as i32).abs() <= 1 && (g as i32 - g2 as i32).abs() <= 1 && (b as i32 - b2 as i32).abs() <= 1,
                "{:?} -> {:?}", (r, g, b), (r2, g2, b2));
        }
        assert_eq!(hsv_to_rgb(0.0, 1.0, 1.0), (255, 0, 0));
        assert_eq!(hsv_to_rgb(180.0, 0.0, 0.5), (128, 128, 128));
        assert_eq!(hsv_to_rgb(90.0, 1.0, 0.0), (0, 0, 0));
        let (_, s, l) = { let (r, g, b) = vivid(60, 90, 60); rgb_to_hsl(r, g, b) };
        assert!(s >= 0.74 && (0.54..=0.73).contains(&l), "dull green made vivid: s {s} l {l}");
    }

    #[test]
    fn every_browser_scene_shows_something() {
        for name in BROWSER_SCENES {
            assert!(Scene::from_name(name).is_some(), "no scene for {name}");
        }
        for scene in Scene::ALL {
            assert_eq!(Scene::from_name(scene.name()), Some(scene));
            assert!(ALIASES.iter().all(|(alias, _)| *alias != scene.name()), "{} is native and aliased", scene.name());
        }
        assert_eq!(Scene::from_name("GTunnel"), Some(Scene::Tunnel));
        assert_eq!(Scene::from_name("julia"), Some(Scene::Plasma));
        assert_eq!(Scene::from_name("n'importe"), None);
    }

    #[test]
    fn n_cycles_through_every_scene() {
        let mut scene = Scene::Codemap;
        let mut seen = Vec::new();
        for _ in 0..Scene::ALL.len() {
            seen.push(scene);
            scene = scene.next();
        }
        assert_eq!(scene, Scene::Codemap, "wraps around");
        assert!(Scene::ALL.iter().all(|s| seen.contains(s)));
    }

    #[test]
    fn scene_directives() {
        let src = "title: Nuit blanche\nbpm: 120\n# scene: bars\nscene:  tunnel   # hypnotique\nscene: bars\n\
                   scene@Drop: gcells\nscene@intro: nimportequoi\nscene@drop: warp\nkick: x . x .";
        assert_eq!(parse_scene(src), Some(Scene::Tunnel));
        assert_eq!(parse_scene_map(src), vec![("drop".to_string(), Scene::Warp)]);
        assert_eq!(parse_title(src).as_deref(), Some("Nuit blanche"));
        assert_eq!(parse_scene("scene: inconnu\nscene: fire"), Some(Scene::Fire));
        assert_eq!(parse_scene("kick: x"), None);
        assert_eq!(parse_title("title:"), None);
        assert_eq!(parse_title(&format!("title: {}", "a".repeat(60))).map(|t| t.len()), Some(40));
    }

    #[test]
    fn director_follows_code_sections_and_vj() {
        let mut d = Director::default();
        d.load("scene: tunnel\nscene@drop: fire\nsong: intro 2 drop 2");
        assert_eq!(d.scene, Scene::Tunnel);

        // N, then a live re-RUN with the same scene: keeps the N choice
        d.cycle();
        assert_eq!(d.scene, Scene::Tunnel.next());
        d.load("scene: tunnel\nscene@drop: fire\nsong: intro 2 drop 2\nkick: x");
        assert_eq!(d.scene, Scene::Tunnel.next());
        d.load("scene: bars\nscene@drop: fire");
        assert_eq!(d.scene, Scene::Bars, "a changed scene: applies");

        assert!(!d.follow(true, 0, Some("intro")));
        assert!(d.follow(true, 2, Some("drop")));
        assert_eq!(d.scene, Scene::Fire);
        d.cycle();
        assert!(!d.follow(true, 3, Some("drop")), "N wins until the section changes");
        d.follow(false, 0, None);
        assert!(d.follow(true, 2, Some("DROP")), "replaying re-enters the section");

        d.vj = true;
        let before = d.scene;
        assert!(!d.follow(true, 5, Some("drop")));
        assert!(d.follow(true, 8, Some("drop")));
        assert_ne!(d.scene, before);
    }

    #[test]
    fn vj_always_changes_scene() {
        for bar in (4..400).step_by(4) {
            for current in Scene::ALL {
                assert_ne!(vj_pick(bar, current), current);
            }
        }
    }

    #[test]
    fn voice_looks_come_from_the_code() {
        let program = Program::parse("kick: x . x . | viz flash | hue 10\nlead: c4 e4 | viz ring\nlead@drop: c5\nhat: x x | mute");
        let looks = voice_looks(&program);
        let names: Vec<&str> = looks.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(names, ["kick", "lead"]);
        assert_eq!((looks[0].hue, looks[0].viz, looks[0].drum), (10.0, Viz::Flash, true));
        assert_eq!(looks[1].viz, Viz::Ring);
        assert_eq!(looks[1].id, name_salt("lead"));
        assert_eq!(looks[1].hue, (name_salt("lead") % 360) as f32);
        assert_eq!(Viz::from_name(Some("zigzag")), Viz::Burst);
    }
}
