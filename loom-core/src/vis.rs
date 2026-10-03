//! Visuels en code : une ligne `vis:` décrit une image, une chaîne d'étapes
//! comme dans Hydra, et chaque nombre peut suivre le son.
//!
//! ```text
//! vis: osc 12 0.1 0.8 | kaleid 5 | rotate t*0.2 | scale 1+low | hue mid | bright high*0.3
//! vis: shape 6 0.2+low*0.4 0.05 | color 1 0.4 0.2 | gain level
//! ```
//!
//! - La première étape est une **source** : `osc FRÉQ VITESSE DÉCALAGE`,
//!   `noise ÉCHELLE VITESSE`, `shape CÔTÉS RAYON FLOU`, `rings FRÉQ VITESSE`,
//!   `grad VITESSE`, `solid R V B`.
//! - Les **coordonnées** se déforment : `rotate ANGLE VITESSE`, `scale K`,
//!   `kaleid N`, `scroll X Y`, `pixel N`, `repeat N`, `warp FORCE FRÉQ`.
//!   Comme dans Hydra, elles s'appliquent à l'endroit où la source est lue
//!   (la dernière écrite agit la première sur le point de l'écran).
//! - La **couleur** se règle ensuite, dans l'ordre : `hue TOURS`, `bright B`,
//!   `contrast C`, `gain G`, `color R V B`, `invert A`, `thresh SEUIL`, `posterize N`.
//! - Chaque nombre est une expression (sans espace, ou entre parenthèses) avec
//!   `t` (secondes), `low mid high` (bandes du son, 0..1), `level` (crête),
//!   `beat` (phase dans le temps, 0..1), `bar` (mesure), `pi`, `+ - * / % ^`
//!   et `sin cos abs floor fract sqrt min max`.
//! - Plusieurs lignes `vis:` s'additionnent (lumière sur lumière).
//!
//! Le même code dessine dans la fenêtre native et dans le navigateur (par WASM).

use std::f32::consts::{PI, TAU};

/// Ce que le son et l'horloge donnent à l'image à chaque image.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Inputs {
    pub t: f32,
    pub low: f32,
    pub mid: f32,
    pub high: f32,
    pub level: f32,
    pub beat: f32,
    pub bar: f32,
}

const VARS: [&str; 7] = ["t", "low", "mid", "high", "level", "beat", "bar"];

impl Inputs {
    fn vals(&self) -> [f32; 7] { [self.t, self.low, self.mid, self.high, self.level, self.beat, self.bar] }
}

// ---------------------------------------------------------------- expressions

#[derive(Debug, Clone, Copy, PartialEq)]
enum Op {
    Num(f32),
    Var(u8),
    Neg,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    F1(u8),
    Min,
    Max,
}

const FUNCS1: [&str; 6] = ["sin", "cos", "abs", "floor", "fract", "sqrt"];

/// Une expression compilée (notation polonaise inverse).
#[derive(Debug, Clone, PartialEq)]
pub struct Expr(Vec<Op>);

impl Expr {
    pub fn parse(src: &str) -> Result<Expr, String> {
        // bornes avant de lire : ni pile d'appels qui déborde, ni pile de calcul trop haute
        if src.len() > 512 { return Err("expression trop longue (512 caractères au plus)".into()); }
        let toks = lex(src)?;
        let mut p = Parser { toks, i: 0, out: Vec::new(), depth: 0 };
        p.sum()?;
        if p.i < p.toks.len() { return Err(format!("« {src} » : en trop après « {} »", p.toks[p.i - 1].text())); }
        if p.out.len() > 256 { return Err(format!("« {src} » : expression trop longue")); }
        let (mut n, mut most) = (0usize, 0usize);
        for op in &p.out {
            match op { Op::Num(_) | Op::Var(_) => n += 1, Op::Neg | Op::F1(_) => {}, _ => n -= 1 }
            most = most.max(n);
        }
        if most > 48 { return Err(format!("« {src} » : expression trop imbriquée")); }
        Ok(Expr(p.out))
    }

