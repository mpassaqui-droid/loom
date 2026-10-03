//! LOOM installé seul, sur une machine propre : exemples et sons sans le dépôt.
//! (Fichier de test à part : il change les variables d'environnement.)

#[test]
fn premier_lancement_sur_machine_propre() {
    let tmp = std::env::temp_dir().join(format!("loom-propre-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    // SAFETY : ce binaire de test n'a qu'un test, rien d'autre ne lit l'environnement en même temps.
    unsafe {
        std::env::set_var("HOME", &tmp);
        std::env::set_var("XDG_DATA_HOME", tmp.join("data"));
        std::env::set_var("XDG_STATE_HOME", tmp.join("state"));
        std::env::set_var("XDG_MUSIC_DIR", tmp.join("music"));
    }
    std::env::set_current_dir(&tmp).unwrap();
    let groups = loom_app::library::Library::scan();
    let ex = groups.iter().find(|(l, _)| l == "Exemples").expect("pas d'exemples");
    assert_eq!(ex.1.len(), loom_app::library::Library::embedded_count());
    assert!(ex.1.len() >= 60, "{}", ex.1.len());
    // chaque exemple se lit et a des voix
    for p in &ex.1 {
        let src = std::fs::read_to_string(p).unwrap();
        assert!(!loom_core::Program::parse(&src).voices.is_empty(), "{}", p.display());
    }
    // un deuxième passage ne réécrit rien
    let stamp = tmp.join("data/loom/examples/.version");
    let t0 = std::fs::metadata(&stamp).unwrap().modified().unwrap();
    loom_app::library::Library::ensure_examples();
    assert_eq!(std::fs::metadata(&stamp).unwrap().modified().unwrap(), t0);
    // les sons de base jouent sans dossier de sons
    let bank = loom_audio::sample::SampleBank::load_default();
    assert!(bank.get("bd").is_some());
    let _ = std::fs::remove_dir_all(&tmp);
}
