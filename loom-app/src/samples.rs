//! « Mes sons », comme la carte du web : importer ses propres fichiers audio,
//! les voir, en jeter un, tout vider. Un son importé est copié en WAV dans
//! `~/.local/share/loom/samples/` (le dossier que le moteur lit déjà), donc il
//! reste là au prochain lancement et se joue avec `| sample NOM`.

use std::path::{Path, PathBuf};

/// Le dossier de mes sons (`$XDG_DATA_HOME/loom/samples` ou `~/.local/share/loom/samples`).
pub fn user_dir() -> Option<PathBuf> {
    Some(loom_audio::paths::data_dir()?.join("samples"))
}

/// Un nom jouable : minuscules, lettres, chiffres, `_` (`Ma Voix-2.WAV` → `ma_voix_2`).
pub fn clean_name(stem: &str) -> String {
    let mut s: String = stem.chars().map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '_' }).collect();
    while s.contains("__") { s = s.replace("__", "_"); }
    let s = s.trim_matches('_').to_string();
    let s = if s.is_empty() { "son".to_string() } else { s };
    if s.starts_with(|c: char| c.is_ascii_digit()) { format!("s{s}") } else { s }
}

/// Les chemins d'une saisie : un glisser-déposer du Finder colle `/a/b\ c.wav`,
/// d'autres terminaux `'/a/b c.wav'` ; plusieurs fichiers sont séparés par des espaces.
pub fn split_paths(input: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut chars = input.trim().chars();
    while let Some(c) = chars.next() {
        match (quote, c) {
            (None, '\\') => { if let Some(n) = chars.next() { cur.push(n); } }
            (None, '\'' | '"') => quote = Some(c),
            (Some(q), c) if c == q => quote = None,
            (None, c) if c.is_whitespace() => {
                if !cur.is_empty() { out.push(PathBuf::from(std::mem::take(&mut cur))); }
            }
            (_, c) => cur.push(c),
        }
    }
    if !cur.is_empty() { out.push(PathBuf::from(cur)); }
    out.into_iter().map(|p| expand_home(&p)).collect()
}

fn expand_home(p: &Path) -> PathBuf {
    match (p.strip_prefix("~"), loom_audio::paths::home()) {
        (Ok(rest), Some(h)) => h.join(rest),
        _ => p.to_path_buf(),
    }
}

/// Mes sons, triés par nom.
pub fn list_in(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir).into_iter().flatten().flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("wav")))
        .filter_map(|p| p.file_stem().and_then(|s| s.to_str()).map(str::to_string))
        .collect();
    v.sort();
    v
}

pub fn list() -> Vec<String> { user_dir().map(|d| list_in(&d)).unwrap_or_default() }

/// Un nom libre dans `dir` et hors de `taken` (les sons fournis) : `voix`, `voix2`, `voix3`…
fn free_name(dir: &Path, base: &str, taken: &[String]) -> String {
    (1..).map(|i| if i == 1 { base.to_string() } else { format!("{base}{i}") })
        .find(|n| !dir.join(format!("{n}.wav")).exists() && !taken.contains(n))
        .unwrap_or_else(|| base.to_string())
}

