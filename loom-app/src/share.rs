//! Sharing songs with the browser version of LOOM.
//!
//! The web app has two formats, both supported here in both directions:
//! - a LINK: `…/index.html#s=<base64 of the code>`;
//! - a SHARE CODE: `LOOM1:<base64 of {"c": code, "bpm": …, "sw": …, "ec": …,
//!   "sp": …, "vo": …, "eql": …, "eqm": …, "eqh": …}>` (the knob values are
//!   strings, as the browser's sliders give them).

/// A song plus the browser's knob settings.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ShareState {
    pub code: String,
    pub bpm: Option<f64>,
    /// SWING knob, 0..100.
    pub swing: Option<f64>,
    /// ECHO knob, 0..100.
    pub echo: Option<f64>,
    /// SPACE knob, 0..100.
    pub space: Option<f64>,
    /// VOL knob, 0..150.
    pub volume: Option<f64>,
    /// Master EQ, dB.
    pub eq_low: Option<f64>,
    pub eq_mid: Option<f64>,
    pub eq_high: Option<f64>,
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn base64_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        for i in 0..4 {
            if i <= chunk.len() { out.push(B64[(n >> (18 - 6 * i) & 63) as usize] as char) } else { out.push('=') }
        }
    }
    out
}

/// Decode standard or URL-safe base64, ignoring whitespace and padding.
pub fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in text.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' | b' ' | b'\n' | b'\r' | b'\t' => continue,
            _ => return None,
        } as u32;
        acc = acc << 6 | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// The `#s=…` part of a browser link for this code.
pub fn link_fragment(code: &str) -> String {
    format!("#s={}", base64_encode(code.as_bytes()))
}

/// A `LOOM1:` share code with the song and knob values.
pub fn share_code(state: &ShareState) -> String {
    let mut json = format!("{{\"c\":{}", json_string(&state.code));
    let knobs = [("bpm", state.bpm), ("sw", state.swing), ("ec", state.echo), ("sp", state.space),
                 ("vo", state.volume), ("eql", state.eq_low), ("eqm", state.eq_mid), ("eqh", state.eq_high)];
    for (key, value) in knobs {
        if let Some(v) = value { json.push_str(&format!(",\"{key}\":\"{}\"", trim_number(v))); }
    }
    json.push('}');
    format!("LOOM1:{}", base64_encode(json.as_bytes()))
}

/// Read anything a user might paste: a `LOOM1:` code, a browser link with
/// `#s=`, a bare `s=…`, or plain LOOM code. `None` if it isn't recognisable.
pub fn decode(text: &str) -> Option<ShareState> {
    let t = text.trim();
    if let Some(rest) = t.strip_prefix("LOOM1:") {
        return decode_json_state(rest);
    }
    if let Some(i) = t.find("#s=").map(|i| i + 3).or_else(|| t.strip_prefix("s=").map(|_| 2)) {
        let payload: String = t[i..].chars().take_while(|c| !c.is_whitespace() && *c != '&').collect();
        let bytes = base64_decode(&percent_decode(&payload))?;
        return String::from_utf8(bytes).ok().map(|code| ShareState { code, ..Default::default() });
    }
    // a bare base64 JSON state (the browser also accepts it without the prefix)
    if !t.contains(char::is_whitespace) && t.len() > 8 {
        if let Some(state) = decode_json_state(t) { return Some(state); }
    }
    // plain LOOM code
    (!loom_core::Program::parse(t).voices.is_empty()).then(|| ShareState { code: t.to_string(), ..Default::default() })
}

fn decode_json_state(b64: &str) -> Option<ShareState> {
    let json = String::from_utf8(base64_decode(b64)?).ok()?;
    let fields = parse_flat_json(&json)?;
    let get = |k: &str| fields.iter().find(|(key, _)| key == k).map(|(_, v)| v.as_str());
    let num = |k: &str| get(k).and_then(|v| v.trim().parse::<f64>().ok());
    Some(ShareState {
        code: get("c")?.to_string(),
        bpm: num("bpm"), swing: num("sw"), echo: num("ec"), space: num("sp"), volume: num("vo"),
        eq_low: num("eql"), eq_mid: num("eqm"), eq_high: num("eqh"),
    })
}

