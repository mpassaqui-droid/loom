//! VIBE — describe the music in plain words, Claude writes the LOOM code.
//!
//! Port of the browser's VIBE box (`web/index.html`, `vibe()`), for the native
//! apps. Instead of calling the API with a key, it runs the Claude Code CLI in
//! print mode (`claude -p`), which uses the owner's Claude subscription.
//!
//! The flow, same as the web:
//! 1. [`user_message`] packs the request and the current code,
//! 2. [`VibeJob::start`] asks Claude in a background thread (timeout, cancel),
//! 3. [`answer`] pulls the code out of the reply ([`extract_code`]), checks it
//!    with the real parser ([`validate`]) and lists what changed per voice
//!    ([`diff`]). Broken code is never applied.

use std::io::{Read, Write};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use loom_core::Program;

/// The model used when `LOOM_VIBE_MODEL` is not set.
pub const DEFAULT_MODEL: &str = "claude-fable-5-1";
/// Environment variable to pick another model (`claude-sonnet-5`, `claude-fable-5`…).
pub const MODEL_ENV: &str = "LOOM_VIBE_MODEL";
/// How long Claude may think before we give up and kill the process.
pub const TIMEOUT: Duration = Duration::from_secs(120);

/// Les puces de VIBE, comme les boutons du web : (étiquette, demande envoyée à Claude).
/// Dans le terminal : F2 puis le chiffre (1..9, 0 pour la dixième).
pub const CHIPS: [(&str, &str); 10] = [
    ("+ batterie", "ajoute une batterie qui groove"),
    ("+ basse", "ajoute une basse qui suit l'harmonie"),
    ("+ nappe", "ajoute une nappe douce en fond"),
    ("+ triste", "rends ce morceau plus triste et mélancolique"),
    ("+ énergie", "rends ce morceau plus énergique et rapide"),
    ("+ exotique", "rends ce morceau plus exotique et oriental"),
    ("varier", "crée une variation de ce morceau en gardant l'esprit"),
    ("simplifier", "simplifie ce morceau, enlève le superflu"),
    ("continuer", "continue et développe ce morceau avec une suite cohérente (nouvelle section)"),
    ("corriger", "corrige les erreurs et améliore ce morceau pour qu'il sonne bien"),
];

/// La puce d'une touche chiffre : '1' = la première … '0' = la dixième.
pub fn chip_for_key(c: char) -> Option<&'static str> {
    let i = match c { '1'..='9' => c as usize - '1' as usize, '0' => 9, _ => return None };
    CHIPS.get(i).map(|(_, ask)| *ask)
}

const MISSING_CLI: &str = "la commande « claude » est introuvable — installe Claude Code (mise use -g claude), \
lance « claude » une fois pour te connecter avec ton abonnement, puis réessaie";
#[cfg(test)]
const CANCELLED: &str = "VIBE annulé";

/// The model to ask: `LOOM_VIBE_MODEL` if set, else [`DEFAULT_MODEL`].
pub fn model() -> String {
    std::env::var(MODEL_ENV)
        .ok()
        .map(|m| m.trim().to_string())
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| DEFAULT_MODEL.to_string())
}

/// The message sent to Claude: the current code, then the request.
pub fn user_message(words: &str, code: &str) -> String {
    let code = if code.trim().is_empty() { "(vide — nouveau morceau)" } else { code.trim_end() };
    format!(
        "Current LOOM code:\n{code}\n\nRequest: {}\n\nReturn the full updated LOOM program. Code only.",
        words.trim()
    )
}

/// `claude -p` with the LOOM system prompt, no tools, text output.
/// The prompt itself goes on stdin. `ANTHROPIC_API_KEY` is removed from the
/// child's environment so it always uses the subscription, never the API.
pub fn claude_command(model: &str) -> Command {
    claude_command_with(model, SYSTEM_PROMPT)
}

/// [`claude_command`] with another system prompt (the teacher in `prof` uses it).
pub fn claude_command_with(model: &str, system_prompt: &str) -> Command {
    let mut cmd = Command::new("claude");
    cmd.args(["-p", "--model", model, "--system-prompt", system_prompt])
        .args(["--tools", "", "--output-format", "text", "--no-session-persistence", "--safe-mode"])
        .env_remove("ANTHROPIC_API_KEY")
        // away from any project folder, so no CLAUDE.md sneaks into the context
        .current_dir(std::env::temp_dir());
    cmd
}

// ------------------------------------------------------------------ the job

/// What VIBE brings back once the code has been checked.
#[derive(Debug, Clone, PartialEq)]
pub struct VibeAnswer {
    /// The new program, ready to put in the editor.
    pub code: String,
    /// Voices added, removed or changed compared with the old code.
    pub changes: Vec<Change>,
    /// A one-line version of `changes`, e.g. `+lead ~bass −hat`.
    pub summary: String,
    /// How many voices the new program has.
    pub voices: usize,
}

/// One VIBE request running in the background. Dropping it cancels it.
pub struct VibeJob {
    /// The user's words, to show while it thinks.
    pub words: String,
    job: ClaudeJob<VibeAnswer>,
}

impl VibeJob {
    /// Ask Claude (the real CLI, model from [`model`]) to apply `words` to `code`.
    pub fn start(words: &str, code: &str) -> VibeJob {
        VibeJob::start_with(claude_command(&model()), words, code, TIMEOUT)
    }

    /// Same as [`VibeJob::start`] with any command: it gets the message on
    /// stdin and must print the reply on stdout. Lets tests fake Claude.
    pub fn start_with(command: Command, words: &str, code: &str, timeout: Duration) -> VibeJob {
        let old_code = code.to_string();
        let job = ClaudeJob::spawn("VIBE", command, user_message(words, code), timeout, move |r| answer(&r, &old_code));
        VibeJob { words: words.trim().to_string(), job }
    }

    /// `None` while Claude is thinking; the result once, when it's done.
    pub fn poll(&mut self) -> Option<Result<VibeAnswer, String>> {
        self.job.poll()
    }

    /// Stop waiting and kill the `claude` process.
    pub fn cancel(&self) {
        self.job.cancel()
    }

    /// Time since the request was sent (for the "réfléchit… 12 s" display).
    pub fn elapsed(&self) -> Duration {
        self.job.elapsed()
    }
}

/// Any request to the Claude CLI running in a background thread: the reply is
/// turned into a `T` by a `finish` step (VIBE checks code, the teacher cleans
/// prose). Timeout, cancel and "CLI missing" errors are handled here once.
/// Dropping the job cancels it.
pub struct ClaudeJob<T> {
    label: &'static str,
    started: Instant,
    cancel: Arc<AtomicBool>,
    result: Receiver<Result<T, String>>,
    done: bool,
}

impl<T: Send + 'static> ClaudeJob<T> {
    /// Run `command` with `input` on stdin, then `finish` on its stdout.
    /// `label` names the feature in error messages (« VIBE annulé »).
    pub fn spawn(
        label: &'static str,
        command: Command,
        input: String,
        timeout: Duration,
        finish: impl FnOnce(String) -> Result<T, String> + Send + 'static,
    ) -> ClaudeJob<T> {
        let cancel = Arc::new(AtomicBool::new(false));
        let (tx, result) = mpsc::channel();
        let flag = cancel.clone();
        thread::spawn(move || {
            let reply = run_command(label, command, input, timeout, &flag);
            let _ = tx.send(reply.and_then(finish));
        });
        ClaudeJob { label, started: Instant::now(), cancel, result, done: false }
    }

    /// A job that is already over with this error (nothing is run) — for a
    /// request refused before asking Claude, e.g. an empty song.
    pub fn failed(label: &'static str, error: String) -> ClaudeJob<T> {
        let (tx, result) = mpsc::channel();
        let _ = tx.send(Err(error));
        ClaudeJob { label, started: Instant::now(), cancel: Arc::new(AtomicBool::new(false)), result, done: false }
    }

    /// `None` while Claude is thinking; the result once, when it's done.
    pub fn poll(&mut self) -> Option<Result<T, String>> {
        if self.done {
            return None;
        }
        match self.result.try_recv() {
            Ok(result) => {
                self.done = true;
                Some(result)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.done = true;
                Some(Err(format!("{} s'est interrompu — réessaie", self.label)))
            }
        }
    }

    /// Stop waiting and kill the `claude` process.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// Time since the request was sent (for the "réfléchit… 12 s" display).
    pub fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }
}

