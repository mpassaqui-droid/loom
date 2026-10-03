//! Petites expressions entières pures : le cœur des voix bytebeat (`t: …`) et
//! des pas-programmes (`=(…)`).
//!
//! Pas d'`eval`, pas de dépendance : un analyseur à descente récursive compile
//! le texte en une suite d'opérations (notation polonaise inverse), évaluée sur
//! une petite pile.
//!
//! Sémantique (identique bit pour bit au miroir JS de `web/index.html`) :
//! - tout est en **entiers signés 32 bits** qui débordent en boucle (`i32`
//!   wrapping), exactement ce que JS obtient avec `|0`, `Math.imul`, `<<`, `>>` ;
//! - `/` tronque vers zéro, `%` garde le signe du dividende ; diviser par zéro
//!   donne 0 (pas d'erreur, pas de NaN) ;
//! - `<<` et `>>` masquent le décalage sur 5 bits (`& 31`) comme en C et en JS ;
//!   `>>` est arithmétique (signe conservé) ;
//! - les comparaisons `< <= > >= == !=` rendent 0 ou 1 ;
//! - littéraux décimaux ou hexadécimaux (`0x…`) jusqu'à 4294967295, ramenés
//!   en 32 bits signés (`0xffffffff` vaut -1, comme `0xffffffff|0` en JS).
//!
//! Précédence (celle du C, de la plus forte à la plus faible) :
//! unaire `- ~ +`, puis `* / %`, `+ -`, `<< >>`, `< <= > >=`, `== !=`, `&`,
//! `^`, `|`. Les variables sont des lettres seules déclarées par l'appelant
//! (`t` pour le bytebeat, `i b n` pour les pas-programmes).

/// Une opération de la pile.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Op {
    Lit(i32),
    Var(u8),
    Neg,
    Not,
    Bin(Bin),
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Bin { Mul, Div, Rem, Add, Sub, Shl, Shr, Lt, Le, Gt, Ge, Eq, Ne, And, Xor, Or }

/// Une expression compilée. `vars` dit quelle lettre occupe quel indice dans
/// le tableau passé à [`Expr::eval`].
#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    ops: Vec<Op>,
    /// Le texte source, nettoyé (pour l'affichage et la comparaison des voix).
    pub src: String,
}

/// Profondeur maximale d'imbrication (parenthèses, unaires) : protège la pile.
const MAX_DEPTH: usize = 64;

struct Parser<'a> {
    b: &'a [u8],
    i: usize,
    vars: &'a [char],
    ops: Vec<Op>,
    depth: usize,
}

