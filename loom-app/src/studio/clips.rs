//! CLIPS: vary one voice live without touching the code.
//!
//! The browser's "clips par voix": each voice has slots — `orig` (as written),
//! `dev` (develop), `rev`, `mut` (mutate 30), `8va` (add 7), `½` (thin 2),
//! `×2` (fast 2). Picking a slot adds that verb to the voice, but only in the
//! code sent to the engine: the editor keeps your text. Like any RUN while
//! playing, the change lands on the next bar line.

/// `(button label, verb added to the voice)`; `""` = as written.
pub const SLOTS: [(&str, &str); 7] = [
    ("orig", ""),
    ("dev", "develop"),
    ("rev", "rev"),
    ("mut", "mutate 30"),
    ("8va", "add 7"),
    ("½", "thin 2"),
    ("×2", "fast 2"),
];

/// The slot chosen for each voice (voices not listed play as written).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Clips {
    chosen: Vec<(String, &'static str)>,
}

impl Clips {
    /// Choose a slot's verb for a voice (`""` goes back to the original).
    pub fn set(&mut self, voice: &str, verb: &'static str) {
        self.chosen.retain(|(v, _)| v != voice);
        if !verb.is_empty() {
            self.chosen.push((voice.to_string(), verb));
        }
    }

    /// The verb playing on a voice, `""` if none.
    pub fn verb_for(&self, voice: &str) -> &'static str {
        self.chosen.iter().find(|(v, _)| v == voice).map_or("", |(_, verb)| *verb)
    }

    #[allow(dead_code)] // used by the terminal app
    pub fn is_empty(&self) -> bool {
        self.chosen.is_empty()
    }

    pub fn clear(&mut self) {
        self.chosen.clear();
    }

    /// The code the engine should play: `src` with each clip's verb added to
    /// its voice's lines (`bass:` and its `bass@drop:` variants).
    pub fn apply(&self, src: &str) -> String {
        if self.chosen.is_empty() {
            return src.to_string();
        }
        let lines: Vec<String> = src
            .split('\n')
            .map(|line| match voice_of(line).map(|name| self.verb_for(&name)) {
                Some(verb) if !verb.is_empty() => add_verb(line, verb),
                _ => line.to_string(),
            })
            .collect();
        lines.join("\n")
    }
}

/// The voice a code line belongs to (`bass@drop: …` → `bass`).
fn voice_of(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    if trimmed.starts_with('#') {
        return None;
    }
    let head = trimmed[..trimmed.find(':')?].trim();
    let name = head.split('@').next().unwrap_or(head);
    (!name.is_empty() && !name.contains(char::is_whitespace)).then(|| name.to_lowercase())
}

/// `lead: c4 e4 | tri # note` + `rev` → `lead: c4 e4 | tri | rev # note`.
fn add_verb(line: &str, verb: &str) -> String {
    let (code, comment) = match line.find(" #") {
        Some(i) => line.split_at(i),
        None => (line, ""),
    };
    format!("{} | {verb}{comment}", code.trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verbs_go_to_the_engine_code_only() {
        let src = "bpm: 100\nlead: c4 e4 | tri # la mélodie\nlead@drop: g4\nkick: x . x .";
        let mut clips = Clips::default();
        assert_eq!(clips.apply(src), src);
        clips.set("lead", "rev");
        assert_eq!(clips.apply(src), "bpm: 100\nlead: c4 e4 | tri | rev # la mélodie\nlead@drop: g4 | rev\nkick: x . x .");
        clips.set("lead", "");
        clips.set("kick", "fast 2");
        assert_eq!(clips.verb_for("lead"), "");
        assert_eq!(clips.apply("kick: x . x ."), "kick: x . x . | fast 2");
        // the result still parses to the same voices
        let p = loom_core::Program::parse(&clips.apply(src));
        assert_eq!(p.voices.len(), 3);
    }
}
