//! SAMPLER: turn any voice into a sample player. Pure logic, no drawing:
//! read a voice line's sample verbs the way the engine hears them, and write
//! them back one at a time (`| sample`, `| root`, `| begin/end`, `| chop`,
//! `| reverse`) while keeping the pattern, the other effects and the comment.
//!
//! The audio side (`loom_audio::sample`) already plays all of these; this
//! module only rewrites the code so the GUI's panel can drive them live.

use crate::vibe::strip_comment;

/// What the sampler panel edits for one voice.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SampleSpec {
    pub sample: Option<String>,
    /// The note the sample was recorded at (`| root NOM`, c3 by default).
    pub root: Option<String>,
    /// Playback window 0..=1 into the sample.
    pub begin: Option<f32>,
    pub end: Option<f32>,
    /// Number of slices played in turn (`| chop N`); `None` = off.
    pub chop: Option<u32>,
    /// `| reverse`.
    pub reverse: bool,
}

impl SampleSpec {
    /// The current state of a voice, exactly as the engine hears it.
    pub fn from_synth(s: &loom_core::Synth) -> SampleSpec {
        SampleSpec {
            sample: s.sample.clone(),
            root: s.root.clone(),
            begin: s.begin,
            end: s.end,
            chop: s.chop,
            reverse: s.reverse,
        }
    }
}

/// One verb to write onto the armed voice's line. `Some` sets it, `None`
/// removes it; `Reverse(false)` also removes.
#[derive(Debug, Clone, PartialEq)]
pub enum Verb {
    Sample(Option<String>),
    Root(Option<String>),
    Begin(Option<f32>),
    End(Option<f32>),
    Chop(Option<u32>),
    Reverse(bool),
}

impl Verb {
    /// The heads the parser accepts for this verb (e.g. `s` is `sample`).
    fn heads(&self) -> &'static [&'static str] {
        match self {
            Verb::Sample(_) => &["sample", "s"],
            Verb::Root(_) => &["root"],
            Verb::Begin(_) => &["begin"],
            Verb::End(_) => &["end"],
            Verb::Chop(_) => &["chop"],
            Verb::Reverse(_) => &["reverse", "rev?"],
        }
    }

    /// The canonical head to write.
    fn head(&self) -> &'static str {
        match self {
            Verb::Sample(_) => "sample",
            Verb::Root(_) => "root",
            Verb::Begin(_) => "begin",
            Verb::End(_) => "end",
            Verb::Chop(_) => "chop",
            Verb::Reverse(_) => "reverse",
        }
    }

    /// What to write after the head; `None` = this verb is removed.
    pub fn value(&self) -> Option<String> {
        match self {
            Verb::Sample(Some(s)) => Some(s.clone()),
            Verb::Root(Some(r)) => Some(r.clone()),
            Verb::Begin(Some(x)) => Some(fmt_f32(*x)),
            Verb::End(Some(x)) => Some(fmt_f32(*x)),
            Verb::Chop(Some(n)) => Some(n.to_string()),
            Verb::Reverse(true) => Some(String::new()),
            _ => None,
        }
    }

    /// `head value`, e.g. `sample bd`; `reverse` alone for the toggle.
    fn render(&self) -> String {
        match self.value() {
            Some(v) if v.is_empty() => self.head().to_string(),
            Some(v) => format!("{} {v}", self.head()),
            None => String::new(),
        }
    }
}

/// `0.25` → `0.25`, `0.5` → `0.5`, `0.0` → `0` (no long float tails).
fn fmt_f32(x: f32) -> String {
    let mut s = format!("{x:.3}");
    while s.ends_with('0') {
        s.pop();
    }
    if s.ends_with('.') {
        s.pop();
    }
    s
}

/// Rewrite ONE voice's sample verbs in `src`. The line keeps its pattern, its
/// other effects and its comment, and must re-parse. `voice` may name a
/// `@section` variant (`bass@drop`).
pub fn apply_verb(src: &str, voice: &str, verb: &Verb) -> Result<String, String> {
    let want = voice.trim().to_lowercase();
    let mut found = false;
    let lines: Vec<String> = src
        .split('\n')
        .map(|line| {
            let code = strip_comment(line);
            let Some((head, _)) = code.split_once(':') else { return line.to_string() };
            if head.trim().to_lowercase() != want {
                return line.to_string();
            }
            found = true;
            let comment = &line[code.len()..];
            let code = set_verb(code.trim_end(), verb);
            if !comment.is_empty() {
                format!("{code}  {comment}")
            } else {
                code
            }
        })
        .collect();
    if !found {
        return Err(format!("aucune voix « {voice} » — vérifie son nom."));
    }
    Ok(lines.join("\n"))
}

/// Write every sampler verb of `spec` onto the chosen voice's line.
pub fn apply_spec(src: &str, voice: &str, spec: &SampleSpec) -> Result<String, String> {
    let mut out = src.to_string();
    for verb in [
        Verb::Sample(spec.sample.clone()),
        Verb::Root(spec.root.clone()),
        Verb::Begin(spec.begin),
        Verb::End(spec.end),
        Verb::Chop(spec.chop),
        Verb::Reverse(spec.reverse),
    ] {
        out = apply_verb(&out, voice, &verb)?;
    }
    Ok(out)
}

/// The spans (start, end) of every `| segment` in `code`, `|` included.
fn spans(code: &str) -> Vec<(usize, usize)> {
    let mut v = Vec::new();
    let mut pos = 0;
    while let Some(rel) = code[pos..].find('|') {
        let seg_start = pos + rel;
        let seg_end = code[seg_start + 1..].find('|').map(|e| seg_start + 1 + e).unwrap_or(code.len());
        v.push((seg_start, seg_end));
        pos = seg_end;
        if seg_end == code.len() {
            break;
        }
    }
    v
}

