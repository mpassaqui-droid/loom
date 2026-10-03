//! La dictée de VIBE, comme le micro du web : on parle, whisper (en local,
//! rien ne quitte la machine) écrit la demande. Ici la partie sans micro :
//! rééchantillonner, écrire le WAV, lancer `whisper-cli` dans un fil à part.

use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, TryRecvError};

/// whisper.cpp veut du 16 kHz mono.
const RATE: u32 = 16_000;

/// Rééchantillonne (linéaire) vers 16 kHz.
pub fn to_16k(samples: &[f32], rate: u32) -> Vec<f32> {
    if rate == RATE || samples.is_empty() { return samples.to_vec(); }
    let n = (samples.len() as u64 * RATE as u64 / rate as u64) as usize;
    (0..n).map(|i| {
        let x = i as f64 * rate as f64 / RATE as f64;
        let (j, f) = (x as usize, (x.fract()) as f32);
        let a = samples[j.min(samples.len() - 1)];
        let b = samples[(j + 1).min(samples.len() - 1)];
        a + (b - a) * f
    }).collect()
}

/// Un WAV PCM 16 bits mono à 16 kHz.
pub fn wav(samples16k: &[f32]) -> Vec<u8> {
    let data: Vec<u8> = samples16k.iter().flat_map(|s| ((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes()).collect();
    let mut b = Vec::with_capacity(44 + data.len());
    b.extend(b"RIFF");
    b.extend((36 + data.len() as u32).to_le_bytes());
    b.extend(b"WAVEfmt ");
    b.extend(16u32.to_le_bytes());
    b.extend(1u16.to_le_bytes());
    b.extend(1u16.to_le_bytes());
    b.extend(RATE.to_le_bytes());
    b.extend((RATE * 2).to_le_bytes());
    b.extend(2u16.to_le_bytes());
    b.extend(16u16.to_le_bytes());
    b.extend(b"data");
    b.extend((data.len() as u32).to_le_bytes());
    b.extend(data);
    b
}

/// Le programme `whisper-cli` (Homebrew, ou dans le PATH).
pub fn whisper_cli() -> Option<PathBuf> {
    let fixed = ["/opt/homebrew/bin/whisper-cli", "/usr/local/bin/whisper-cli", "/usr/bin/whisper-cli"];
    fixed.iter().map(PathBuf::from).find(|p| p.is_file()).or_else(|| {
        std::env::var_os("PATH").and_then(|path| std::env::split_paths(&path).map(|d| d.join("whisper-cli")).find(|p| p.is_file()))
    })
}

/// Le modèle : `$LOOM_WHISPER_MODEL`, sinon le meilleur trouvé dans `~/.whisper-models`.
pub fn model() -> Option<PathBuf> {
    if let Some(m) = std::env::var_os("LOOM_WHISPER_MODEL").map(PathBuf::from).filter(|p| p.is_file()) { return Some(m); }
    let home = PathBuf::from(std::env::var_os("HOME")?);
    let dirs = [home.join(".whisper-models"), home.join(".local/share/whisper"), home.join(".cache/whisper")];
    let names = ["ggml-large-v3-turbo.bin", "ggml-medium.bin", "ggml-small.bin", "ggml-base.bin", "ggml-tiny.bin"];
    names.iter().flat_map(|n| dirs.iter().map(move |d| d.join(n))).find(|p| p.is_file())
}

/// Ce que whisper écrit pour un silence ou un bruit, à ne pas envoyer à VIBE.
fn is_noise(text: &str) -> bool {
    let t = text.trim().trim_matches(|c: char| c == '[' || c == ']' || c == '(' || c == ')' || c == '.' || c == ' ').to_lowercase();
    t.is_empty() || ["blank_audio", "silence", "musique", "music", "sous-titres réalisés par la communauté d'amara.org"].contains(&t.as_str())
}

/// Nettoie la sortie de whisper : une seule ligne, sans les marques de silence.
pub fn clean(stdout: &str) -> String {
    let joined = stdout.lines().map(str::trim).filter(|l| !l.is_empty() && !is_noise(l)).collect::<Vec<_>>().join(" ");
    joined.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Une transcription en cours, dans un fil à part.
pub struct Dictation {
    rx: Receiver<Result<String, String>>,
}

impl Dictation {
    /// Lance whisper sur `samples` (mono, à `rate` Hz).
    pub fn start(samples: Vec<f32>, rate: u32) -> Result<Dictation, String> {
        let exe = whisper_cli().ok_or("whisper-cli introuvable : brew install whisper-cpp")?;
        let model = model().ok_or("aucun modèle whisper : mets un ggml-*.bin dans ~/.whisper-models")?;
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let _ = tx.send(transcribe(&exe, &model, &samples, rate));
        });
        Ok(Dictation { rx })
    }

    /// `Some` quand whisper a fini.
    pub fn poll(&mut self) -> Option<Result<String, String>> {
        match self.rx.try_recv() {
            Ok(r) => Some(r),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Err("la dictée s'est arrêtée".into())),
        }
    }
}