    pub fn eval(&self, vars: &[f32; 7]) -> f32 {
        let mut st = [0.0f32; 64];
        let mut n = 0usize;
        for op in &self.0 {
            match *op {
                Op::Num(v) => { st[n] = v; n += 1; }
                Op::Var(i) => { st[n] = vars[i as usize]; n += 1; }
                Op::Neg => st[n - 1] = -st[n - 1],
                Op::F1(f) => {
                    let x = st[n - 1];
                    st[n - 1] = match f { 0 => x.sin(), 1 => x.cos(), 2 => x.abs(), 3 => x.floor(), 4 => x - x.floor(), _ => x.max(0.0).sqrt() };
                }
                _ => {
                    let (a, b) = (st[n - 2], st[n - 1]);
                    n -= 1;
                    st[n - 1] = match *op {
                        Op::Add => a + b,
                        Op::Sub => a - b,
                        Op::Mul => a * b,
                        Op::Div => if b == 0.0 { 0.0 } else { a / b },
                        Op::Mod => if b == 0.0 { 0.0 } else { a - b * (a / b).floor() },
                        Op::Pow => a.abs().powf(b),
                        Op::Min => a.min(b),
                        _ => a.max(b),
                    };
                }
            }
            // la pile est bornée par la longueur (256) et la profondeur, jamais par le hasard
            if n >= st.len() { return 0.0; }
        }
        let v = if n > 0 { st[n - 1] } else { 0.0 };
        if v.is_finite() { v } else { 0.0 }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f32),
    Id(String),
    Sym(char),
}

impl Tok {
    fn text(&self) -> String {
        match self { Tok::Num(v) => v.to_string(), Tok::Id(s) => s.clone(), Tok::Sym(c) => c.to_string() }
    }
}

fn lex(src: &str) -> Result<Vec<Tok>, String> {
    let b: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c.is_whitespace() { i += 1; continue; }
        if c.is_ascii_digit() || (c == '.' && b.get(i + 1).is_some_and(|d| d.is_ascii_digit())) {
            let s = i;
            while i < b.len() && (b[i].is_ascii_digit() || b[i] == '.') { i += 1; }
            let t: String = b[s..i].iter().collect();
            out.push(Tok::Num(t.parse().map_err(|_| format!("nombre illisible « {t} »"))?));
        } else if c.is_ascii_alphabetic() {
            let s = i;
            while i < b.len() && b[i].is_ascii_alphanumeric() { i += 1; }
            out.push(Tok::Id(b[s..i].iter().collect::<String>().to_ascii_lowercase()));
        } else if "+-*/%^(),".contains(c) {
            out.push(Tok::Sym(c));
            i += 1;
        } else {
            return Err(format!("« {c} » inattendu dans « {src} »"));
        }
    }
    Ok(out)
}

struct Parser {
    toks: Vec<Tok>,
    i: usize,
    out: Vec<Op>,
    depth: u32,
}

/// Le plus de parenthèses, signes ou puissances emboîtés.
const MAX_DEPTH: u32 = 32;

impl Parser {
    fn peek(&self) -> Option<&Tok> { self.toks.get(self.i) }
    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(&Tok::Sym(c)) { self.i += 1; true } else { false }
    }
    fn sum(&mut self) -> Result<(), String> {
        self.product()?;
        loop {
            if self.eat('+') { self.product()?; self.out.push(Op::Add); }
            else if self.eat('-') { self.product()?; self.out.push(Op::Sub); }
            else { return Ok(()); }
        }
    }
    fn product(&mut self) -> Result<(), String> {
        self.unary()?;
        loop {
            if self.eat('*') { self.unary()?; self.out.push(Op::Mul); }
            else if self.eat('/') { self.unary()?; self.out.push(Op::Div); }
            else if self.eat('%') { self.unary()?; self.out.push(Op::Mod); }
            else { return Ok(()); }
        }
    }
    fn unary(&mut self) -> Result<(), String> {
        self.depth += 1;
        let r = self.unary_inner();
        self.depth -= 1;
        r
    }
    fn unary_inner(&mut self) -> Result<(), String> {
        if self.depth > MAX_DEPTH { return Err("expression trop imbriquée".into()); }
        if self.eat('-') { self.unary()?; self.out.push(Op::Neg); return Ok(()); }
        if self.eat('+') { return self.unary(); }
        self.atom()?;
        if self.eat('^') { self.unary()?; self.out.push(Op::Pow); }
        Ok(())
    }
    fn atom(&mut self) -> Result<(), String> {
        let Some(t) = self.toks.get(self.i).cloned() else { return Err("expression incomplète".into()) };
        self.i += 1;
        match t {
            Tok::Num(v) => self.out.push(Op::Num(v)),
            Tok::Sym('(') => {
                self.sum()?;
                if !self.eat(')') { return Err("« ( » sans « ) »".into()); }
            }
            Tok::Id(id) if id == "pi" => self.out.push(Op::Num(PI)),
            Tok::Id(id) => {
                if let Some(v) = VARS.iter().position(|v| *v == id) { self.out.push(Op::Var(v as u8)); return Ok(()); }
                let f1 = FUNCS1.iter().position(|f| *f == id);
                if f1.is_none() && id != "min" && id != "max" { return Err(format!("« {id} » inconnu (variables : t low mid high level beat bar)")); }
                if !self.eat('(') { return Err(format!("{id} demande des parenthèses : {id}(…)")); }
                self.sum()?;
                match f1 {
                    Some(f) => self.out.push(Op::F1(f as u8)),
                    None => {
                        if !self.eat(',') { return Err(format!("{id} demande deux nombres : {id}(a,b)")); }
                        self.sum()?;
                        self.out.push(if id == "min" { Op::Min } else { Op::Max });
                    }
                }
                if !self.eat(')') { return Err(format!("{id}( sans « ) »")); }
            }
            Tok::Sym(c) => return Err(format!("« {c} » inattendu")),
        }
        Ok(())
    }
}