impl Parser<'_> {
    fn ws(&mut self) { while self.i < self.b.len() && self.b[self.i].is_ascii_whitespace() { self.i += 1; } }
    fn peek(&mut self) -> Option<u8> { self.ws(); self.b.get(self.i).copied() }
    fn starts(&mut self, s: &str) -> bool { self.ws(); self.b[self.i..].starts_with(s.as_bytes()) }

    /// Niveau binaire `lvl` (0 = `|`, le plus faible … 7 = `* / %`).
    fn binary(&mut self, lvl: usize) -> Result<(), String> {
        if lvl == 8 { return self.unary(); }
        self.binary(lvl + 1)?;
        loop {
            // les opérateurs à deux caractères d'abord, et `|`/`&` ne doivent pas
            // avaler `||`/`&&` (absents du langage : erreur plus loin)
            let op = match lvl {
                0 => if self.starts("|") && !self.starts("||") { Some((Bin::Or, 1)) } else { None },
                1 => if self.starts("^") { Some((Bin::Xor, 1)) } else { None },
                2 => if self.starts("&") && !self.starts("&&") { Some((Bin::And, 1)) } else { None },
                3 => if self.starts("==") { Some((Bin::Eq, 2)) } else if self.starts("!=") { Some((Bin::Ne, 2)) } else { None },
                4 => if self.starts("<<") || self.starts(">>") { None }
                     else if self.starts("<=") { Some((Bin::Le, 2)) } else if self.starts(">=") { Some((Bin::Ge, 2)) }
                     else if self.starts("<") { Some((Bin::Lt, 1)) } else if self.starts(">") { Some((Bin::Gt, 1)) } else { None },
                5 => if self.starts("<<") { Some((Bin::Shl, 2)) } else if self.starts(">>") { Some((Bin::Shr, 2)) } else { None },
                6 => if self.starts("+") { Some((Bin::Add, 1)) } else if self.starts("-") { Some((Bin::Sub, 1)) } else { None },
                _ => if self.starts("*") { Some((Bin::Mul, 1)) } else if self.starts("/") { Some((Bin::Div, 1)) }
                     else if self.starts("%") { Some((Bin::Rem, 1)) } else { None },
            };
            let Some((op, len)) = op else { return Ok(()) };
            self.i += len;
            self.binary(lvl + 1)?;
            self.ops.push(Op::Bin(op));
        }
    }

    fn unary(&mut self) -> Result<(), String> {
        self.depth += 1;
        if self.depth > MAX_DEPTH { return Err("expression trop imbriquée".into()); }
        let r = match self.peek() {
            Some(b'-') => { self.i += 1; self.unary().map(|_| self.ops.push(Op::Neg)) }
            Some(b'~') => { self.i += 1; self.unary().map(|_| self.ops.push(Op::Not)) }
            Some(b'+') => { self.i += 1; self.unary() }
            _ => self.atom(),
        };
        self.depth -= 1;
        r
    }

    fn atom(&mut self) -> Result<(), String> {
        match self.peek() {
            Some(b'(') => {
                self.i += 1;
                self.binary(0)?;
                if self.peek() != Some(b')') { return Err("« ) » manquante".into()); }
                self.i += 1;
                Ok(())
            }
            Some(c) if c.is_ascii_digit() => {
                let start = self.i;
                let hex = self.b[self.i..].starts_with(b"0x") || self.b[self.i..].starts_with(b"0X");
                if hex { self.i += 2; }
                while self.i < self.b.len() && self.b[self.i].is_ascii_alphanumeric() { self.i += 1; }
                let txt = std::str::from_utf8(&self.b[start..self.i]).unwrap_or("");
                let v = if hex { u64::from_str_radix(&txt[2..], 16) } else { txt.parse::<u64>() }
                    .map_err(|_| format!("nombre invalide « {txt} »"))?;
                if v > u32::MAX as u64 { return Err(format!("nombre trop grand « {txt} »")); }
                self.ops.push(Op::Lit(v as u32 as i32));
                Ok(())
            }
            Some(c) if c.is_ascii_alphabetic() => {
                let start = self.i;
                while self.i < self.b.len() && self.b[self.i].is_ascii_alphanumeric() { self.i += 1; }
                let word = std::str::from_utf8(&self.b[start..self.i]).unwrap_or("");
                let mut ch = word.chars();
                match (ch.next(), ch.next()) {
                    (Some(c), None) => match self.vars.iter().position(|v| *v == c) {
                        Some(k) => { self.ops.push(Op::Var(k as u8)); Ok(()) }
                        None => Err(format!("variable inconnue « {c} »")),
                    },
                    _ => Err(format!("mot inconnu « {word} »")),
                }
            }
            Some(c) => Err(format!("caractère inattendu « {} »", c as char)),
            None => Err("expression incomplète".into()),
        }
    }
}

impl Expr {
    /// Compile `src` avec les variables `vars` (lettres seules, ex. `&['t']`).
    pub fn parse(src: &str, vars: &[char]) -> Result<Expr, String> {
        let mut p = Parser { b: src.as_bytes(), i: 0, vars, ops: Vec::new(), depth: 0 };
        if p.peek().is_none() { return Err("expression vide".into()); }
        p.binary(0)?;
        if let Some(c) = p.peek() { return Err(format!("caractère inattendu « {} »", c as char)); }
        Ok(Expr { ops: p.ops, src: src.split_whitespace().collect::<Vec<_>>().join(" ") })
    }

    /// Évalue l'expression ; `vals[k]` est la valeur de la k-ième variable.
    pub fn eval(&self, vals: &[i32]) -> i32 {
        let mut st: Vec<i32> = Vec::with_capacity(16);
        for op in &self.ops {
            match *op {
                Op::Lit(v) => st.push(v),
                Op::Var(k) => st.push(vals.get(k as usize).copied().unwrap_or(0)),
                Op::Neg => { let a = st.pop().unwrap_or(0); st.push(a.wrapping_neg()); }
                Op::Not => { let a = st.pop().unwrap_or(0); st.push(!a); }
                Op::Bin(op) => {
                    let b = st.pop().unwrap_or(0);
                    let a = st.pop().unwrap_or(0);
                    st.push(match op {
                        Bin::Mul => a.wrapping_mul(b),
                        Bin::Div => if b == 0 { 0 } else { a.wrapping_div(b) },
                        Bin::Rem => if b == 0 { 0 } else { a.wrapping_rem(b) },
                        Bin::Add => a.wrapping_add(b),
                        Bin::Sub => a.wrapping_sub(b),
                        Bin::Shl => a.wrapping_shl(b as u32),
                        Bin::Shr => a.wrapping_shr(b as u32),
                        Bin::Lt => (a < b) as i32,
                        Bin::Le => (a <= b) as i32,
                        Bin::Gt => (a > b) as i32,
                        Bin::Ge => (a >= b) as i32,
                        Bin::Eq => (a == b) as i32,
                        Bin::Ne => (a != b) as i32,
                        Bin::And => a & b,
                        Bin::Xor => a ^ b,
                        Bin::Or => a | b,
                    });
                }
            }
        }
        st.pop().unwrap_or(0)
    }
}