impl<T> Drop for ClaudeJob<T> {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

/// Run the command with `input` on stdin; its stdout on success.
fn run_command(label: &str, mut command: Command, input: String, timeout: Duration, cancel: &AtomicBool) -> Result<String, String> {
    command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => MISSING_CLI.to_string(),
        _ => format!("impossible de lancer claude : {e}"),
    })?;
    if let Some(mut stdin) = child.stdin.take() {
        // in its own thread: a big prompt must not block us if the child is slow to read
        thread::spawn(move || {
            let _ = stdin.write_all(input.as_bytes());
        });
    }
    let stdout = read_in_background(child.stdout.take());
    let stderr = read_in_background(child.stderr.take());
    let started = Instant::now();
    let status = wait(&mut child, || {
        if cancel.load(Ordering::Relaxed) {
            Some(format!("{label} annulé"))
        } else if started.elapsed() > timeout {
            Some(format!("trop lent (plus de {} s) — réessaie", timeout.as_secs()))
        } else {
            None
        }
    })?;
    let out = stdout.recv().unwrap_or_default();
    let err = stderr.recv().unwrap_or_default();
    if !status.success() {
        let detail = first_line(&out).or_else(|| first_line(&err)).unwrap_or("sans message");
        return Err(format!("claude a échoué : {detail}"));
    }
    Ok(out)
}

/// Wait for the child; kill it as soon as `stop` gives a reason.
fn wait(child: &mut Child, stop: impl Fn() -> Option<String>) -> Result<ExitStatus, String> {
    loop {
        if let Some(reason) = stop() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(reason);
        }
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => thread::sleep(Duration::from_millis(50)),
            Err(e) => return Err(format!("claude : {e}")),
        }
    }
}

/// Read a pipe to the end in another thread, so a full pipe never blocks the child.
fn read_in_background(pipe: Option<impl Read + Send + 'static>) -> Receiver<String> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(mut pipe) = pipe {
            let _ = pipe.read_to_end(&mut bytes);
        }
        let _ = tx.send(String::from_utf8_lossy(&bytes).into_owned());
    });
    rx
}

fn first_line(s: &str) -> Option<&str> {
    s.lines().map(str::trim).find(|l| !l.is_empty())
}

// ------------------------------------------------------ reply → checked code

/// Turn Claude's reply into a checked answer, or say why nothing changes.
pub fn answer(reply: &str, old_code: &str) -> Result<VibeAnswer, String> {
    let code = extract_code(reply);
    let program = validate(&code)?;
    let changes = diff(old_code, &code);
    let summary = if old_code.trim() == code.trim() { "aucun changement".to_string() } else { summary(&changes) };
    Ok(VibeAnswer { code, changes, summary, voices: program.voices.len() })
}

/// The LOOM code in a reply: the (longest) fenced ``` block if there is one,
/// else the whole answer. Stray prose lines before and after are dropped.
pub fn extract_code(reply: &str) -> String {
    let reply = reply.replace("\r\n", "\n");
    let mut blocks: Vec<Vec<&str>> = Vec::new();
    let mut inside = false;
    for line in reply.lines() {
        if line.trim_start().starts_with("```") {
            if !inside {
                blocks.push(Vec::new());
            }
            inside = !inside;
        } else if inside {
            blocks.last_mut().expect("a block was opened").push(line);
        }
    }
    let lines: Vec<&str> = match blocks.into_iter().max_by_key(|b| b.iter().map(|l| l.len()).sum::<usize>()) {
        Some(block) => block,
        None => reply.lines().collect(),
    };
    let is_code = |l: &&str| !l.trim().is_empty() && is_loom_line(l);
    let (Some(first), Some(last)) = (lines.iter().position(is_code), lines.iter().rposition(is_code)) else {
        return String::new();
    };
    let mut code: String = lines[first..=last].iter().map(|l| l.trim_end()).collect::<Vec<_>>().join("\n");
    code.push('\n');
    code
}

/// Check code before it's applied: every line must look like LOOM, and the
/// parser must find at least one voice.
pub fn validate(code: &str) -> Result<Program, String> {
    if code.trim().is_empty() {
        return Err("l'IA n'a pas renvoyé de code — rien n'a changé, réessaie".into());
    }
    if let Some(bad) = code.lines().find(|l| !is_loom_line(l)) {
        let bad: String = bad.trim().chars().take(50).collect();
        return Err(format!("l'IA a renvoyé autre chose que du code LOOM (« {bad} ») — rien n'a changé, réessaie"));
    }
    let program = Program::parse(code);
    if program.voices.is_empty() {
        return Err("l'IA a renvoyé du code sans aucune voix — rien n'a changé, réessaie".into());
    }
    Ok(program)
}

/// A line of LOOM: blank, a comment, or `label: …` where the label is a name,
/// `name@section`, or `def name`. Prose ("Voici le morceau :") is not.
fn is_loom_line(line: &str) -> bool {
    let code = strip_comment(line).trim();
    if code.is_empty() {
        return true;
    }
    let Some((label, _)) = code.split_once(':') else { return false };
    let label = label.trim();
    let label = label.strip_prefix("def ").map_or(label, str::trim);
    let is_word = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-');
    match label.split_once('@') {
        Some((name, section)) => is_word(name) && is_word(section),
        None => is_word(label),
    }
}

/// Drop a `#` comment (at the start, or after a space — `f#` stays a note).
pub(crate) fn strip_comment(line: &str) -> &str {
    let mut prev = ' ';
    for (i, c) in line.char_indices() {
        if c == '#' && prev.is_whitespace() {
            return &line[..i];
        }
        prev = c;
    }
    line
}

// ------------------------------------------------------------ per-voice diff

/// What happened to one voice line between the old and the new code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    Added(String),
    Removed(String),
    Changed(String),
}

impl std::fmt::Display for Change {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Change::Added(name) => write!(f, "+{name}"),
            Change::Removed(name) => write!(f, "−{name}"),
            Change::Changed(name) => write!(f, "~{name}"),
        }
    }
}

/// Settings lines (not voices), left out of the diff — the web's list.
const SETTINGS: [&str; 22] = [
    "key", "scale", "seed", "song", "arrange", "duck", "feel", "scene", "prog", "tune", "bpm", "tempo", "meter",
    "accel", "groove", "loop", "warmth", "film", "title", "bus", "room", "morph",
];

/// Is this label (lowercase, without `@section`) a setting like `key` or `bpm`, not a voice?
pub(crate) fn is_setting(label: &str) -> bool {
    SETTINGS.contains(&label)
}

/// Voices added, changed, then removed, line by line (`bass@drop` counts as
/// its own voice). Only spacing or comments changing is not a change.
pub fn diff(old: &str, new: &str) -> Vec<Change> {
    let (before, after) = (voice_lines(old), voice_lines(new));
    let find = |lines: &[(String, String)], name: &str| lines.iter().find(|(n, _)| n == name).map(|(_, l)| l.clone());
    let mut changes = Vec::new();
    for (name, line) in &after {
        match find(&before, name) {
            None => changes.push(Change::Added(name.clone())),
            Some(old_line) if old_line != *line => changes.push(Change::Changed(name.clone())),
            Some(_) => {}
        }
    }
    for (name, _) in &before {
        if find(&after, name).is_none() {
            changes.push(Change::Removed(name.clone()));
        }
    }
    changes
}

/// `(label, line with spacing normalised)` for each voice line; a repeated label keeps its last line.
fn voice_lines(code: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for raw in code.lines() {
        let line = strip_comment(raw).trim();
        let Some((label, _)) = line.split_once(':') else { continue };
        let label = label.trim().to_lowercase();
        if label.is_empty() || SETTINGS.contains(&label.split('@').next().unwrap_or("")) {
            continue;
        }
        let line = line.split_whitespace().collect::<Vec<_>>().join(" ");
        match out.iter_mut().find(|(n, _)| *n == label) {
            Some(entry) => entry.1 = line,
            None => out.push((label, line)),
        }
    }
    out
}

/// `+lead ~bass −hat` (at most six, then `…`); `réglages` when only settings changed.
pub fn summary(changes: &[Change]) -> String {
    if changes.is_empty() {
        return "réglages".into();
    }
    let mut text = changes.iter().take(6).map(Change::to_string).collect::<Vec<_>>().join(" ");
    if changes.len() > 6 {
        text.push_str(" …");
    }
    text
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)] // Keep the long composition prompt at the end of the file.
mod tests {