// ---------------------------------------------------------------- étapes

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Src,
    Coord,
    Color,
}

const STAGES: [(&str, Kind, &[f32]); 21] = [
    ("osc", Kind::Src, &[10.0, 0.1, 0.0]),
    ("noise", Kind::Src, &[4.0, 0.1]),
    ("shape", Kind::Src, &[3.0, 0.4, 0.02]),
    ("rings", Kind::Src, &[10.0, 0.2]),
    ("grad", Kind::Src, &[0.1]),
    ("solid", Kind::Src, &[1.0, 1.0, 1.0]),
    ("rotate", Kind::Coord, &[0.0, 0.0]),
    ("scale", Kind::Coord, &[1.0]),
    ("kaleid", Kind::Coord, &[4.0]),
    ("scroll", Kind::Coord, &[0.0, 0.0]),
    ("pixel", Kind::Coord, &[20.0]),
    ("repeat", Kind::Coord, &[3.0]),
    ("warp", Kind::Coord, &[0.2, 3.0]),
    ("hue", Kind::Color, &[0.0]),
    ("bright", Kind::Color, &[0.0]),
    ("contrast", Kind::Color, &[1.0]),
    ("gain", Kind::Color, &[1.0]),
    ("color", Kind::Color, &[1.0, 1.0, 1.0]),
    ("invert", Kind::Color, &[1.0]),
    ("thresh", Kind::Color, &[0.5]),
    ("posterize", Kind::Color, &[4.0]),
];

/// Les noms d'étapes, pour l'aide et la coloration.
pub fn stage_names() -> impl Iterator<Item = &'static str> { STAGES.iter().map(|s| s.0) }

#[derive(Debug, Clone, PartialEq)]
struct Stage {
    id: u8,
    args: Vec<Expr>,
}

/// Une ligne `vis:` : une source, des déformations, des réglages de couleur.
#[derive(Debug, Clone, PartialEq)]
pub struct Layer {
    src: Stage,
    coords: Vec<Stage>,
    colors: Vec<Stage>,
}

