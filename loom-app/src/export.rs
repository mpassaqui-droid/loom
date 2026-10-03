//! Exporter le son dans le format que le nom de fichier demande : WAV écrit
//! par LOOM (16, 24 bits ou flottant), FLAC, MP3, M4A, AIFF, OGG convertis
//! par l'outil du système (afconvert sur macOS, sinon ffmpeg).

use std::path::Path;
use std::process::{Command, Stdio};

/// Les extensions que l'export sait faire.
pub const FORMATS: [&str; 6] = ["wav", "flac", "mp3", "m4a", "aiff", "ogg"];

fn ext(path: &Path) -> String {
    path.extension().and_then(|e| e.to_str()).unwrap_or("wav").to_lowercase()
}

/// La commande de conversion WAV → `dst`, essayée dans l'ordre.
fn converters(src: &Path, dst: &Path) -> Vec<(String, Vec<String>)> {
    let (s, d) = (src.display().to_string(), dst.display().to_string());
    let af = |fmt: &str, data: &str, extra: &[&str]| {
        let mut a = vec![s.clone(), "-f".into(), fmt.into(), "-d".into(), data.into()];
        a.extend(extra.iter().map(|x| x.to_string()));
        a.push(d.clone());
        ("/usr/bin/afconvert".to_string(), a)
    };
    let ff = |extra: &[&str]| {
        let mut a: Vec<String> = ["-v", "error", "-y", "-i", &s].iter().map(|x| x.to_string()).collect();
        a.extend(extra.iter().map(|x| x.to_string()));
        a.push(d.clone());
        ("ffmpeg".to_string(), a)
    };
    match ext(dst).as_str() {
        "flac" => vec![af("flac", "flac", &[]), ff(&["-c:a", "flac"])],
        "m4a" => vec![af("m4af", "aac", &["-b", "256000"]), ff(&["-c:a", "aac", "-b:a", "256k"])],
        "aif" | "aiff" => vec![af("AIFF", "BEI24", &[]), ff(&["-c:a", "pcm_s24be"])],
        "mp3" => vec![ff(&["-c:a", "libmp3lame", "-b:a", "320k"]), ("lame".into(), vec!["-b".into(), "320".into(), s.clone(), d.clone()])],
        "ogg" => vec![ff(&["-c:a", "libvorbis", "-q:a", "8"])],
        _ => vec![],
    }
}

/// Écrit `samples` (stéréo entrelacé) dans `path`. Rend le nom du format écrit.
pub fn write_audio(path: &Path, samples: &[f32], sample_rate: u32) -> Result<String, String> {
    let e = ext(path);
    if e == "wav" {
        loom_audio::write_wav(path, samples, sample_rate).map_err(|e| e.to_string())?;
        return Ok(format!("WAV {}", loom_audio::WavFormat::current().name()));
    }
    if !FORMATS.contains(&e.as_str()) && e != "aif" {
        return Err(format!("format .{e} inconnu (formats : {})", FORMATS.join(", ")));
    }
    // un WAV 32 bits flottant intermédiaire, à côté de la sortie
    let tmp = path.with_extension(format!("{e}.loom-tmp.wav"));
    loom_audio::write_wav_as(&tmp, samples, sample_rate, loom_audio::WavFormat::Float32).map_err(|e| e.to_string())?;
    let mut last = String::from("aucun convertisseur");
    for (exe, args) in converters(&tmp, path) {
        let _ = std::fs::remove_file(path);
        match Command::new(&exe).args(&args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped()).output() {
            Ok(o) if o.status.success() && path.is_file() => {
                let _ = std::fs::remove_file(&tmp);
                return Ok(e.to_uppercase());
            }
            Ok(o) => last = format!("{exe} : {}", String::from_utf8_lossy(&o.stderr).lines().last().unwrap_or("échec")),
            Err(err) => last = format!("{exe} : {err}"),
        }
    }
    let _ = std::fs::remove_file(&tmp);
    Err(format!("export .{e} impossible ({last}) ; installe ffmpeg ou choisis .wav"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convertisseurs_par_extension() {
        let c = converters(Path::new("/t/a.wav"), Path::new("/t/a.flac"));
        assert!(c[0].0.ends_with("afconvert") && c[0].1.contains(&"flac".to_string()));
        assert_eq!(converters(Path::new("a.wav"), Path::new("a.mp3"))[0].0, "ffmpeg");
        assert!(converters(Path::new("a.wav"), Path::new("a.xyz")).is_empty());
        assert!(write_audio(Path::new("/nonexistent/a.xyz"), &[0.0, 0.0], 48_000).is_err());
    }

    /// Bout en bout sur une machine qui a les outils : `cargo test -p loom-app export -- --ignored`
    #[test]
    #[ignore]
    fn exporte_flac_mp3_m4a() {
        let dir = std::env::temp_dir().join(format!("loom-export-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sig: Vec<f32> = (0..96_000).map(|i| (i as f32 * 0.03).sin() * 0.4).collect();
        for e in ["flac", "mp3", "m4a", "aiff"] {
            let p = dir.join(format!("t.{e}"));
            write_audio(&p, &sig, 48_000).unwrap_or_else(|err| panic!("{e} : {err}"));
            assert!(std::fs::metadata(&p).unwrap().len() > 1000, "{e}");
        }
        assert!(std::fs::read_dir(&dir).unwrap().flatten().all(|f| !f.file_name().to_string_lossy().contains("loom-tmp")));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