fn transcribe(exe: &std::path::Path, model: &std::path::Path, samples: &[f32], rate: u32) -> Result<String, String> {
    let peak = samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    if samples.len() < rate as usize / 2 || peak < 0.01 { return Err("je n'ai rien entendu".into()); }
    let path = std::env::temp_dir().join(format!("loom-dictee-{}.wav", std::process::id()));
    std::fs::write(&path, wav(&to_16k(samples, rate))).map_err(|e| e.to_string())?;
    let lang = std::env::var("LOOM_DICTEE_LANG").unwrap_or_else(|_| "fr".into());
    let out = std::process::Command::new(exe)
        .arg("-m").arg(model).args(["-l", &lang, "-nt", "-np"]).arg("-f").arg(&path)
        .stdin(std::process::Stdio::null())
        .output();
    let _ = std::fs::remove_file(&path);
    let out = out.map_err(|e| format!("whisper : {e}"))?;
    if !out.status.success() {
        return Err(format!("whisper : {}", String::from_utf8_lossy(&out.stderr).lines().last().unwrap_or("échec")));
    }
    let text = clean(&String::from_utf8_lossy(&out.stdout));
    if text.is_empty() { Err("je n'ai rien compris".into()) } else { Ok(text) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reechantillonne_et_ecrit_le_wav() {
        let s: Vec<f32> = (0..48_000).map(|i| (i as f32 * 0.01).sin()).collect();
        let r = to_16k(&s, 48_000);
        assert_eq!(r.len(), 16_000);
        let w = wav(&r);
        assert_eq!(w.len(), 44 + 32_000);
        assert_eq!(&w[..4], b"RIFF");
        assert_eq!(u32::from_le_bytes([w[24], w[25], w[26], w[27]]), 16_000);
    }

    #[test]
    fn nettoie_la_sortie() {
        assert_eq!(clean("\n [BLANK_AUDIO]\n  ajoute une basse   qui groove\n"), "ajoute une basse qui groove");
        assert_eq!(clean("[Musique]\n"), "");
    }

    /// Bout en bout sur macOS : `say` fabrique une phrase, whisper la relit.
    /// `cargo test -p loom-app dictee -- --ignored`
    #[test]
    #[ignore]
    fn whisper_relit_une_phrase() {
        let dir = std::env::temp_dir().join(format!("loom-dictee-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (aiff, w) = (dir.join("p.aiff"), dir.join("p.wav"));
        let ok = std::process::Command::new("say").args(["-v", "Thomas", "ajoute une basse qui groove", "-o"]).arg(&aiff).status().unwrap();
        assert!(ok.success());
        assert!(std::process::Command::new("afconvert").arg(&aiff).args(["-f", "WAVE", "-d", "LEI16@44100"]).arg(&w).status().unwrap().success());
        let d = loom_audio::sample::SampleData::from_file(&w).unwrap();
        let mut job = Dictation::start(d.channels[0].clone(), d.sample_rate as u32).unwrap();
        let text = loop {
            if let Some(r) = job.poll() { break r.unwrap(); }
            std::thread::sleep(std::time::Duration::from_millis(100));
        };
        let _ = std::fs::remove_dir_all(&dir);
        assert!(text.to_lowercase().contains("basse"), "{text}");
    }
}