/// Fréquence d'échantillonnage du bytebeat classique : `t` compte à 8000 Hz.
pub const BYTEBEAT_RATE: u32 = 8000;

/// Un échantillon bytebeat : l'octet bas de la valeur, centré sur zéro
/// (`(v & 255) / 128 - 1`, de -1 à 0.9921875).
pub fn bytebeat_sample(v: i32) -> f32 {
    (v & 255) as f32 / 128.0 - 1.0
}

/// Le `t` bytebeat de l'échantillon `n` (compté depuis le début du morceau) à
/// la fréquence `sr` : échantillonné-bloqué, `t = floor(n * 8000 / sr)`, ramené
/// en 32 bits signés. Même calcul exact en JS (`Math.floor(n*8000/sr)|0`, le
/// produit tient dans un double tant que n < 2^40).
pub fn bytebeat_t(n: u64, sr: u32) -> i32 {
    (n.wrapping_mul(BYTEBEAT_RATE as u64) / sr.max(1) as u64) as u32 as i32
}

/// `len` échantillons bytebeat à partir de l'échantillon absolu `start`, à la
/// fréquence `sr` (échantillonnage-blocage). La référence du miroir JS
/// `bytebeatFill`, comparée bit pour bit par `tools/parity_test.js`.
pub fn bytebeat_fill(e: &Expr, start: u64, len: usize, sr: u32) -> Vec<f32> {
    let mut out = Vec::with_capacity(len);
    let mut last: Option<(i32, f32)> = None;
    for k in 0..len as u64 {
        let t = bytebeat_t(start + k, sr);
        let v = match last {
            Some((lt, lv)) if lt == t => lv,
            _ => { let v = bytebeat_sample(e.eval(&[t])); last = Some((t, v)); v }
        };
        out.push(v);
    }
    out
}