/// Coupe aux espaces hors parenthèses : `rotate (low * 2) 0.1` → `rotate`, `(low * 2)`, `0.1`.
fn words(s: &str) -> Vec<String> {
    let (mut out, mut cur, mut depth) = (Vec::new(), String::new(), 0i32);
    for c in s.chars() {
        match c {
            '(' => { depth += 1; cur.push(c); }
            ')' => { depth -= 1; cur.push(c); }
            c if c.is_whitespace() && depth <= 0 => { if !cur.is_empty() { out.push(std::mem::take(&mut cur)); } }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() { out.push(cur); }
    out
}

impl Layer {
    pub fn parse(src: &str) -> Result<Layer, String> {
        let mut stages = Vec::new();
        for part in src.split('|') {
            let w = words(part);
            let Some(name) = w.first() else { continue };
            let name = name.to_ascii_lowercase();
            let Some(id) = STAGES.iter().position(|s| s.0 == name) else {
                return Err(format!("étape « {name} » inconnue (sources : osc noise shape rings grad solid)"));
            };
            let max = STAGES[id].2.len();
            if w.len() - 1 > max { return Err(format!("{name} prend au plus {max} nombre(s)")); }
            let args = w[1..].iter().map(|a| Expr::parse(a)).collect::<Result<Vec<_>, _>>()?;
            stages.push(Stage { id: id as u8, args });
        }
        let mut it = stages.into_iter();
        let src = it.next().ok_or("vis: vide")?;
        if STAGES[src.id as usize].1 != Kind::Src {
            return Err(format!("vis: commence par une source (osc, noise, shape, rings, grad, solid), pas « {} »", STAGES[src.id as usize].0));
        }
        let (mut coords, mut colors) = (Vec::new(), Vec::new());
        for s in it {
            match STAGES[s.id as usize].1 {
                Kind::Src => return Err(format!("une seule source par ligne vis: (« {} » en trop) ; ajoute une autre ligne vis:", STAGES[s.id as usize].0)),
                Kind::Coord => coords.push(s),
                Kind::Color => colors.push(s),
            }
        }
        // la dernière déformation écrite agit la première sur le point de l'écran
        coords.reverse();
        Ok(Layer { src, coords, colors })
    }
}

/// Toutes les lignes `vis:` d'un morceau.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Vis {
    pub layers: Vec<Layer>,
    /// Les lignes `vis:` illisibles, avec la raison (elles ne dessinent rien).
    pub errors: Vec<String>,
}

/// Le reste de la ligne après `vis:` (ou `None` si ce n'est pas une ligne vis:).
pub fn vis_body(line: &str) -> Option<&str> {
    let l = line.trim();
    let i = l.find(':')?;
    l[..i].trim().eq_ignore_ascii_case("vis").then(|| &l[i + 1..])
}

/// La ligne sans son commentaire (`#` ou `//`), comme la lit [`Vis::parse`].
fn strip(line: &str) -> &str {
    let line = line.split('#').next().unwrap_or("");
    line.split("//").next().unwrap_or("")
}

/// Pour le correcteur : `None` si ce n'est pas une ligne vis:, sinon son erreur éventuelle.
pub fn check_line(line: &str) -> Option<Result<(), String>> {
    vis_body(strip(line)).map(|b| Layer::parse(b).map(|_| ()))
}

impl Vis {
    pub fn parse(src: &str) -> Vis {
        let mut v = Vis::default();
        for line in src.lines() {
            let Some(body) = vis_body(strip(line)) else { continue };
            match Layer::parse(body) {
                Ok(l) => v.layers.push(l),
                Err(e) => v.errors.push(format!("vis: {e}")),
            }
        }
        v
    }

    pub fn is_empty(&self) -> bool { self.layers.is_empty() }

    /// Dessine l'image `w × h` en RGBA (alpha plein), ligne par ligne depuis le haut.
    pub fn render(&self, w: usize, h: usize, inp: &Inputs, out: &mut Vec<u8>) {
        out.clear();
        out.resize(w * h * 4, 0);
        if w == 0 || h == 0 { return; }
        let vars = inp.vals();
        let frames: Vec<LayerFrame> = self.layers.iter().map(|l| LayerFrame::new(l, &vars)).collect();
        let aspect = w as f32 / h as f32;
        for j in 0..h {
            let y = 1.0 - 2.0 * (j as f32 + 0.5) / h as f32;
            for i in 0..w {
                let x = (2.0 * (i as f32 + 0.5) / w as f32 - 1.0) * aspect;
                let mut c = [0.0f32; 3];
                for f in &frames {
                    let p = f.pixel(x, y, inp.t);
                    for k in 0..3 { c[k] += p[k]; }
                }
                let o = (j * w + i) * 4;
                for k in 0..3 { out[o + k] = (c[k].clamp(0.0, 1.0) * 255.0 + 0.5) as u8; }
                out[o + 3] = 255;
            }
        }
    }
}

/// Une couche dont les nombres sont calculés pour cette image : chaque étape
/// devient une opération prête (sinus, matrices… calculés une fois par image).
struct LayerFrame {
    src: Src,
    coords: Vec<Coord>,
    colors: Vec<Col>,
}

enum Src {
    /// fréquence, phase, (sin, cos) du décalage et du double décalage
    Osc(f32, f32, [f32; 4]),
    Noise(f32, f32),
    Shape { poly: bool, sector: f32, r: f32, e: f32 },
    Rings(f32, f32),
    Grad(f32),
    Solid([f32; 3]),
}

enum Coord {
    Rot(f32, f32),
    Scale(f32),
    Kaleid(f32),
    Scroll(f32, f32),
    Pixel(f32),
    Repeat(f32),
    Warp(f32, f32),
}

enum Col {
    Hue([[f32; 3]; 3]),
    Bright(f32),
    Contrast(f32),
    Gain(f32),
    Color([f32; 3]),
    Invert(f32),
    Thresh(f32),
    Poster(f32),
}

fn values(s: &Stage, vars: &[f32; 7]) -> (&'static str, Vec<f32>) {
    let def = STAGES[s.id as usize].2;
    (STAGES[s.id as usize].0, (0..def.len()).map(|k| s.args.get(k).map_or(def[k], |e| e.eval(vars))).collect())
}

fn hue_matrix(turns: f32) -> [[f32; 3]; 3] {
    let (s, c) = (turns * TAU).sin_cos();
    let k = 1.0 / 3.0;
    let q = (1.0 / 3.0f32).sqrt();
    let a = c + (1.0 - c) * k;
    let b = k * (1.0 - c) - q * s;
    let d = k * (1.0 - c) + q * s;
    [[a, b, d], [d, a, b], [b, d, a]]
}

fn hash3(x: i32, y: i32, z: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343) ^ (y as u32).wrapping_mul(0xd816_3841) ^ (z as u32).wrapping_mul(0xcb1a_b31f);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    (h & 0xffff) as f32 / 65535.0
}

