//! LE PROF — the AI music teacher, in plain French prose (never code).
//!
//! Port of the browser's teacher calls (`web/index.html`), the only model calls
//! besides VIBE (already in [`crate::vibe`]):
//! - EXPLIQUE (`explain()`, the `explainBtn`): why the song sounds the way it does,
//! - the `?` question in the VIBE box (`lookup()`): first the offline knowledge
//!   base ([`lookup`]), then the AI teacher for anything else ([`ask`]).
//!
//! The web sent these prompts to a free model (puter / pollinations); here they
//! run through the Claude CLI job runner ([`ClaudeJob`]) with the owner's
//! subscription. The prompts are the web's, word for word, split into a system
//! prompt (who the teacher is) and a message (the task and the code). A short
//! LOOM primer was added to the system prompt, because Claude doesn't know the
//! language. The web's offline analysis shown before the AI answer
//! (`analyzePiece`: harmony, consonance, form) is now [`analyze`].

pub mod analyze;
pub mod lore;

pub use analyze::analyze;

use std::process::Command;
use std::time::Duration;

use crate::ear::{demo, Demo};
use crate::lessons::{normalize, REFERENCE};
use crate::vibe::{claude_command_with, model, ClaudeJob, TIMEOUT};
use lore::{LORE, SCALE_LORE, TRADITION_LORE};

/// A teacher request running in the background: its result is the cleaned French answer.
pub type Job = ClaudeJob<String>;

/// Longest explanation kept. The web cuts at 650 characters to fit its small
/// log box; the native panels scroll, so the teacher gets room to finish.
pub const EXPLAIN_MAX_CHARS: usize = 2000;
/// Longest answer to a question kept (the web's `slice(0,600)`).
pub const ASK_MAX_CHARS: usize = 600;

/// Said when there is no song to explain.
pub const NEED_A_SONG: &str = "[ prof ] écris ou génère d'abord un morceau";
/// Said while the teacher thinks about the song.
pub const THINKING: &str = "[ prof ] j'approfondis… (le cerveau IA)";
/// Said for an empty `?`.
pub const HELP: &str =
    "[ ? ] pose une question : ?raga, ?maqam, ?gamelan, ?modes, ?pythagore, ?dorian, ?quinte, ou ?comment rendre ça plus triste";

const PRIMER: &str = "Le langage LOOM : une voix par ligne « nom: motif | verbe | verbe ». \
Batterie (kick snare hat clap) : x = frappe, . = silence. Mélodies : notes (c3 eb2) ou, avec « key: », degrés de la gamme 1-7 \
(' monte d'une octave, , descend). Accords : Am7, ^ suit la progression « prog: ». Les verbes après | transforment le motif \
(rev, fast, every, develop, mutate…) ou le son (saw, sine, lp = filtre, reverb, delay, gain…). « song: » découpe le morceau en sections.";

/// The teacher who explains a song (the web's persona and rules).
pub fn explain_system_prompt() -> String {
    format!("Tu es un professeur de musique bienveillant et clair. Tu réponds toujours en français, en prose simple : pas de code, pas de listes, pas de markdown.\n{PRIMER}")
}

/// The teacher who answers a question.
pub fn ask_system_prompt() -> String {
    format!("Tu es un professeur de musique clair et bref. Tu réponds toujours en français, en prose simple : pas de listes, pas de code, pas de markdown.\n{PRIMER}")
}

/// The web's explain prompt, with the song.
pub fn explain_message(code: &str) -> String {
    format!(
        "En français, en 3 ou 4 phrases simples (pas de code, pas de listes, pas de markdown), explique ce morceau écrit en langage LOOM : \
son ambiance, la gamme/tonalité et POURQUOI il sonne ainsi, et un conseil pour l'améliorer.\n\nCODE:\n{}",
        code.trim_end()
    )
}

/// The web's question prompt.
pub fn ask_message(question: &str) -> String {
    format!("Réponds en français en 2-3 phrases simples (sans listes, sans code) à cette question : {}", question.trim())
}

/// EXPLIQUE: ask the teacher why `code` sounds the way it does.
pub fn explain(code: &str) -> Job {
    explain_with(claude_command_with(&model(), &explain_system_prompt()), code, TIMEOUT)
}

/// [`explain`] with any command (tests fake Claude). An empty song fails at once.
pub fn explain_with(command: Command, code: &str, timeout: Duration) -> Job {
    if code.trim().is_empty() {
        return ClaudeJob::failed("le prof", NEED_A_SONG.into());
    }
    ClaudeJob::spawn("le prof", command, explain_message(code), timeout, |reply| clean(&reply, EXPLAIN_MAX_CHARS))
}

