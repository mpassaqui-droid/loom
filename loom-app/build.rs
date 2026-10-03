//! Embarque les morceaux d'exemple (`*.loom` à la racine du dépôt) : le
//! sélecteur de morceaux en a dès le premier lancement, sans install.sh.
use std::{env, fs, path::PathBuf};

fn main() {
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("..");
    println!("cargo:rerun-if-changed={}", root.display());
    let mut files: Vec<PathBuf> = fs::read_dir(&root).map(|r| r.filter_map(|e| e.ok().map(|e| e.path())).collect()).unwrap_or_default();
    files.retain(|p| p.is_file() && p.extension().is_some_and(|e| e == "loom"));
    files.sort();
    let mut out = String::from("/// Les morceaux d'exemple embarqués : (nom de fichier, texte).\npub static EXAMPLES: &[(&str, &str)] = &[\n");
    for p in &files {
        out += &format!("    ({:?}, include_str!({:?})),\n", p.file_name().unwrap().to_string_lossy(), p.canonicalize().unwrap());
    }
    out += "];\n";
    fs::write(PathBuf::from(env::var("OUT_DIR").unwrap()).join("examples.rs"), out).unwrap();
}