/// Coupe le corps d'une voix bytebeat (`t: EXPR | verbe …`) en l'expression et
/// la suite des segments. Un `|` hors parenthèses appartient à l'expression
/// (OU binaire) SAUF s'il est suivi d'un mot d'au moins deux lettres : c'est
/// alors un verbe (`| gain 0.4`, `| lp 900`). Les variables étant des lettres
/// seules, `t*5|t>>7` reste une seule expression.
pub fn split_bytebeat(body: &str) -> (&str, &str) {
    let b = body.as_bytes();
    let mut depth = 0i32;
    for (k, &c) in b.iter().enumerate() {
        match c {
            b'(' => depth += 1,
            b')' => depth -= 1,
            b'|' if depth <= 0 => {
                let rest = body[k + 1..].trim_start();
                let word = rest.bytes().take_while(|c| c.is_ascii_alphabetic()).count();
                if word >= 2 { return (&body[..k], &body[k..]); }
            }
            _ => {}
        }
    }
    (body, "")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(s: &str, t: i32) -> i32 { Expr::parse(s, &['t']).unwrap().eval(&[t]) }

    #[test]
    fn precedence_follows_c() {
        assert_eq!(ev("1+2*3", 0), 7);
        assert_eq!(ev("(1+2)*3", 0), 9);
        assert_eq!(ev("1<<2+1", 0), 8);           // + avant <<
        assert_eq!(ev("1|2&3", 0), 3);            // & avant |
        assert_eq!(ev("6^3&5", 0), 7);            // & avant ^ : 6 ^ (3&5)
        assert_eq!(ev("5|2^7", 0), 5 | (2 ^ 7)); // ^ avant |
        assert_eq!(ev("1+2<4==1", 0), 1);         // (3<4)==1
        assert_eq!(ev("8>>1<4", 0), 0);           // << >> avant <
        assert_eq!(ev("-2*3", 0), -6);
        assert_eq!(ev("~0", 0), -1);
        assert_eq!(ev("-(-3)", 0), 3);
        assert_eq!(ev("10-3-2", 0), 5);           // gauche à droite
        assert_eq!(ev("100/10/5", 0), 2);
        assert_eq!(ev("2>=2", 0), 1);
        assert_eq!(ev("2!=2", 0), 0);
        assert_eq!(ev("3<=2", 0), 0);
        assert_eq!(ev("t*2", 21), 42);
        assert_eq!(ev("0x10+0XfF", 0), 271);
    }

    #[test]
    fn division_by_zero_gives_zero() {
        assert_eq!(ev("7/0", 0), 0);
        assert_eq!(ev("7%0", 0), 0);
        assert_eq!(ev("t/(t-t)", 9), 0);
        assert_eq!(ev("-7/2", 0), -3);            // tronque vers zéro
        assert_eq!(ev("-7%3", 0), -1);            // signe du dividende
    }

    #[test]
    fn wraps_like_js_int32() {
        assert_eq!(ev("2147483647+1", 0), i32::MIN);
        assert_eq!(ev("65536*65536", 0), 0);
        assert_eq!(ev("0xffffffff", 0), -1);
        assert_eq!(ev("1<<33", 0), 2);            // décalage masqué & 31
        assert_eq!(ev("-16>>2", 0), -4);          // arithmétique
        assert_eq!(ev("(0-2147483647-1)/(0-1)", 0), i32::MIN);
        assert_eq!(ev("(0-2147483647-1)%(0-1)", 0), 0);
    }

    #[test]
    fn classic_formulas_known_values() {
        // « 42 melody » : t*(42&t>>10)
        let f = "t*(42&t>>10)";
        assert_eq!(ev(f, 0), 0);
        assert_eq!(ev(f, 1024), 0);               // 42 & 1 = 0
        assert_eq!(ev(f, 2048), 2048 * 2);        // 42 & 2 = 2
        assert_eq!(ev(f, 10000), 10000 * (42 & (10000 >> 10)));
        // Viznut, premier morceau : t*((t>>12|t>>8)&63&t>>4)
        let g = "t*((t>>12|t>>8)&63&t>>4)";
        for t in [0, 1, 255, 4096, 12345, 65535, 1_000_000] {
            assert_eq!(ev(g, t), t.wrapping_mul(((t >> 12) | (t >> 8)) & 63 & (t >> 4)));
        }
        // l'exemple de la doc
        let h = "(t*5&t>>7)|(t*3&t>>10)";
        for t in [0, 99, 8000, 77777] {
            assert_eq!(ev(h, t), ((t * 5) & (t >> 7)) | ((t * 3) & (t >> 10)));
        }
    }

    #[test]
    fn bad_input_is_an_error() {
        for s in ["", "1+", "(1", "x", "tt", "1 2", "1||2", "3 @ 4", "99999999999"] {
            assert!(Expr::parse(s, &['t']).is_err(), "{s}");
        }
        let deep = "(".repeat(200) + "1" + &")".repeat(200);
        assert!(Expr::parse(&deep, &['t']).is_err());
    }

    #[test]
    fn sample_and_clock() {
        assert_eq!(bytebeat_sample(0), -1.0);
        assert_eq!(bytebeat_sample(128), 0.0);
        assert_eq!(bytebeat_sample(256 + 64), -0.5);
        assert_eq!(bytebeat_sample(-1), 255.0 / 128.0 - 1.0);
        assert_eq!(bytebeat_t(0, 44100), 0);
        assert_eq!(bytebeat_t(44100, 44100), 8000);
        assert_eq!(bytebeat_t(5, 44100), 0);      // tenu jusqu'au prochain pas de 8 kHz
        assert_eq!(bytebeat_t(6, 44100), 1);
        assert_eq!(bytebeat_t(48000 * 3, 48000), 24000);
    }

    #[test]
    fn split_keeps_or_and_cuts_verbs() {
        assert_eq!(split_bytebeat(" (t*5&t>>7)|(t*3&t>>10) | gain 0.4"), (" (t*5&t>>7)|(t*3&t>>10) ", "| gain 0.4"));
        assert_eq!(split_bytebeat("t*5 | t>>7"), ("t*5 | t>>7", ""));
        assert_eq!(split_bytebeat("t|(t>>8)|lp 900"), ("t|(t>>8)", "|lp 900"));
        assert_eq!(split_bytebeat("(t | gain)"), ("(t | gain)", ""));
    }
}