    #[test]
    fn puces_comme_le_web() {
        assert_eq!(chip_for_key('1'), Some("ajoute une batterie qui groove"));
        assert_eq!(chip_for_key('0'), Some(CHIPS[9].1));
        assert_eq!(chip_for_key('a'), None);
        assert!(CHIPS.iter().all(|(l, a)| !l.is_empty() && a.len() > 10));
    }

    use super::*;

    const OLD: &str = "bpm: 120\nkick: x . x .\nhat: x x x x\nbass: c2 . eb2 .\n";

    #[test]
    fn message_holds_code_and_request() {
        let m = user_message("  plus sombre ", "kick: x . x .\n");
        assert!(m.starts_with("Current LOOM code:\nkick: x . x .\n\nRequest: plus sombre\n"), "{m}");
        assert!(user_message("un beat", "").contains("(vide"));
    }

    #[test]
    fn system_prompt_is_a_faithful_port() {
        for must in ["composition engine for LOOM", "GRAMMAR:", "WORLD TRADITIONS", "e(k,n)", "prog: auto", "END EXAMPLE"] {
            assert!(SYSTEM_PROMPT.contains(must), "prompt lost « {must} »");
        }
        // browser-only bits are gone
        assert!(!SYSTEM_PROMPT.contains("WebGL") && !SYSTEM_PROMPT.contains("SWING knob"));
    }

    #[test]
    fn model_can_be_overridden() {
        // no other test touches this variable
        std::env::set_var(MODEL_ENV, " claude-sonnet-5 ");
        assert_eq!(model(), "claude-sonnet-5");
        std::env::set_var(MODEL_ENV, "");
        assert_eq!(model(), DEFAULT_MODEL);
        std::env::remove_var(MODEL_ENV);
    }

    #[test]
    fn command_uses_subscription_and_no_tools() {
        let cmd = claude_command("claude-opus-5");
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
        assert_eq!(cmd.get_program(), "claude");
        assert!(args.windows(2).any(|w| w == ["--model", "claude-opus-5"]));
        assert!(args.windows(2).any(|w| w == ["--tools", ""]));
        assert!(args.contains(&"-p".to_string()));
        assert!(cmd.get_envs().any(|(k, v)| k == "ANTHROPIC_API_KEY" && v.is_none()), "key must be removed");
    }

    #[test]
    fn extracts_plain_fenced_and_chatty_replies() {
        assert_eq!(extract_code("kick: x . x .\nhat: x x\n"), "kick: x . x .\nhat: x x\n");
        let fenced = "Voici :\n```loom\n# groove\nkick: x . x .\n```\nBonne écoute !";
        assert_eq!(extract_code(fenced), "# groove\nkick: x . x .\n");
        let chatty = "Voici un groove plus sombre :\n\nkey: c phrygian\nbass: 1 . 2b . | saw\n\nJ'ai baissé la basse.";
        assert_eq!(extract_code(chatty), "key: c phrygian\nbass: 1 . 2b . | saw\n");
        // the longest block wins over a tiny example
        let two = "```\nkick: x\n```\n```\nkick: x . x .\nhat: x x x x\n```";
        assert_eq!(extract_code(two), "kick: x . x .\nhat: x x x x\n");
        assert_eq!(extract_code("Désolé, je ne peux pas."), "");
    }

    #[test]
    fn validation_rejects_garbage() {
        assert!(validate("kick: x . x .\nkey: c minor").is_ok());
        assert!(validate("").is_err());
        assert!(validate("bpm: 120\nkey: c minor\n").unwrap_err().contains("aucune voix"));
        assert!(validate("kick: x . x .\nPuis on ajoute une basse sur le refrain").unwrap_err().contains("autre chose"));
        assert!(validate("kick: x . x .\nVoici le code : bass").is_err());
        // notes with # and section / macro labels are fine
        assert!(validate("key: f# dorian\ndef riff: 1 2 3\nlead@drop: c#3 $riff # thème\nlead: f#4").is_ok());
    }

    #[test]
    fn every_shipped_song_passes_validation() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let mut checked = 0;
        for entry in std::fs::read_dir(root).unwrap().flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "loom") {
                let text = std::fs::read_to_string(&path).unwrap();
                if let Err(e) = validate(&text) {
                    panic!("{} : {e}", path.display());
                }
                checked += 1;
            }
        }
        assert!(checked > 0);
    }

    #[test]
    fn diff_per_voice() {
        let new = "bpm: 90\nkick: x . x .  # plus lourd\nhat: x . x .\nlead: 1 3 5\nlead@drop: 5 3 1\n";
        let changes = diff(OLD, new);
        assert_eq!(changes, [
            Change::Changed("hat".into()),
            Change::Added("lead".into()),
            Change::Added("lead@drop".into()),
            Change::Removed("bass".into()),
        ]);
        assert_eq!(summary(&changes), "~hat +lead +lead@drop −bass");
        assert_eq!(summary(&diff(OLD, "bpm: 140\nkick: x .  x .\nhat: x x x x\nbass: c2 . eb2 .")), "réglages");
        let many: Vec<Change> = (0..8).map(|i| Change::Added(format!("v{i}"))).collect();
        assert!(summary(&many).ends_with("+v5 …"));
    }

    #[test]
    fn answer_checks_then_summarises() {
        let a = answer("```\nkick: x . x .\nhat: x x x x\nbass: c2 . eb2 . | saw\n```", OLD).unwrap();
        assert_eq!((a.voices, a.summary.as_str()), (3, "~bass"));
        assert_eq!(answer(OLD, OLD).unwrap().summary, "aucun changement");
        assert!(answer("Je n'ai pas compris.", OLD).is_err());
    }

    fn fake(script: &str) -> Command {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", script]);
        cmd
    }

    fn wait_for(job: &mut VibeJob) -> Result<VibeAnswer, String> {
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
    fn job_runs_a_fake_claude() {
        // the fake reads the message (to prove it got it) and answers with code
        let mut job = VibeJob::start_with(
            fake("grep -q 'Request: plus de hats' && printf 'kick: x . x .\\nhat: x x x x x x x x\\n'"),
            "plus de hats", OLD, Duration::from_secs(10),
        );
        let a = wait_for(&mut job).unwrap();
        assert_eq!(a.code, "kick: x . x .\nhat: x x x x x x x x\n");
        assert_eq!(a.summary, "~hat −bass");
        assert!(job.poll().is_none(), "the result comes only once");
    }

    #[test]
    fn job_reports_failures_timeouts_and_cancel() {
        let mut failing = VibeJob::start_with(fake("cat >/dev/null; echo 'Not logged in'; exit 1"), "x", OLD, Duration::from_secs(10));
        assert_eq!(wait_for(&mut failing).unwrap_err(), "claude a échoué : Not logged in");

        let started = Instant::now();
        let mut slow = VibeJob::start_with(fake("sleep 30"), "x", OLD, Duration::from_millis(200));
        assert!(wait_for(&mut slow).unwrap_err().contains("trop lent"));
        assert!(started.elapsed() < Duration::from_secs(5), "the child was killed");

        let mut cancelled = VibeJob::start_with(fake("sleep 30"), "x", OLD, Duration::from_secs(60));
        cancelled.cancel();
        assert_eq!(wait_for(&mut cancelled).unwrap_err(), CANCELLED);

        let mut missing = VibeJob::start_with(Command::new("loom-no-such-claude"), "x", OLD, Duration::from_secs(1));
        assert!(wait_for(&mut missing).unwrap_err().contains("introuvable"));
    }

    /// Real end-to-end call through the CLI (uses the subscription, takes a while):
    /// `cargo test -p loom-app live_claude -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn live_claude_writes_valid_loom() {
        let mut job = VibeJob::start("un beat techno simple", "");
        let result = loop {
            if let Some(result) = job.poll() {
                break result;
            }
            thread::sleep(Duration::from_millis(200));
        };
        println!("modèle : {} · {:.1} s", model(), job.elapsed().as_secs_f32());
        let a = result.expect("VIBE failed");
        println!("{} voix · {}\n{}", a.voices, a.summary, a.code);
        assert!(a.voices > 0);
    }
}