fn trim_number(v: f64) -> String {
    if v.fract() == 0.0 { format!("{}", v as i64) } else { format!("{v}") }
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) { out.push(v); i += 3; continue; }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Parse a flat JSON object whose values are strings, numbers, booleans or
/// null (all returned as text). Enough for the browser's share state.
fn parse_flat_json(json: &str) -> Option<Vec<(String, String)>> {
    let chars: Vec<char> = json.chars().collect();
    let mut i = 0;
    let skip_ws = |i: &mut usize| { while *i < chars.len() && chars[*i].is_whitespace() { *i += 1; } };
    let read_string = |i: &mut usize| -> Option<String> {
        if chars.get(*i) != Some(&'"') { return None; }
        *i += 1;
        let mut s = String::new();
        while *i < chars.len() {
            let c = chars[*i];
            *i += 1;
            match c {
                '"' => return Some(s),
                '\\' => {
                    let e = *chars.get(*i)?;
                    *i += 1;
                    match e {
                        'n' => s.push('\n'), 't' => s.push('\t'), 'r' => s.push('\r'),
                        'b' => s.push('\u{8}'), 'f' => s.push('\u{c}'),
                        'u' => {
                            let hex: String = chars.get(*i..*i + 4)?.iter().collect();
                            *i += 4;
                            let mut code = u32::from_str_radix(&hex, 16).ok()?;
                            // surrogate pair (emoji…)
                            if (0xD800..0xDC00).contains(&code) && chars.get(*i) == Some(&'\\') && chars.get(*i + 1) == Some(&'u') {
                                let lo: String = chars.get(*i + 2..*i + 6)?.iter().collect();
                                let lo = u32::from_str_radix(&lo, 16).ok()?;
                                *i += 6;
                                code = 0x10000 + ((code - 0xD800) << 10) + (lo - 0xDC00);
                            }
                            s.push(char::from_u32(code).unwrap_or('\u{FFFD}'));
                        }
                        other => s.push(other),
                    }
                }
                c => s.push(c),
            }
        }
        None
    };
    skip_ws(&mut i);
    if chars.get(i) != Some(&'{') { return None; }
    i += 1;
    let mut out = Vec::new();
    loop {
        skip_ws(&mut i);
        match chars.get(i) {
            Some('}') => return Some(out),
            Some(',') => { i += 1; continue; }
            Some('"') => {}
            _ => return None,
        }
        let key = read_string(&mut i)?;
        skip_ws(&mut i);
        if chars.get(i) != Some(&':') { return None; }
        i += 1;
        skip_ws(&mut i);
        let value = if chars.get(i) == Some(&'"') {
            read_string(&mut i)?
        } else {
            let start = i;
            while i < chars.len() && !matches!(chars[i], ',' | '}') { i += 1; }
            chars[start..i].iter().collect::<String>().trim().to_string()
        };
        out.push((key, value));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_round_trips() {
        for s in ["", "a", "ab", "abc", "kick: x . x . # thème ✦"] {
            assert_eq!(base64_decode(&base64_encode(s.as_bytes())).unwrap(), s.as_bytes());
        }
        assert_eq!(base64_encode(b"LOOM"), "TE9PTQ==");
    }

    #[test]
    fn reads_browser_links_and_codes() {
        // what the browser's btoa(unescape(encodeURIComponent("kick: x"))) gives
        let link = "https://example.org/loom/index.html#s=a2ljazogeA==";
        assert_eq!(decode(link).unwrap().code, "kick: x");
        // a browser share code: JSON with string knob values
        let json = r#"{"c":"bass: c2 . | saw\n# é","bpm":"128","sw":"12","ec":"15","sp":"20","vo":"80","eql":"0","eqm":"-1.5","eqh":"2","mb":0}"#;
        let st = decode(&format!("LOOM1:{}", base64_encode(json.as_bytes()))).unwrap();
        assert_eq!(st.code, "bass: c2 . | saw\n# é");
        assert_eq!((st.bpm, st.swing, st.eq_mid), (Some(128.0), Some(12.0), Some(-1.5)));
    }

    #[test]
    fn share_code_round_trips_and_plain_code_is_accepted() {
        let st = ShareState { code: "lead: \"c4\" . | tri\n".into(), bpm: Some(96.0), space: Some(35.5), ..Default::default() };
        assert_eq!(decode(&share_code(&st)).unwrap(), st);
        assert_eq!(decode(&link_fragment("hat: x x")).unwrap().code, "hat: x x");
        assert_eq!(decode("kick: x . x .").unwrap().code, "kick: x . x .");
        assert!(decode("bonjour").is_none());
    }
}