fn vnoise(x: f32, y: f32, z: f32) -> f32 {
    let (fx, fy, fz) = (x.floor(), y.floor(), z.floor());
    let (ix, iy, iz) = (fx as i32, fy as i32, fz as i32);
    let s = |v: f32| v * v * (3.0 - 2.0 * v);
    let (u, v, w) = (s(x - fx), s(y - fy), s(z - fz));
    let l = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let plane = |dz: i32| {
        let a = l(hash3(ix, iy, iz + dz), hash3(ix + 1, iy, iz + dz), u);
        let b = l(hash3(ix, iy + 1, iz + dz), hash3(ix + 1, iy + 1, iz + dz), u);
        l(a, b, v)
    };
    l(plane(0), plane(1), w)
}

// Le calcul par point évite la libm (lente en WASM) : des approximations
// à 1e-3 près, invisibles à l'écran, et identiques en natif et sur le web.

fn len(x: f32, y: f32) -> f32 { (x * x + y * y).sqrt() }

fn sin(x: f32) -> f32 {
    if !x.is_finite() { return 0.0; }
    // au-delà de 1e5, la réduction rapide perd sa précision : fmod exact
    let x = if x.abs() < 1e5 { x - TAU * (x / TAU + 0.5).floor() } else {
        let m = x % TAU;
        if m > PI { m - TAU } else if m < -PI { m + TAU } else { m }
    };
    let y = 4.0 / PI * x - 4.0 / (PI * PI) * x * x.abs();
    0.225 * (y * y.abs() - y) + y
}

fn cos(x: f32) -> f32 { sin(x + PI * 0.5) }

fn atan2(y: f32, x: f32) -> f32 {
    let (ax, ay) = (x.abs(), y.abs());
    let (mx, mn) = (ax.max(ay), ax.min(ay));
    if mx == 0.0 || !mx.is_finite() { return 0.0; }
    let a = mn / mx;
    let s = a * a;
    let mut r = ((-0.046_496_475 * s + 0.159_314_22) * s - 0.327_622_76) * s * a + a;
    if ay > ax { r = PI * 0.5 - r; }
    if x < 0.0 { r = PI - r; }
    if y < 0.0 { -r } else { r }
}