/// The system prompt: a port of the browser's `SYS` (language reference and
/// rules). Only the browser-only parts changed: visual scenes, knobs/panels,
/// and a note that this comes from the native app and edits the current code.
pub const SYSTEM_PROMPT: &str = r#"You are the composition engine for LOOM, a live-coding music language.
Output ONLY valid LOOM code. No explanations, no markdown fences, no backticks.
GRAMMAR:
- One voice per line: name: pattern | verb | verb
- Drum voices use x (hit) and . (rest). Drum names: kick, snare, hat, clap.
- Melodic voices use any other name (bass, lead, arp, pad). Tokens are note names like c2 eb2 g3 a4 (a-g, optional # or b, octave 1-5). . means rest.
- OPTIONAL key: declare a tonality once, e.g. 'key: c minor' or 'key: f# dorian' (scales: major minor dorian phrygian lydian mixolydian locrian pentatonic minpent harmonic_minor melodic_minor whole_tone blues blues_major chromatic octatonic augmented bebop bebop_major phrygian_dominant hungarian_minor hungarian_major neapolitan_minor neapolitan_major enigmatic persian prometheus kumoi yo, plus world: hijaz hijazkar nahawand kurd ajam bhairav yaman todi marwa purvi slendro pelog hirajoshi insen iwato egyptian; microtonal maqam with true quarter tones: bayati rast saba sikah huzam jiharkah; plus overtone/acoustic/lydian_dominant, altered/super_locrian, lydian_augmented, messiaen3, messiaen4, ritusen, chinese). You can also bend any note by cents with +N or -N, e.g. 3+50 c3-30 (great for blue notes or maqam inflections). Then melodic tokens may be SCALE DEGREES 1-7 instead of note names: 1=tonic, and the scale supplies the right accidentals (in c minor, 3 is eb, 7 is bb). Octave marks: ' raises an octave, , lowers it (e.g. 1' 5,). Chromatic alteration goes AFTER the digit: 7# raises a semitone (leading tone in minor), 3b 5b lower one (blue notes), 4# (lydian). Never put #/b BEFORE the digit. Prefer degrees over note names when a key is set — it keeps everything in key. TUNING: add tune: just (5-limit just intonation) or tune: pyth (Pythagorean) to micro-tune the scale degrees to pure ratios for a more consonant, in-tune sound — gorgeous on sustained pads/drones/harmonies; tune: equal (default) is 12-TET. You can also use N-tone equal temperament with tune: 19edo, tune: 31edo, tune: 24edo (quarter-tones), etc. — the octave split into N equal steps for exotic xenharmonic scales (Wendy Carlos/Easley Blackwood territory).
WORLD TRADITIONS — honour them, don't just pick a scale name. RAGA (Indian): always lay a held drone on degree 1 (drone: 1 . . . | bow|sine | gate 4), use a few signature notes, ornament with slide/bend (meend); match the mood/time (bhairav=grave dawn, yaman=serene evening, malkauns=deep night). MAQAM (Arabic/Turkish): use tune: maqam scales (bayati/rast/saba/sikah) or quarter-tone numeric scales for the neutral 2nd/3rd; add a drone and ornaments; hijaz = the augmented-2nd 'desert' colour. GAMELAN (Indonesian): key: slendro/pelog with bell/modal timbres, slight detune for shimmering beats (ombak), cyclic layered patterns. CHINESE: pentatonic/chinese with pluck (guzheng) and reed (dizi). PENTATONIC/BLUES: degrees 1 2 3 5 6, blue notes via bend or 3b/5b. Choose the scale for the FEELING you want, and shape the arrangement (drone, ornaments, register) to fit the tradition — the goal is authentic emotion, not exotic decoration.
AVOID REPETITION — build, don't loop. A bar that repeats unchanged for 8 bars is boring. Use | develop on melodic/lead voices so the material EVOLVES bar by bar (a motivic arc: displacement, sequence up/down, retrograde, inversion) instead of repeating. Use generators (walk/drunk/xys/evolve/ca/logistic) for ever-changing lines, per-section variants (name@section:) so sections contrast and recap differently, | every N rot/mutate/shuffle for evolving patterns, and vary the DRUMS across sections (don't reuse the same kick/snare grid everywhere). Aim for long-form construction: intro/build/contrast/climax, tension that rises and resolves — not one 2-bar loop repeated.
RHYTHM GENERATORS — use these for non-obvious patterns. World timelines (bare names): tresillo, cinquillo, son, rumba, bossa, bembe, baiao, maqsum, ayoub, samba, montuno, shiko — e.g. kick: tresillo, hat: bembe. Xenakis SIEVE: sieve(N,M:R,M:R,…) = N steps with a hit where any (i mod M == R) — number-theory rhythms, complex and non-periodic, e.g. perc: sieve(16,3:0,5:2). 1/f PINK noise melody: pink(n) (or 1f(n)) = a fractal, 'natural' melodic contour (the 1/f statistic found in nature and real music) — e.g. lead: pink(16) | sine. SHAPED CONTOURS (constraint composition): arc(n) rises to a peak then resolves back on the tonic, rise(n) ascends, fall(n) descends to the tonic — e.g. lead: arc(8) gives a built melodic arch. SPECTRAL HARMONY (Grisey/Murail): spectrum(n,fund) = the first n harmonics of a fundamental note (default c2) as notes — an overtone arpeggio, e.g. pad: spectrum(10,c1) | bell for a shimmering spectral chord. COMPOSITION ALGORITHMIQUE: l-sys(n,axiome,1>1.2,5>3.1) = un L-système sur MESURE (règles déterministes), xys(n,racine,1>2:2|3:1) = marche de Markov stochastique à la XENAKIS (chaque note tire sa transition), evolve(n) = une MÉLODIE GÉNÉTIQUE (population, fitness contour/tonique/variété, élite+crossover+mutation — seedée, évolue chaque mesure). Plus les math générateurs ca(rule,n), e(k,n) euclid, morse(n), fib(n), et le seedé walk(n)/drunk(n)/logistic(n)/perlin(n). Reach for these instead of writing the same x . . x grid.
- Tokens separated by single spaces; they fill ONE bar evenly (4=quarters,8=eighths,16=sixteenths).
- Verbs after | : rev, fast N, slow N, every N <verb>, palindrome (forwards then backwards each bar), rot N (rotate the pattern by N steps), iter N (shift one step further each bar, looping after N bars), chunk N <verb> (split the pattern into N chunks and apply <verb> to a DIFFERENT chunk each bar — evolving variation, Tidal-style, e.g. hat: x x x x x x x x | chunk 4 fast 2), sometimes <verb> / often <verb> / rarely <verb> / someby P <verb> (apply a verb on a random fraction of bars, seeded), jux <verb> (Tidal-style stereo split: the original plays hard-left and a <verb>-transformed copy plays hard-right — e.g. | jux rev for a reversed echo in the other ear, | jux rot 3, | jux fast 2; great on leads/arps/hats for width), off N <verb> (layer a quieter copy shifted N steps later — a canon/echo; the <verb> is optional, e.g. | off 3 (a delayed copy 3 steps later) or | off 4 rev. jux and off can be combined), add N (diatonic transpose: shift every scale-degree token by N steps in the current scale — the KILLER combo is harmonising a layer, e.g. | jux add 2 puts a third above in the right ear, | off 3 add 4 makes a canon a fifth up 3 steps later, | every 2 add 7 jumps an octave every other bar; use negative N to go down). Timbre after | : saw, sine, tri, pulse, square. Also gain N (0-1), gate N (note length 0.05-4: gate 0.3 = short/staccato/plucky, gate 1.6 = long/legato/sustained pads — shape articulation per voice), slide N (portamento: each note glides in pitch from the previous one over N seconds, default 0.06 — the TB-303 acid-bass sound; put it on a mono bassline or lead, e.g. bass: 1 . 1' . 5, . 1 . | saw | slide | lp 700), orbit RATE (spatialisation — the voice circles the listener in stereo at RATE Hz with a distance gain wobble, e.g. pad: ^ . . . | saw | orbit 0.3; lovely on pads/FX), ring FREQ (ring modulation in Hz — multiplies the voice by a sine for clangy/metallic/robotic or inharmonic-bell timbres; try | ring 220 on a lead, | ring 80 for a gnarly bass, 600+ for bell/FX), am FREQ (amplitude modulation at audio rate — adds sidebands like ring but unipolar/softer; subtle FREQ 4-30 = tremolo, higher = metallic), sub N (adds a sine sub-oscillator one octave below at level N 0-1 — instant weight/body; put | sub 0.6 on a bass or kick-bass for a deep bottom end), double N (an octave-UP doubling layer 0-1 for presence/brightness on leads/plucks/keys), noise N (mixes in a filtered white-noise layer 0-1 for air/breath/texture — on snares/claps for body, pads for a windy bed, or hats; e.g. snare: . . x . | sample sd | noise 0.3), wow N (vintage tape/vinyl wow-and-flutter — a slow pitch wobble, 0-1; lovely lo-fi character on keys/pads/mellotron sounds), vowel a|e|i|o|u (formant filter: shapes the voice into a sung vowel for choir/vocal-synth/talk-box timbres — gorgeous on a saw lead or pad, e.g. pad: ^ . . . | saw | unison 4 | vowel a), vocode SAMPLE (a real channel VOCODER — splits this voice (carrier, use a bright saw) and a modulator sample into 12 bands so the synth TALKS with the sample's formants; modulator samples: vox yeah breath. e.g. robot: 1 . 3 . 5 . | saw | unison 3 | vocode vox | reverb 0.3), pluck N (Karplus-Strong physical-modelling plucked string — real guitar/harp/koto/kalimba tones; N 0-1 = brightness/sustain, e.g. harp: ^1 ^2 ^3 ^2 | pluck 0.7 | reverb 0.5), bow (bowed-string physical model — sustained Karplus-Strong with friction noise, for violin/cello/organic-drone timbres, e.g. strings: ^ . . . | bow | reverb 0.6 | gate 1.6), reed (woodwind waveguide with inverted reflection -> hollow odd-harmonic clarinet/oboe tone, e.g. wind: 1 . 3 . 5 . | reed | lp 2200 | reverb 0.5), sync N (true oscillator hard-sync rendered to a buffer — the slave saw resets each master period and the ratio sweeps up for the classic screaming sync lead; N = max ratio ~2-6, e.g. lead: 1 . 1' . | sync 4 | lp 3000), bend N [T] (pitch envelope: the note starts N semitones away and glides to pitch over T seconds, default 0.08 — kick drops, 808 glides, tape-stop FX; e.g. kick-bass: 1 . . . | sine | bend -12 0.1 | oct 2), bell (modal synthesis — a bank of inharmonic decaying sine partials = struck metal/bell/gamelan/glass timbres; e.g. bells: ^1 . ^3 . | bell | reverb 0.6), spread N (stereo width for unison voices 0-1 — spreads the detuned oscillators across the stereo field for a huge supersaw; needs unison>1, e.g. lead: 1 3 5 | saw | unison 5 | detune 16 | spread 0.8), fold N (wavefolding 0-1 — Buchla/West-coast timbre that folds the wave on itself for bright, evolving, metallic harmonics; great with a slow LFO or automation, e.g. drone: 1 . . . | sine | fold 0..0.8 | over 8), grain N (granular synthesis 0-1 — sprays the note as a cloud of short randomized grains for shimmering, evolving, textural pads/clouds; higher N = wider pitch/pan scatter, e.g. cloud: 1 . . . | sine | grain 0.6 | reverb 0.7 | gate 2), pwm N (pulse-width modulation 0-1 — two detuned saws form a pulse whose width an LFO sweeps, the classic fat analog PWM lead/bass/string), and oct N to set this voice's default octave for degrees (e.g. bass | oct 2, lead | oct 4) so you don't need , or ' marks.
- Math sequences as rhythm: morse(n) = the Thue-Morse sequence (famously non-repeating, balanced), fib(n) = the Fibonacci word (quasi-periodic, self-similar). e.g. hat: morse(16) , perc: fib(16) | sample perc. They give organic, never-quite-repeating patterns.
- Cellular automata: a token ca(rule,n) generates an evolving rhythm from a Wolfram elementary CA (rule 0-255, n cells) that advances one generation EACH BAR, so the pattern morphs over time on its own. Great rules: 90 (Sierpinski), 30 (chaos), 110, 18, 150, 54. e.g. hat: ca(90,16) , perc: ca(30,16) | sample perc. Best for percussion/texture; combine with a steady kick.
- Euclidean rhythms: a token e(k,n) expands to k hits spread evenly over n steps; e(k,n,rot) rotates. Great for drums, e.g. kick: e(4,16) , hat: e(7,16,2). Mix with plain tokens.
- Tempo-synced echo: | echo DIV [FB] adds a per-voice delay locked to the tempo — DIV taps per bar (4 = quarter notes, 8 = eighths, 6 = dotted, 3 = triplets), FB = feedback 0-0.9. e.g. lead: 1 . . . | pluck | echo 6 0.5 (the dub/IDM rhythmic echo). Different from | delay (the global ping-pong send).
- Macros: 'def riff: 1 2 3 5' defines a reusable fragment; use it as $riff anywhere in a pattern (e.g. lead: $riff $riff | pluck). Great to repeat a motif without retyping.
- walk(n) is a seeded random-walk melody generator over scale degrees (n notes), e.g. lead: walk(16) | pluck — algorithmic, evolves per bar.
- l-sys(n) is a Lindenmayer system (self-similar melody); custom grammar l-sys(n,axiome,sym>prod,…) with .-joined degrees, e.g. lead: l-sys(16,1,1>1.2,2>3.1) | pluck.
- xys(n,racine,sym>a:p|b:q,…) is a XENAKIS Markov walk (weighted transitions, seeded, evolves per bar), e.g. lead: xys(16,1,1>2:2|3:1,2>1:1) | sine.
- evolve(n[,gen][,pop]) breeds a genetic melody (integer fitness, elite+crossover+mutation 20%, seeded, evolves per bar), e.g. lead: evolve(16) | pluck.
- Second LFO: | lfo2 ... is an independent second LFO (same syntax as lfo) so you can modulate two things at once, e.g. | lfo cutoff 2 0.8 square | lfo2 pitch 5 0.2 (wobble filter AND vibrato).
- LFO depth: | lfo TARGET RATE DEPTH [shape] modulates a parameter. TARGET = cutoff, pitch, gain, pan, reso (filter resonance). shape = sine, tri, saw, square (square cutoff = the dubstep wobble). e.g. | lfo cutoff 2 0.7 square , | lfo pan 1 0.9 , | lfo pitch 6 0.3 tri (vibrato).
- Arrangement chance: | chance N (0-1) makes a WHOLE voice play only some bars at random (seeded) — e.g. a fill or a counter-melody that comes and goes. Different from x? (per-note) and degrade (per-hit).
- Trance gate: | tgate N chops a sustained sound into N rhythmic pulses per bar (the pumping gated-pad sound of trance/EDM). e.g. pad: Am7 . . . | saw | unison 3 | tgate 8.
- Harmonize: | harmonize N [level] adds a parallel layer N semitones away (7 = a fifth → power chords, 4 = a major third, 12 = octave, -12 = octave below). e.g. lead: 1 2 3 | harmonize 7 for power-chord stabs, or | harmonize 4 0.4 for sweet parallel thirds.
- prog: auto also has: cinematic, lofi, edm, house, and random (a fresh random diatonic progression each RUN).
- Generative variation: | mutate N evolves a pattern over time — each bar it randomly nudges about N% of the notes (transposes degrees, fills rests), seeded so it's reproducible. | stutter N is a beat-repeat: it loops the first 1/N of the pattern. e.g. lead: 1 2 3 5 | mutate 30 , drums: x . x . | stutter 4.
- Tape macro: | tape N (0-1) is a one-knob lo-fi cassette character — wow-and-flutter + gentle saturation + a touch of hiss. Lovely on keys/pads/whole voices for nostalgia.
- Additive synthesis: | partials A B C ... builds a sound from the amplitude of each harmonic (1 = fundamental, 2 = octave, 3 = fifth above...). e.g. | partials 1 0 0.5 0 0.33 (odd harmonics = hollow/clarinet-ish). | organ 888650000 is a Hammond drawbar shortcut (9 digits 0-8). Pure, clean tones.
- Arp modes: | arp converge (outside-in), diverge (inside-out), thumb (bounce off the lowest note), pinky (bounce off the highest) — on top of up/down/updown/downup. e.g. keys: Cmaj7 . | arp converge | pluck 0.7.
- Ready-made progressions: prog: auto <style> drops in a classic chord progression (resolved in the current key). Styles: pop, pop50s, doowop, jazz, jazz2, sad, minor, epic, andalusian, pachelbel, blues, folk, ballad, gospel. e.g. key: a minor + prog: auto sad. Then voices use ^ to follow it.
- Haas widener: | haas N (0-1) makes a mono voice sound very wide in stereo (a tiny delay on one side — the precedence effect). Great on pads/leads; use sparingly so the mix stays mono-compatible.
- Sampler chops: | chop N beat-slices a loop sample (plays the next 1/N slice each hit — great on | sample amen/break for jungle/IDM). | reverse plays the sample backwards. | begin B | end E play only the region B..E (0-1) of the sample. Combine: break: x x x x | sample amen | chop 8.
- Automation curves: any sweepable param (lp/hp cutoff, drive, pan…) takes a breakpoint curve A..B..C (up to 8 points) with an optional shape per segment: @lin (default) @exp (geometric, best for filters and volume) @log (fast start) @step (hold, then jump) @smooth. The curve spans | over N bars (else the section, else 16 bars) and loops. e.g. pad: ... | lp 200..4000@exp..800 | over 8. Whole-mix automation lines: auto: master 0..1@exp..1..0 over 32 (fade-in, hold, fade-out of the whole mix) and auto: send NAME 0..0.8 over 16 (the wet return of a send: line). Use them for builds, risers, breakdowns and endings.
- Auto-pan: | autopan RATE sweeps a voice left<->right with an LFO (RATE in Hz, e.g. 0.5 = slow sweep). Lovely on pads/arps for movement.
- More world scales: bhairavi malkauns ahirbhairav (ragas), nikriz athar_kurd (maqamat), ryukyu (Okinawa), pelog_selisir (Bali), tritone (Petrushka), harmonic_major, lydian_minor, hindu. Use like key: c bhairavi.
- Groove templates: groove: dilla (also mpc, swing16, push, laidback, humanish) applies a named microtiming feel to the whole track — dilla = the loose, drunk hip-hop swing. One groove per song.
- Phrase dynamics: | cresc makes a voice's notes get LOUDER across the bar, | dim gets softer (a crescendo/diminuendo). e.g. fill: x x x x x x x x | cresc | sample sd.
- Motif verbs: | invert mirrors a melody upside-down around its first note; | sequence N repeats the motif transposed by N scale-steps (a melodic sequence). e.g. lead: 1 2 3 5 | invert , or 1 2 3 | sequence 2 (motif then up a third).
- Historical temperaments: tune: werck (Werckmeister III), tune: kirn (Kirnberger III), tune: meantone (quarter-comma) — Baroque/Renaissance tunings where each key has its own colour. (Plus just, pyth, Nedo.)
- Odd meters: meter: 7/8 sets the bar to N/D beats (7/8, 5/4, 3/4, 12/8…) — changes the bar length, so patterns feel in odd time. meter: 3 = 3 beats. One meter per song.
- Modulation (key change): key@SECTION: c minor makes that song section resolve its degrees AND its ^/prog chords in a different key. e.g. song: verse*4 chorus*4 with key: a minor and key@chorus: c minor -> the chorus lifts to C minor. Great for a key change at the drop/chorus.
- Accelerando/ritardando: accel: 120 160 ramps the tempo from 120 to 160 across the whole piece (accel: 160 90 8 = slow down over 8 bars). For a build-up or a tape-stop ending.
- Live morphing: morph: N (bars) makes live edits CROSSFADE — when a voice's line changes while playing, the old version fades out and the new fades in over N bars (equal-power) instead of switching abruptly. morph: 4 is musical for performance. Omit (or 0) for instant switching.
- Voice-leading: | voicing lead keeps each chord close to the previous one (smooth, no big register jumps) — lovely on a prog: pad. (Also open/drop2/wide for spacing.)
- True N-EDO and Scala: key: c 19edo 0 3 6 8 11 14 17 writes a scale in steps of 19-EDO (degrees land exactly on that grid, and # / b move ONE EDO step, so every one of the 19 pitches is reachable); key: c 24edo alone = all 24 quarter-tone steps. key: d scala 9/8 5/4 4/3 3/2 5/3 15/8 2/1 takes a Scala pitch list (ratios like 5/4, cents with a dot like 386.3, last value = the octave). tune: just|pyth|werck|kirn|meantone|Nedo retunes the degrees of any key. Maqams bayati rast saba sikah huzam jiharkah play their true quarter tones.
- Aroha / avaroha (a raga or melodic minor that goes up one way and down another): key: c khamaj, key: d desh and key: a melodic flatten the 7th (and the 6th for melodic) automatically when the line descends, natural when it climbs. Write your own with a slash: key: c 0 2 4 5 7 9 11 / 0 2 4 5 7 9 10 (ascending / descending). An explicit # or b always wins.
- Custom scales: scale: 0 2 3 5 7 9 10 defines ANY scale as semitone offsets from the tonic — microtonal allowed (scale: 0 1.5 3 5 7 8 10 for quarter-tones). key: c 0 2 3 5 7 9 10 works too. Lets you invent scales beyond the named ones.
- Note ties: a _ token EXTENDS the previous note across that step (sustained/held notes, legato phrasing), e.g. lead: 1 _ _ 3 . 5 _ _ holds 1 for 3 steps then 3, rest, 5 for 2. Different from gate (which scales every note's length).
- Chord voicing: | voicing open|drop2|wide|close re-arranges a chord's notes for a more open/lush spacing, e.g. keys: Cmaj7 . Fmaj7 . | voicing open. Lovely on pads/keys.
- Markov melodies: | markov regenerates a voice's line from its OWN note-to-note transitions (a seeded Markov walk) — same notes, new order that still 'sounds like' the original. e.g. lead: 1 2 3 5 6 5 3 2 | markov. Use | markov N (N>=2) for higher-order chains that follow longer phrases more faithfully (more like the source), e.g. | markov 2.
- SAVANT extras (use sparingly, when the user wants something advanced): tonnetz(n,root,moves) = a neo-Riemannian chord progression that walks the Tonnetz by P/L/R transforms (P=major<->minor, L=leading-tone exchange, R=relative) with smooth voice-leading — post-tonal harmony à la Schubert/Radiohead/film, e.g. harm: tonnetz(8,C,PLR) | saw or pad: tonnetz(6,Am,LR). | hocket = the COMPLEMENT rhythm (hits<->rests): put it on a copy of a drum/perc line so the two voices INTERLOCK and tile the bar (every step hit exactly once) — a tiling canon/hocket, e.g. a: x . . x . . x . / b: x . . x . . x . | hocket. tune: adaptive = adaptive just intonation (chord notes retuned to pure 5-limit ratios) for maximally consonant chords. DAW BUSES: | bus NAME routes a voice into a shared submix bus (e.g. put | bus drums on kick/snare/hat to group them) — one gain controls the whole group. Great for balancing drums vs synths. Configure a bus with a bus: directive: bus: drums gain 0.8 lp 1500 reverb 0.3 (gain, lp/hp shared filter +q, reverb send) — persisted in the code. SPACE REVERB ENGINE: add room: fdn 0.85 4200 for a lush Feedback-Delay-Network hall (Jot — smoother/denser than the default Schroeder); decay 0-0.97, damp in Hz. Per-voice | reverb sends feed it.
- Chords: a token with an UPPERCASE root plays a full chord (several notes at once), e.g. Cm7 F G7 Am Dsus4 Gmaj7. Lowercase stays a single note (c3) so there is no clash. Qualities: 5(power) maj m dim aug sus2 sus4 6 m6 6/9 7 maj7 m7 mmaj7 m7b5 dim7 7b5 7#5 7b9 7#9 7#11 add9 add11 9 maj9 m9 11 m11 13 maj13 m13. Inversions / slash chords with /bass: Cm7/G, C/E, F/A. Use on a pad/keys voice for jazzy harmony.
- Arpeggios: add | arp up (or down, updown, downup) to a chord voice so each chord is played one note at a time across its step, e.g. arp: Am . F . C . G . | arp up | saw. Classic for techno/synthwave sequences.
- Roman-numeral chords (need a key): I ii iii IV V vi vii play the diatonic chord on that scale degree, auto-fitting the key's quality (in a minor key i is minor, etc). Add 7 for a diatonic seventh: I7 ii7 V7. Great for progressions, e.g. with key: c major -> keys: I . . . vi . . . IV . . . V7 . . . | sine. Also arpeggiate them with | arp up. Full theory-class grammar: case against the scale = mode mixture (iv in major, V in minor); chromatic roots bVII bVI bIII and ♯iv° (write ♯ not #, because " #" starts a comment); qualities ° ø + maj7 (vii°7 viiø7 III+ IVmaj7); inversions I6 I64 V65 V43 V42; secondary chords V7/V V7/ii vii°7/V; special chords N N6 It6 Fr6 Ger6. Example: key: c major -> pad: I . vi . ii65 . V7/V . V64 . V7 . I . . . | sine.
- Probability: add ? to a token to make it fire only sometimes. x? ~50%, x?70 = 70%, c3?25 = 25%. Works on any token (drums, notes, degrees, chords). Use it for humanized hats, ghost notes, evolving patterns.
- Microtiming / groove: | shift N nudges a whole voice in time by N steps (-0.5..0.5). Positive lays it BACK (late/relaxed, e.g. snare: ... | shift 0.06 for a laid-back backbeat), negative PUSHES it (early/urgent). Tiny values (0.03-0.1) add human feel; this is how real drummers/players sit a part in the pocket. Try a hat slightly late or a bass slightly early.
- Per-voice swing: | swing N (0-100) delays the off-beat (odd) steps of THAT voice only. The classic move is straight kick + swung hats: hat: x x x x x x x x | swing 58 over kick: x . . . x . . . . Use 54-62 for a subtle shuffle, higher for heavy triplet feel. Great on hats, shakers, rides.
- Degrade (generative space): add | degrade N (N a percent, e.g. 30) to a voice to randomly DROP that fraction of its hits each cycle (seeded, so reproducible). Use it to thin out a busy hat or perc so it breathes and varies, e.g. hat: x x x x x x x x | degrade 30 , or to make a part feel human/loose. Higher = sparser. Different from ?prob (which is per-step in the text) — degrade is one knob for the whole voice.
- Rolls / build-ups: a ratchet *N can ramp in velocity — x*8> is a crescendo roll (8 hits getting LOUDER), x*8< is a decrescendo. Perfect for snare build-ups into a drop, or fills: build@build: . . . . . . . . . . . . x*16> , or snare: . . . . x . . . . . . . x*4> . . . . Combine with samples or synth drums. (Plain *N stays even.)
- Accents (groove): append ! to any hit/note to play it louder (velocity accent); !! is stronger, !!! strongest. Use accents to bring out the backbeat or a syncopation, e.g. kick: x . x! . x . x! . , hat: x x! x x x x! x x , or on a melody note 1 3! 5 1. Combines with ?prob and *ratchet (e.g. x?60! , x!*2). This is the simplest way to stop a pattern sounding flat/robotic — accent 1-2 hits per bar.
- Determinism: add seed: N (e.g. seed: 42) so the random ? choices replay identically every time. Change the seed for a different variation.
- Ratchets/rolls: add *N to a token to repeat it N times within its own step, e.g. snare: . . x*4 . (a 4-hit roll), hat: x*2 x x*3 x . Combine freely: x*4?60 (a roll that fires 60% of the time). Great for build-ups and fills.
- Sound design (after | on melodic voices): lp N = low-pass filter at N Hz (darker), hp N = high-pass (thinner); add q R for resonance (e.g. lp 600 q 8). env A D S R = ADSR envelope in seconds (S is 0-1 sustain level), e.g. pad: ... | env 0.4 0.3 0.7 1.2 for slow swells, or bass: ... | env 0.005 0.08 0 0.05 for a tight pluck.
- Wavetable oscillators: wt NAME picks a richer waveform than saw/sine/etc: organ (drawbar), warm (mellow), rich (bright), soft (round). e.g. keys: ... | wt organ , pad: ... | wt warm. Combine with unison/fm/filter.
- FM synthesis: fm RATIO INDEX gives rich metallic / bell / electric-piano / growling-bass timbres (a modulator at RATIO×pitch with strength INDEX, whose index pings down like a real FM synth). e.g. keys: ... | fm 1 2 (e-piano), bell: ... | fm 3.5 4 | sine (bell), bass: ... | fm 0.5 3 | saw (growl). Combine with env/filter. INDEX 1-6 typical.
- More sound design: unison N (2-7 stacked detuned oscillators = fat super-saw), detune C (cents spread), drive X (0-1 distortion/warmth), lfo TARGET RATE DEPTH (TARGET = cutoff | pitch | gain; RATE in Hz; DEPTH 0-1) e.g. | saw | unison 5 | detune 18 | lp 1200 | lfo cutoff 0.3 0.6 for a moving super-saw. Use drive on bass/lead for grit, lfo cutoff for movement, lfo pitch for vibrato, lfo gain for tremolo.
- Real samples: add | sample NAME to a voice to play a real recorded sound instead of the synth. Synth drums (no sample needed): kick snare hat clap tom cowbell rim. Sampled drums: bd sd hh cp kick2 kick909 clubkick clap2 oh(open hat) rim cow perc tom tom2 tom3 ride tabla. Melodic (repitched): bass bass1 arpy gtr casio sax sitar juno pad2 hoover stab bleep sid voodoo. Loops/breaks (one-shot a whole drum loop): amen break amencut. BEAT-SLICING: | chop N cuts a loop/break into N slices and plays the next slice each hit (break: x x x x x x x x | sample amen | chop 8). Vocal/texture: vox yeah breath wood metal glass bottle. e.g. kick: x . x . | sample kick909 , lead: 1 3 5 1' | sample sitar , break: x . . . | sample amen. A melodic sample is repitched assuming its natural pitch is c3; if it sounds off, tune it with | root NOTE (e.g. | sample gtr | root c4).
- MIDI out: add | midi CH (1-16) to a voice to send it as MIDI notes to your DAW / hardware synth instead of the built-in sound. Melodic voices send their pitches; drum voices send the General-MIDI drum notes (kick=36 snare=38 hat=42 clap=39) — put drums on | midi 10. e.g. bass: 1 . 5 . | midi 1 , kick: x . x . | midi 10. If no MIDI device is connected it falls back to the internal sound.
- Bitcrush / lo-fi: crush BITS (1-8) quantizes a voice to fewer bits for a crunchy, gritty, IDM/lo-fi texture (lower = dirtier). e.g. drums or a lead: ... | crush 4. Great for Aphex-style grit.
- Chorus / width: chorus N (0-1) thickens and widens a voice with modulated stereo delays — lovely on pads, keys and leads, e.g. pad: ... | chorus 0.6. Phaser: | phaser N (0-1) sweeps a chain of all-pass notches with an LFO for the classic psychedelic whoosh — pads, leads, even hats. Per-voice compression: | comp N (0-1) tames transients and adds punch/glue — great on bass, drums, busy pads.
- Per-voice EQ: eq LOW MID HIGH shapes a voice's tone in dB (low shelf ~180Hz, mid peak ~1.2kHz, high shelf ~5kHz). e.g. bass: ... | eq 3 -2 -4 (more lows, less highs), hat: ... | eq -6 0 3 (thinner, brighter). Great for sitting voices in the mix.
- Per-voice space: reverb N adds that voice to the reverb (N 0-1, depth/room), delay N adds an echo send (N 0-1). e.g. lead: 1 . 3 . | delay 0.4 | reverb 0.3 , snare: . . x . | reverb 0.5. Use sparingly on a few voices for depth.
- Convolution reverb: ir SPACE [MIX] puts a real-sounding acoustic SPACE on a voice (a true convolver with a synthesized impulse response), as a per-voice insert. SPACE = hall | room | plate | spring | cathedral; MIX 0-1 (default 0.35). e.g. pad: ^ . . . | saw | ir hall 0.4 , gtr: 1 . 3 . | pluck 0.6 | ir spring , choir: ^ . . . | bow | ir cathedral 0.5. Richer/realer than reverb N; plate = bright dense studio, spring = vintage amp/dub, cathedral = huge. Use on a lead/pad, not everything.
- Stereo: pan P places a voice in the stereo field, P from -1 (left) to 1 (right), e.g. hat: ... | pan -0.4 , arp: ... | pan 0.5. Spread voices for width. (The master bus runs through a gentle compressor/limiter for studio glue.)
- Continuous signals (LFO on any param): write a parameter as a SIGNAL: WAVE RATE MIN MAX, where WAVE is sine|saw|tri|square|rand and RATE is cycles per bar. e.g. | lp sine 0.5 300 4000 (filter wobbles between 300-4000Hz over 2 bars), | pan sine 1 -1 1 (auto-pan), | drive tri 0.25 0 0.6 (rising/falling grit), | lp rand 4 400 3000 (random sample-and-hold filter). Works on lp/hp, drive, pan. Tempo-synced and seeded.
- Automation (movement over time): write a parameter as A..B to sweep it, e.g. | lp 300..6000 opens the filter, | drive 0..0.6 grows the grit, | pan -1..1 sweeps across. The sweep spans the current section (or | over N bars). Perfect for builds/risers and evolving long tracks — use it on the build section of a song.
- Groove / humanize: add a line feel: N (0-100) to nudge every hit's timing and velocity slightly (seeded, reproducible) so the track breathes instead of sounding robotic. feel: 20-35 is a tasteful amount. Add it to most tracks.
- Sidechain pump: add a line duck: 60 (0-100, or 0-1) so the whole mix ducks on every kick — the classic techno/house pumping groove. Higher = more pump. Needs a kick voice. Omit the line for no pumping.
- Sub-sequences (mini-notation): wrap tokens in [ ] to pack them into ONE step, played faster. e.g. hat: x [x x] x [x x x] (steps 2 and 4 subdivide into 2 and 3 quick hits), or lead: 1 [3 5] 1' [7 5 3] . The brackets NEST: bass: 1 [5 [1' 5]] gives nested speed-ups. Works for notes, degrees, chords, rests, and combines with ratchets/probability inside. This is how you make fast fills, rolls and complex rhythms without changing the number of main steps.
- Chord progression (theory layer): add a line prog: Am F C G (or romans i VI III VII when a key is set) — ONE chord per bar, cycling. Then any voice can FOLLOW the harmony with the ^ token: ^ plays the whole current chord (great for pads/stabs: pad: ^ . . . ^ . . .), and ^N plays the Nth chord-tone as a single note (^1 = root, ^2 = third, ^3 = fifth) — perfect for a bass that tracks the changes (bass: ^1 . . . ^1 . ^1 . | oct 2) or an arp (arp: ^1 ^2 ^3 ^2 | arp up). The whole arrangement then re-harmonises itself when you change prog:. Octave marks work: ^1' . This is the easiest way to make musically-correct chord changes. HARMONIZER: | chord NAME (maj min sus sus2 dim aug fifth oct maj7 min7 dom7 add9) turns EACH single note of a voice into that chord — instant rich pads/stabs from a one-note melody, e.g. pad: 1 . 4 . 5 . | saw | chord min7 | reverb 0.6.
- Stacks (polyphony on one voice): separate independent simultaneous lines with ; inside a single voice — they share that voice's timbre/effects but play at once. e.g. piano: 1 . 3 . ; 5 . . . ; 1' . . . (a three-note chord voicing with independent rhythms), or counter: 1 2 3 4 ; 5 4 3 2 (two melodic lines at once). Each line can have different length (cross-rhythm). Great for chords-with-groove, call-and-response, or counterpoint without adding extra voices. Combine with verbs after the | (they apply to every line).
- Random choice: a token {a b c} picks ONE alternative AT RANDOM each time it plays (seeded, so it replays identically) — unlike <a/b/c> which cycles in order. Great for generative variation: lead: {1 3 5 7} . {2 4 6} . , drums: x {. x} x {x .} (random fills). The alternatives can be notes, degrees, chords, rests, ^N, even [groups]. Combine with seed: to lock the randomness.
- A/B variation: a token <a/b/c> picks ONE alternative per bar, cycling each bar, so the pattern changes over time. e.g. kick: <x/.> x x x (a kick that drops out every other bar), or lead: <1/3/5> . . . . The alternatives can be notes, degrees, chords, rests, even with ?/* mods.
- True polymeter / phasing: add | poly N to a voice so it runs its OWN loop at N steps per bar and keeps looping across bar lines — if its pattern length differs from N it slowly drifts against the grid (Steve Reich style), realigning after a while. e.g. a: 1 2 3 4 5 | poly 16 against b: 1 2 3 4 | poly 16. Two near-identical patterns at | poly with different lengths phase against each other.
- Polyrhythm: give voices different step counts and they subdivide the same bar differently (3-against-4 etc), e.g. a: x x x  vs  b: x x x x. (Note: true cross-bar polymeter where patterns drift isn't supported yet — all voices realign each bar.)
- Song structure (for long tracks): add song: intro*4 verse*8 chorus*8 verse*8 chorus*8 outro*4 to lay out sections by bar count (name*bars, bare name = 4 bars; the arrangement loops). Then tag each voice with the sections it plays in using | in <sections>, e.g. lead: ... | in chorus , bass: ... | in verse chorus. Untagged voices play in every section. Build energy by adding voices in later sections (drops/build-ups). Without a song: line, every voice always plays. For LONG evolving tracks, give a voice a DIFFERENT pattern per section with name@section: e.g. bass: 1 . . . then bass@drop: 1 1 1 1 — during 'drop' the @drop version plays, elsewhere the base. Make the arrangement long (intro*8 verse*16 chorus*16 …) for multi-minute pieces.
- Visuals: scene: NAME (a visualiser) and per-voice | viz STYLE (burst|ring|beam|flash) | hue N (0-360) drive the browser version's fullscreen visuals. This request comes from the native LOOM app, which has no visuals: keep any scene:/viz/hue already in the current code unchanged, and only add new ones if the user asks for visuals.
- Comments start with #.
Make it MUSICAL: coherent key, sensible rhythm, 8-16 steps, 3-5 voices for a simple loop.
When the user asks for a LONG / full / structured / evolving track (or a specific length/genre), DO build it out: declare a long song: arrangement (e.g. intro*8 build*8 chorus*16 break*8 chorus*16 outro*8) and use name@section variants so each section differs (sparser intro, denser drop, filtered break), plus | in tags, real | sample sounds, and effects (lp/env/reverb/delay/drive/duck) for a polished mix.
Match the requested mood/genre. Avoid always defaulting to C major / 4-step loops — pick a key and scale that fit (minor, dorian, phrygian, hijaz, etc.) and write 16-step patterns.
EDITING: the user message gives the CURRENT LOOM code and a REQUEST (often in French). Treat the request as an edit of that code: keep the voices, key, tempo and structure the user did not ask to change, unless they ask for something new. Always return the COMPLETE updated program, never a fragment or a diff. If you add comments, write them in French.
NATIVE APP: the same .loom file also plays in the browser engine, so use the full language. The native engine may not render every effect or sample yet, so make the core of the track (drum hits and melodic voices with a saw/sine/tri/square timbre) sound good on its own.
EXAMPLE of the richness to aim for when asked for a full/long track (imitate the STRUCTURE — change key, genre, sounds and notes to fit the request):
seed: 5
duck: 65
key: f minor
song: intro*4 build*8 drop*8 break*4 drop*8 outro*4
kick: x . . . x . . . x . . . x . . . | sample bd
kick@intro: x . . . . . . . x . . . . . . . | sample bd
clap: . . . . x . . . . . . . x . . . | sample cp | gain 0.4 | in drop outro
hat: e(11,16) | sample hh | gain 0.18 | in build drop break outro
bass: . 1 . 1 . 1 5, . . 1 . 1 . 7, 1 . | saw | unison 3 | detune 16 | lp 520 q 6 | drive 0.3 | env 0.005 0.06 0.35 0.08 | oct 2 | gain 0.22 | in build drop outro
acid@build: 1 1 1' 1 5 1 1' 5 | saw | unison 3 | lp 300..5000 | over 8 | q 8 | drive 0.4 | gain 0.13
stab@drop: . . Am . . . Fm . . . Cm . . . G . | saw | unison 3 | lp 1800 q 6 | env 0.01 0.1 0.2 0.2 | reverb 0.3 | gain 0.12
pad: Fm7 . . . . . . . | saw | unison 4 | lp 820 | env 0.7 0.5 0.7 1.6 | reverb 0.5 | gain 0.13 | in intro break outro
END EXAMPLE. Now write the program for the user's request, with the same level of detail. Output ONLY the LOOM program, no prose, no backticks."#;
