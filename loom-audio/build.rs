//! Embarque les sons de base (`web/samples/*.wav`) dans le binaire : LOOM joue
//! `| sample bd` même installé seul, sans le dépôt.
use std::{env, fs, path::PathBuf};

fn main() {
    let dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("../web/samples");
    println!("cargo:rerun-if-changed={}", dir.display());
    let embed = env::var_os("CARGO_FEATURE_EMBED").is_some();
    let mut files: Vec<PathBuf> = if !embed { Vec::new() } else { fs::read_dir(&dir).map(|r| r.filter_map(|e| e.ok().map(|e| e.path())).collect()).unwrap_or_default() };
    files.retain(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("wav")));
    files.sort();
    let mut out = String::from("/// Les sons de base embarqués : (nom, octets WAV).\npub static EMBEDDED: &[(&str, &[u8])] = &[\n");
    for p in &files {
        let name = p.file_stem().unwrap().to_string_lossy();
        out += &format!("    ({:?}, include_bytes!({:?})),\n", name, p.canonicalize().unwrap());
    }
    out += "];\n";
    fs::write(PathBuf::from(env::var("OUT_DIR").unwrap()).join("embedded_samples.rs"), out).unwrap();
}