/// The first word of a `| segment` (its verb head).
fn head_of(seg: &str) -> Option<&str> {
    seg.split_whitespace().next()
}

/// Set or remove one verb on a comment-free voice line.
fn set_verb(code: &str, verb: &Verb) -> String {
    let all = spans(code);
    let is_matching = |s: usize, e: usize| {
        head_of(&code[s + 1..e]).is_some_and(|h| verb.heads().iter().any(|hh| h.eq_ignore_ascii_case(hh)))
    };
    if verb.value().is_some() {
        // replace the LAST segment with this head, or append one.
        if let Some(&(s, e)) = all.iter().rev().find(|(s, e)| is_matching(*s, *e)) {
            let replaced = &code[s..e];
            let sep = if replaced.ends_with(char::is_whitespace) { " " } else { "" }; // keep the next `|` separated
            let mut out = code.to_string();
            out.replace_range(s..e, &format!("| {}{}", verb.render(), sep));
            out
        } else {
            format!("{code} | {}", verb.render())
        }
    } else {
        // remove every segment with this head.
        let mut out = String::new();
        let mut prev = 0;
        for (s, e) in &all {
            if is_matching(*s, *e) {
                out.push_str(&code[prev..*s]);
            } else {
                out.push_str(&code[prev..*e]);
            }
            prev = *e;
        }
        out.push_str(&code[prev..]);
        out.trim_end().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use loom_core::Program;

    #[test]
    fn sets_sample_and_keeps_the_comment() {
        let src = "kick: x . x .\nlead: 1 3 5 1' | pulse # une mélodie\n";
        let t = apply_verb(src, "lead", &Verb::Sample(Some("pad".into()))).unwrap();
        assert!(t.contains("lead: 1 3 5 1' | pulse | sample pad  # une mélodie"), "{t}");
        assert_eq!(t.lines().count(), 2);
        assert!(Program::parse(&t).voices.len() == 2);
    }

    #[test]
    fn replaces_the_last_matching_segment_and_aliases() {
        // `s` is the parser's alias of `sample`: the last one wins when heard,
        // so it is replaced too.
        let t = apply_verb("hat: x x x x | s bd | filter", "hat", &Verb::Sample(Some("clap".into()))).unwrap();
        assert!(t.contains("hat: x x x x | sample clap | filter"), "{t}");
        assert!(!t.contains("s bd"), "{t}");
        let t = apply_verb("hat: x x x x | sample bd | filter", "hat", &Verb::Sample(Some("clap".into()))).unwrap();
        assert!(t.contains("hat: x x x x | sample clap | filter"), "{t}");
        assert!(!t.contains("sample bd"), "{t}");
    }

    #[test]
    fn removes_verbs_without_touching_the_pattern() {
        let src = "lead: 1 3 5 1' | begin 0.5 | pulse | reverse\n";
        let t = apply_verb(src, "lead", &Verb::Begin(None)).unwrap();
        assert!(t.contains("lead: 1 3 5 1'"), "{t}");
        assert!(!t.contains("begin"), "{t}");
        assert!(t.contains("pulse | reverse"), "{t}");
        let t = apply_verb(&t, "lead", &Verb::Reverse(false)).unwrap();
        assert!(!t.contains("reverse"), "{t}");
        assert!(Program::parse(&t).voices[0].synth.begin.is_none());
        assert!(!Program::parse(&t).voices[0].synth.reverse);
    }

    #[test]
    fn apply_spec_writes_the_whole_state() {
        let src = "bass: 1 . 5, .\n";
        let spec = SampleSpec {
            sample: Some("amen".into()),
            root: Some("a2".into()),
            begin: Some(0.25),
            end: Some(0.75),
            chop: Some(4),
            reverse: true,
        };
        let t = apply_spec(src, "bass", &spec).unwrap();
        assert!(t.contains("| sample amen"), "{t}");
        assert!(t.contains("| root a2"), "{t}");
        assert!(t.contains("| begin 0.25"), "{t}");
        assert!(t.contains("| end 0.75"), "{t}");
        assert!(t.contains("| chop 4"), "{t}");
        assert!(t.contains("| reverse"), "{t}");
        let v = &Program::parse(&t).voices[0].synth;
        assert_eq!(v.sample.as_deref(), Some("amen"));
        assert_eq!(v.root.as_deref(), Some("a2"));
        assert_eq!(v.begin, Some(0.25));
        assert_eq!(v.end, Some(0.75));
        assert_eq!(v.chop, Some(4));
        assert!(v.reverse);
        // idempotent: applying again keeps a single each
        let t2 = apply_spec(&t, "bass", &spec).unwrap();
        assert_eq!(t2.matches("sample amen").count(), 1);
        assert_eq!(t2.matches("reverse").count(), 1);
    }

    #[test]
    fn unknown_voice_and_variant_are_handled() {
        assert!(apply_verb("a: 1 2 3", "wow", &Verb::Chop(Some(4))).unwrap_err().contains("aucune voix"));
        let var = "bass: 1 . 5, .\nbass@drop: 1 1 1 1\n";
        let t = apply_verb(var, "bass@drop", &Verb::Sample(Some("sub".into()))).unwrap();
        assert!(t.contains("bass@drop: 1 1 1 1 | sample sub"), "{t}");
        assert!(t.contains("bass: 1 . 5, ."), "{t}");
    }

    #[test]
    fn fmt_f32_is_short() {
        assert_eq!(fmt_f32(0.5), "0.5");
        assert_eq!(fmt_f32(0.25), "0.25");
        assert_eq!(fmt_f32(0.0), "0");
        assert_eq!(fmt_f32(1.0), "1");
    }
}