/// `?question` that the offline [`lookup`] doesn't know: ask the teacher.
pub fn ask(question: &str) -> Job {
    ask_with(claude_command_with(&model(), &ask_system_prompt()), question, TIMEOUT)
}

/// [`ask`] with any command (tests fake Claude). An empty question fails at once.
pub fn ask_with(command: Command, question: &str, timeout: Duration) -> Job {
    if question.trim().is_empty() {
        return ClaudeJob::failed("le prof", HELP.into());
    }
    ClaudeJob::spawn("le prof", command, ask_message(question), timeout, |reply| clean(&reply, ASK_MAX_CHARS))
}

/// The teacher's reply as one paragraph of plain text: code blocks, markdown
/// marks, headings and bullets removed, spaces collapsed (as the web did), cut
/// to `max_chars` at the end of a sentence when possible.
pub fn clean(reply: &str, max_chars: usize) -> Result<String, String> {
    let mut words = Vec::new();
    let mut in_code = false;
    for line in reply.replace("\r\n", "\n").lines() {
        let t = line.trim();
        if t.starts_with("```") {
            in_code = !in_code;
            continue;
        }
        if in_code {
            continue;
        }
        let t = t.trim_start_matches('#').trim_start();
        let t = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")).or_else(|| t.strip_prefix("• ")).unwrap_or(t);
        let t = match t.split_once(". ") {
            Some((n, rest)) if !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()) => rest,
            _ => t,
        };
        let t = t.replace(['*', '`'], "").replace("__", "");
        words.extend(t.split_whitespace().map(str::to_string));
    }
    let text = words.join(" ");
    if text.is_empty() {
        return Err("[ prof ] pas de réponse du prof — réessaie".into());
    }
    Ok(shorten(&text, max_chars))
}

/// At most `max_chars` characters: whole sentences if they fill at least half, else whole words and `…`.
fn shorten(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let cut: String = text.chars().take(max_chars).collect();
    if let Some(end) = cut.rfind(['.', '!', '?']).filter(|&i| cut[..i].chars().count() >= max_chars / 2) {
        return cut[..=end].to_string();
    }
    let words = cut.rfind(' ').map_or(cut.as_str(), |i| &cut[..i]);
    format!("{}…", words.trim_end_matches([',', ';', ':', ' ']))
}

/// What the offline teacher knows about a `?term`.
#[derive(Debug, Clone, PartialEq)]
pub struct Lookup {
    /// Lines to show.
    pub lines: Vec<String>,
    /// An example to play, when there is one (intervals, chords, scales…).
    pub demo: Option<Demo>,
}