/// Convertit un fichier non WAV en WAV avec l'outil du système (afconvert sur
/// macOS, ffmpeg ailleurs).
fn to_wav(src: &Path, dst: &Path) -> Result<(), String> {
    use std::process::{Command, Stdio};
    let tries: [(&str, Vec<&std::ffi::OsStr>); 2] = [
        ("/usr/bin/afconvert", vec![src.as_os_str(), "-f".as_ref(), "WAVE".as_ref(), "-d".as_ref(), "LEI16".as_ref(), dst.as_os_str()]),
        ("ffmpeg", vec!["-v".as_ref(), "quiet".as_ref(), "-y".as_ref(), "-i".as_ref(), src.as_os_str(), dst.as_os_str()]),
    ];
    for (exe, args) in tries {
        let ok = Command::new(exe).args(&args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
            .status().is_ok_and(|s| s.success());
        if ok && dst.exists() { return Ok(()); }
    }
    Err("format non lu : donne un WAV (ou installe ffmpeg)".into())
}

/// Importe `src` dans `dir` et rend le nom à écrire après `| sample`.
/// `taken` = les noms des sons fournis, qu'un import ne doit jamais masquer.
pub fn import_into(src: &Path, dir: &Path, taken: &[String]) -> Result<String, String> {
    let label = src.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    if !src.is_file() { return Err(format!("« {label} » introuvable")); }
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let stem = src.file_stem().and_then(|s| s.to_str()).unwrap_or("son");
    let name = free_name(dir, &clean_name(stem), taken);
    let dst = dir.join(format!("{name}.wav"));
    let is_wav = src.extension().is_some_and(|x| x.eq_ignore_ascii_case("wav"));
    if is_wav { std::fs::copy(src, &dst).map_err(|e| e.to_string())?; } else { to_wav(src, &dst)?; }
    // vérifie que le moteur sait le lire ; sinon on ne garde rien
    match loom_audio::sample::SampleData::from_file(&dst) {
        Ok(d) if d.frames() > 0 => Ok(name),
        Ok(_) => { let _ = std::fs::remove_file(&dst); Err(format!("« {label} » : fichier vide")) }
        Err(e) => { let _ = std::fs::remove_file(&dst); Err(format!("« {label} » : {e}")) }
    }
}

pub fn import(src: &Path, taken: &[String]) -> Result<String, String> {
    let dir = user_dir().ok_or("pas de dossier personnel")?;
    import_into(src, &dir, taken)
}

/// Met un de mes sons à la Corbeille (récupérable), jamais un son fourni.
pub fn trash_in(name: &str, dir: &Path, trash: &Path) -> Result<(), String> {
    if name.contains(['/', '\\']) || name.starts_with('.') { return Err("nom invalide".into()); }
    let src = dir.join(format!("{name}.wav"));
    if !src.is_file() { return Err(format!("« {name} » n'est pas un de tes sons")); }
    std::fs::create_dir_all(trash).map_err(|e| e.to_string())?;
    let dst = (1..).map(|i| trash.join(if i == 1 { format!("{name}.wav") } else { format!("{name} {i}.wav") }))
        .find(|p| !p.exists()).ok_or("corbeille pleine")?;
    std::fs::rename(&src, &dst).or_else(|_| std::fs::copy(&src, &dst).and_then(|_| std::fs::remove_file(&src)))
        .map_err(|e| e.to_string())
}

pub fn trash(name: &str) -> Result<(), String> {
    let dir = user_dir().ok_or("pas de dossier personnel")?;
    let bin = crate::library::Library::trash_dir();
    trash_in(name, &dir, &bin)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wav(path: &Path) {
        // 8 kHz, mono, 16 bits, 4 trames
        let data: Vec<u8> = [0i16, 1000, -1000, 0].iter().flat_map(|s| s.to_le_bytes()).collect();
        let mut b = Vec::new();
        b.extend(b"RIFF"); b.extend((36 + data.len() as u32).to_le_bytes()); b.extend(b"WAVEfmt ");
        b.extend(16u32.to_le_bytes()); b.extend(1u16.to_le_bytes()); b.extend(1u16.to_le_bytes());
        b.extend(8000u32.to_le_bytes()); b.extend(16000u32.to_le_bytes()); b.extend(2u16.to_le_bytes()); b.extend(16u16.to_le_bytes());
        b.extend(b"data"); b.extend((data.len() as u32).to_le_bytes()); b.extend(data);
        std::fs::write(path, b).unwrap();
    }

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("loom-sons-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn noms_et_chemins() {
        assert_eq!(clean_name("Ma Voix-2"), "ma_voix_2");
        assert_eq!(clean_name("808"), "s808");
        assert_eq!(clean_name("---"), "son");
        let p = split_paths("/a/b\\ c.wav '/d/e f.aif' \"/g.wav\"");
        assert_eq!(p, vec![PathBuf::from("/a/b c.wav"), PathBuf::from("/d/e f.aif"), PathBuf::from("/g.wav")]);
    }

    #[test]
    fn importer_lister_jeter() {
        let root = tmp("imp");
        let (src, dir, bin) = (root.join("in"), root.join("sons"), root.join("bin"));
        std::fs::create_dir_all(&src).unwrap();
        wav(&src.join("Ma Voix.wav"));
        wav(&src.join("bd.wav"));
        assert_eq!(import_into(&src.join("Ma Voix.wav"), &dir, &[]).unwrap(), "ma_voix");
        assert_eq!(import_into(&src.join("Ma Voix.wav"), &dir, &[]).unwrap(), "ma_voix2");
        // un son fourni n'est jamais masqué
        assert_eq!(import_into(&src.join("bd.wav"), &dir, &["bd".into()]).unwrap(), "bd2");
        assert_eq!(list_in(&dir), vec!["bd2", "ma_voix", "ma_voix2"]);
        // un faux WAV est refusé et ne laisse rien
        std::fs::write(src.join("faux.wav"), b"pas du son").unwrap();
        assert!(import_into(&src.join("faux.wav"), &dir, &[]).is_err());
        assert!(import_into(&src.join("absent.wav"), &dir, &[]).is_err());
        assert_eq!(list_in(&dir).len(), 3);
        trash_in("ma_voix", &dir, &bin).unwrap();
        assert!(bin.join("ma_voix.wav").exists());
        assert!(trash_in("../x", &dir, &bin).is_err());
        assert!(trash_in("inconnu", &dir, &bin).is_err());
        assert_eq!(list_in(&dir), vec!["bd2", "ma_voix2"]);
        let _ = std::fs::remove_dir_all(&root);
    }
}