impl LayerFrame {
    fn new(l: &Layer, vars: &[f32; 7]) -> LayerFrame {
        let t = vars[0];
        let (name, a) = values(&l.src, vars);
        let src = match name {
            "osc" => { let (s1, c1) = a[2].sin_cos(); let (s2, c2) = (2.0 * a[2]).sin_cos(); Src::Osc(a[0], t * a[1] * TAU, [s1, c1, s2, c2]) }
            "noise" => Src::Noise(a[0], t * a[1]),
            "shape" => Src::Shape { poly: a[0] >= 2.5, sector: TAU / a[0].clamp(3.0, 64.0).floor(), r: a[1], e: a[2].abs().max(1e-4) },
            "rings" => Src::Rings(a[0], t * a[1] * TAU),
            "grad" => Src::Grad(0.5 + 0.5 * (t * a[0] * TAU).sin()),
            _ => Src::Solid([a[0], a[1], a[2]]),
        };
        let coords = l.coords.iter().map(|s| {
            let (name, a) = values(s, vars);
            match name {
                "rotate" => { let (sn, c) = (-(a[0] + t * a[1])).sin_cos(); Coord::Rot(sn, c) }
                "scale" => Coord::Scale(1.0 / if a[0].abs() < 1e-4 { 1e-4 } else { a[0] }),
                "kaleid" => Coord::Kaleid(TAU / a[0].clamp(1.0, 64.0)),
                "scroll" => Coord::Scroll(a[0], a[1]),
                "pixel" => Coord::Pixel(a[0].clamp(1.0, 2000.0)),
                "repeat" => Coord::Repeat(a[0].clamp(0.01, 200.0) * 0.5),
                _ => Coord::Warp(a[0], a[1]),
            }
        }).collect();
        let colors = l.colors.iter().map(|s| {
            let (name, a) = values(s, vars);
            match name {
                "hue" => Col::Hue(hue_matrix(a[0])),
                "bright" => Col::Bright(a[0]),
                "contrast" => Col::Contrast(a[0]),
                "gain" => Col::Gain(a[0]),
                "color" => Col::Color([a[0], a[1], a[2]]),
                "invert" => Col::Invert(a[0]),
                "thresh" => Col::Thresh(a[0]),
                _ => Col::Poster(a[0].clamp(2.0, 256.0)),
            }
        }).collect();
        LayerFrame { src, coords, colors }
    }