/// `?term` without the AI, in the web's order: a tradition in depth (`raga`,
/// `maqam`…), a scale's character, a theory concept, then the reference table.
/// `None` = unknown: ask the teacher with [`ask`]. (The web's `?spectre SAMPLE`
/// analyses browser samples and isn't ported.)
pub fn lookup(term: &str) -> Option<Lookup> {
    let term = term.trim().trim_start_matches('?').trim().to_lowercase();
    if term.is_empty() {
        return Some(Lookup { lines: vec![HELP.into()], demo: None });
    }
    let plain = normalize(&term);
    if let Some((_, lines)) = TRADITION_LORE.iter().find(|(k, _)| *k == plain) {
        return Some(Lookup { lines: lines.iter().map(|l| l.to_string()).collect(), demo: None });
    }
    let known = SCALE_LORE.iter().chain(LORE).find(|(k, _)| *k == term);
    if let Some((_, text)) = known {
        let demo = demo(&term);
        let listen = if demo.is_some() { "  ♪ (écoute)" } else { "" };
        return Some(Lookup { lines: vec![format!("[ ? ] {term} — {text}{listen}")], demo });
    }
    REFERENCE.iter().find(|r| format!("{} {}", r.syntax, r.section).to_lowercase().contains(&term)).map(|r| Lookup {
        lines: vec![format!("[ ? ] {} — {}", r.syntax, r.explanation)],
        demo: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Instant;

    #[test]
    fn prompts_are_the_webs() {
        let m = explain_message("key: a minor\nlead: 1 3 5\n\n");
        assert!(m.starts_with("En français, en 3 ou 4 phrases simples (pas de code, pas de listes, pas de markdown), explique ce morceau"));
        assert!(m.contains("POURQUOI il sonne ainsi, et un conseil pour l'améliorer."));
        assert!(m.ends_with("CODE:\nkey: a minor\nlead: 1 3 5"));
        assert_eq!(ask_message(" c'est quoi une cadence ? "), "Réponds en français en 2-3 phrases simples (sans listes, sans code) à cette question : c'est quoi une cadence ?");
        assert!(explain_system_prompt().starts_with("Tu es un professeur de musique bienveillant et clair."));
        assert!(ask_system_prompt().starts_with("Tu es un professeur de musique clair et bref.") && ask_system_prompt().contains("LOOM"));
    }

    #[test]
    fn cleaning_gives_plain_prose() {
        let reply = "## Analyse\r\n\r\nCe morceau en **la mineur** sonne *sombre*.\n\n```loom\nlead: 1 3 5\n```\n- Le `kick` pose le pas.\n2. Conseil : ajoute une nappe.\n";
        assert_eq!(clean(reply, 650).unwrap(), "Analyse Ce morceau en la mineur sonne sombre. Le kick pose le pas. Conseil : ajoute une nappe.");
        assert!(clean("  \n```\nkick: x\n```\n", 650).is_err());
        // cut at a sentence end, else at a word with …
        let long = "Première phrase assez longue pour compter. Deuxième phrase qui déborde largement de la limite fixée.";
        assert_eq!(clean(long, 60).unwrap(), "Première phrase assez longue pour compter.");
        assert_eq!(clean("un deux trois quatre cinq six sept huit", 20).unwrap(), "un deux trois…");
        let accents = "é".repeat(700);
        assert!(clean(&accents, 650).unwrap().chars().count() <= 651, "cuts on characters, not bytes");
        assert_eq!(clean("Court.", 650).unwrap(), "Court.");
    }

    #[test]
    fn offline_lookup_like_the_web() {
        let raga = lookup("?raga").unwrap();
        assert_eq!(raga.lines.len(), 4);
        assert!(raga.lines[0].starts_with("[ raga ]"));
        assert!(lookup("maqâm").is_some(), "accents ignored for traditions");
        let dorian = lookup("dorian").unwrap();
        assert!(dorian.lines[0].starts_with("[ ? ] dorian — mineur avec une 6e majeure") && dorian.lines[0].ends_with("♪ (écoute)"));
        assert!(dorian.demo.is_some());
        let quinte = lookup("Quinte").unwrap();
        assert!(quinte.lines[0].contains("ratio 3:2") && quinte.demo.is_some());
        let tension = lookup("tension").unwrap();
        assert!(tension.demo.is_none() && !tension.lines[0].contains("écoute"));
        let r = lookup("mutate").unwrap();
        assert!(r.lines[0].starts_with("[ ? ] ") && r.lines[0].contains("mutate"), "{:?}", r.lines);
        assert_eq!(lookup("").unwrap().lines, [HELP]);
        assert!(lookup("comment rendre ça plus triste").is_none(), "→ ask the AI");
    }

    fn fake(script: &str) -> Command {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", script]);
        cmd
    }

    fn wait_for(job: &mut Job) -> Result<String, String> {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(result) = job.poll() {
                return result;
            }
            assert!(Instant::now() < deadline, "job never finished");
            thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn jobs_run_a_fake_teacher() {
        let mut job = explain_with(
            fake("grep -q 'CODE:' && printf '**Ambiance** : sombre.\\n\\n- En la mineur.\\n'"),
            "key: a minor\nlead: 1 3 5\n",
            Duration::from_secs(10),
        );
        assert_eq!(wait_for(&mut job).unwrap(), "Ambiance : sombre. En la mineur.");
        let mut job = ask_with(fake("grep -q 'cadence' && echo 'Une cadence termine une phrase.'"), "cadence", Duration::from_secs(10));
        assert_eq!(wait_for(&mut job).unwrap(), "Une cadence termine une phrase.");
        let mut empty = explain_with(fake("echo jamais"), "  \n", Duration::from_secs(10));
        assert_eq!(wait_for(&mut empty).unwrap_err(), NEED_A_SONG);
        let mut cancelled = ask_with(fake("sleep 30"), "x", Duration::from_secs(60));
        cancelled.cancel();
        assert_eq!(wait_for(&mut cancelled).unwrap_err(), "le prof annulé");
        let mut silent = ask_with(fake("cat >/dev/null; echo"), "x", Duration::from_secs(10));
        assert!(wait_for(&mut silent).unwrap_err().contains("pas de réponse"));
    }

    #[test]
    fn real_commands_use_the_teacher_prompt() {
        let cmd = claude_command_with("claude-opus-5", &explain_system_prompt());
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
        assert!(args.windows(2).any(|w| w[0] == "--system-prompt" && w[1].contains("professeur de musique")));
        assert!(!args.iter().any(|a| a.contains("composition engine")));
    }
}