    fn pixel(&self, mut x: f32, mut y: f32, t: f32) -> [f32; 3] {
        for op in &self.coords {
            match *op {
                Coord::Rot(s, c) => (x, y) = (x * c - y * s, x * s + y * c),
                Coord::Scale(k) => { x *= k; y *= k; }
                Coord::Kaleid(sector) => {
                    let r = len(x, y);
                    let ang = (atan2(y, x).rem_euclid(sector) - sector * 0.5).abs();
                    let (s, c) = (sin(ang), cos(ang));
                    (x, y) = (r * c, r * s);
                }
                Coord::Scroll(dx, dy) => { x += dx; y += dy; }
                Coord::Pixel(n) => {
                    x = ((x * n).floor() + 0.5) / n;
                    y = ((y * n).floor() + 0.5) / n;
                }
                Coord::Repeat(n) => {
                    let tile = |v: f32| { let u = v * n + 0.5; (u - u.floor()) * 2.0 - 1.0 };
                    (x, y) = (tile(x), tile(y));
                }
                Coord::Warp(a, f) => {
                    let (dx, dy) = (a * sin(y * f + t), a * sin(x * f + t * 1.3));
                    x += dx;
                    y += dy;
                }
            }
        }
        let mut c = match self.src {
            Src::Osc(f, ph, [s1, c1, s2, c2]) => {
                // sin(v + o) = sin v cos o + cos v sin o : un seul sinus par point
                let (sv, cv) = (sin(x * f + ph), cos(x * f + ph));
                [0.5 + 0.5 * sv, 0.5 + 0.5 * (sv * c1 + cv * s1), 0.5 + 0.5 * (sv * c2 + cv * s2)]
            }
            Src::Noise(sc, z) => { let n = vnoise(x * sc, y * sc, z); [n, n, n] }
            Src::Shape { poly, sector, r, e } => {
                let len = len(x, y);
                let d = if poly {
                    let ang = atan2(x, y) + PI;
                    cos((ang / sector + 0.5).floor() * sector - ang) * len
                } else { len };
                let k = ((d - r) / e).clamp(0.0, 1.0);
                let v = 1.0 - k * k * (3.0 - 2.0 * k);
                [v, v, v]
            }
            Src::Rings(f, ph) => { let v = 0.5 + 0.5 * sin(len(x, y) * f - ph); [v, v, v] }
            Src::Grad(b) => [(0.5 + 0.5 * x).clamp(0.0, 1.0), (0.5 + 0.5 * y).clamp(0.0, 1.0), b],
            Src::Solid(rgb) => rgb,
        };
        for op in &self.colors {
            c = match *op {
                Col::Hue(m) => [0, 1, 2].map(|r| m[r][0] * c[0] + m[r][1] * c[1] + m[r][2] * c[2]),
                Col::Bright(b) => c.map(|v| v + b),
                Col::Contrast(k) => c.map(|v| (v - 0.5) * k + 0.5),
                Col::Gain(g) => c.map(|v| v * g),
                Col::Color(m) => [c[0] * m[0], c[1] * m[1], c[2] * m[2]],
                Col::Invert(a) => c.map(|v| v + (1.0 - 2.0 * v) * a),
                Col::Thresh(th) => c.map(|v| if v >= th { 1.0 } else { 0.0 }),
                Col::Poster(n) => c.map(|v| (v.clamp(0.0, 1.0) * n).floor().min(n - 1.0) / (n - 1.0).max(1.0)),
            };
        }
        c.map(|v| if v.is_finite() { v } else { 0.0 })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(s: &str, i: Inputs) -> f32 { Expr::parse(s).unwrap().eval(&i.vals()) }

    #[test]
    fn expressions() {
        let i = Inputs { t: 2.0, low: 0.5, mid: 0.25, ..Default::default() };
        assert_eq!(ev("1+2*3", i), 7.0);
        assert_eq!(ev("(1+2)*3", i), 9.0);
        assert_eq!(ev("-low*2", i), -1.0);
        assert_eq!(ev("2^3", i), 8.0);
        assert_eq!(ev("t%1.5", i), 0.5);
        assert_eq!(ev("max(low,mid)+min(1,t)", i), 1.5);
        assert!((ev("sin(pi/2)", i) - 1.0).abs() < 1e-6);
        assert_eq!(ev("1/0", i), 0.0);
        assert_eq!(ev("fract(2.75)", i), 0.75);
        assert!(Expr::parse("lowx").is_err());
        assert!(Expr::parse("(1+2").is_err());
        assert!(Expr::parse("sin 2").is_err());
        assert!(Expr::parse("1 2").is_err());
        assert!(Expr::parse("max(1)").is_err());
        // l'emboîtement est borné : erreur, ni débordement de pile ni 0 silencieux
        let deep = format!("{}1{}", "(".repeat(5000), ")".repeat(5000));
        assert!(Expr::parse(&deep).is_err());
        assert!(Expr::parse(&"-".repeat(100_000)).is_err());
        assert!(Expr::parse(&format!("{}0.5{}", "max(0,".repeat(63), ")".repeat(63))).is_err());
        assert_eq!(ev(&format!("{}0.5{}", "max(0,".repeat(20), ")".repeat(20)), i), 0.5);
        assert!(Vis::parse(&format!("vis: osc {}", "(".repeat(20000))).layers.is_empty());
    }

    #[test]
    fn lignes_et_erreurs() {
        let v = Vis::parse("bpm: 120\nvis: osc 12 0.1 | kaleid 5 | rotate (t * 0.2) | hue mid # couleur\nkick: x . x .\nvis: nope 1\nvis: kaleid 3\nvis: osc | noise\nvis: osc 1 2 3 4");
        assert_eq!(v.layers.len(), 1);
        assert_eq!(v.errors.len(), 4, "{:?}", v.errors);
        assert!(v.errors[0].contains("nope"));
        assert!(v.errors[1].contains("source"));
        // une ligne vis: n'est pas une voix
        assert!(crate::Program::parse("vis: osc 4\nkick: x . . .").voices.iter().all(|v| v.name != "vis"));
    }

    #[test]
    fn l_image_suit_le_son() {
        let mut buf = Vec::new();
        // un disque qui grossit avec les basses : plus de blanc quand low monte
        let v = Vis::parse("vis: shape 0 0.2+low*0.6 0.01");
        let white = |low: f32, buf: &mut Vec<u8>| {
            v.render(32, 32, &Inputs { low, ..Default::default() }, buf);
            buf.chunks(4).filter(|p| p[0] > 128).count()
        };
        let (quiet, loud) = (white(0.0, &mut buf), white(1.0, &mut buf));
        assert!(quiet > 20 && loud > quiet * 10, "{quiet} {loud}");
        assert_eq!(buf.len(), 32 * 32 * 4);
        assert!(buf.chunks(4).all(|p| p[3] == 255));
        // le centre est blanc ; près du haut, blanc seulement quand les basses tapent ; le coin reste noir
        let px = |b: &[u8], i: usize, j: usize| b[(j * 32 + i) * 4];
        assert_eq!((px(&buf, 16, 16), px(&buf, 16, 3), px(&buf, 0, 0)), (255, 255, 0));
        v.render(32, 32, &Inputs::default(), &mut buf);
        assert_eq!((px(&buf, 16, 16), px(&buf, 16, 3)), (255, 0));
    }

    #[test]
    fn approximations() {
        for k in -2000..2000 {
            let x = k as f32 * 0.01;
            assert!((sin(x) - x.sin()).abs() < 2e-3 && (cos(x) - x.cos()).abs() < 2e-3, "{x}");
            let (a, b) = ((k as f32 * 0.37).sin() * 3.0, (k as f32 * 0.11).cos() * 2.0);
            assert!((atan2(a, b) - a.atan2(b)).abs() < 1e-3, "{a} {b} {}", (atan2(a, b) - a.atan2(b)).abs());
        }
        assert_eq!((sin(f32::NAN), atan2(0.0, 0.0)), (0.0, 0.0));
        for x in [1e6f32, 1e9, -1e9, 3.4e38] { assert!(sin(x).abs() <= 1.0001, "{x} {}", sin(x)); }
    }

    #[test]
    fn etapes() {
        let mut buf = Vec::new();
        let px = |v: &str, inp: Inputs, buf: &mut Vec<u8>| { Vis::parse(v).render(8, 8, &inp, buf); buf[..3].to_vec() };
        // solid, color, gain, invert, bright
        assert_eq!(px("vis: solid 1 0.5 0 | color 1 1 1 | gain 0.5", Inputs::default(), &mut buf), [128, 64, 0]);
        assert_eq!(px("vis: solid 1 0 0 | invert", Inputs::default(), &mut buf), [0, 255, 255]);
        assert_eq!(px("vis: solid 0 0 0 | bright level", Inputs { level: 1.0, ..Default::default() }, &mut buf), [255, 255, 255]);
        // hue d'un tiers de tour : le rouge devient vert
        let g = px("vis: solid 1 0 0 | hue 1/3", Inputs::default(), &mut buf);
        assert!(g[1] > 250 && g[0] < 5 && g[2] < 5, "{g:?}");
        // posterize et thresh
        assert_eq!(px("vis: solid 0.6 0.2 0.9 | thresh 0.5", Inputs::default(), &mut buf), [255, 0, 255]);
        assert_eq!(px("vis: solid 0.6 0.2 0.9 | posterize 2", Inputs::default(), &mut buf), [255, 0, 255]);
        // deux couches s'additionnent
        assert_eq!(px("vis: solid 0.25 0 0\nvis: solid 0.25 0 0.5", Inputs::default(), &mut buf), [128, 0, 128]);
        // chaque source et chaque déformation dessine sans NaN ni panique
        for s in ["osc 10 0.1 1", "noise 3 0.5", "shape 6 0.3", "rings 8 1", "grad 1"] {
            for c in ["rotate 1 0.5", "scale 0", "kaleid 0", "scroll 0.1 0.2", "pixel 0", "repeat 3", "warp 0.3 4"] {
                Vis::parse(&format!("vis: {s} | {c} | contrast 2")).render(16, 9, &Inputs { t: 3.7, ..Default::default() }, &mut buf);
                assert_eq!(buf.len(), 16 * 9 * 4);
            }
        }
        // osc bouge avec le temps
        let a = px("vis: osc 10 0.3", Inputs { t: 0.0, ..Default::default() }, &mut buf);
        let b = px("vis: osc 10 0.3", Inputs { t: 0.4, ..Default::default() }, &mut buf);
        assert_ne!(a, b);
    }
}
