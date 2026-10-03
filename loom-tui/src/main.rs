//! `loom` — LOOM in the terminal.
//!
//!   loom [song.loom]                     live-coding editor with native audio
//!   loom play song.loom                  play without the editor (Ctrl+C to stop)
//!   loom render song.loom out.wav [N]    bounce N bars (default: the song, or 8) to WAV
//!   loom render song.loom dir --stems [N]  one WAV per voice in `dir`
//!   loom bake song.loom out.json [N]     MIDI notes per voice + tempo + tuning, for Ableton Live
//!
//! Architecture: the audio thread lives inside `loom_audio::Engine` (cpal);
//! this thread draws with ratatui and sends it commands. Edits hot-swap at
//! the next bar line, so the music never stutters while you type.

mod app;
mod editor;
mod scene;
mod studio_view;
mod teacher_view;
mod ui;

use app::App;
use ratatui::crossterm::event::{self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyEventKind};
use ratatui::crossterm::execute;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

const USAGE: &str = "\
LOOM — un langage de live-coding musical

usage :
  loom [morceau.loom]                          éditeur live avec son natif
  loom play morceau.loom                       jouer sans l'éditeur (Ctrl+C pour arrêter)
  loom render morceau.loom sortie.wav [N]      exporter N mesures (.wav .flac .mp3 .m4a .aiff .ogg)
  loom render morceau.loom dossier --stems [N] une piste WAV par voix dans le dossier
  loom render --help                           détails de l'export
  loom bake morceau.loom sortie.json [N]       cuire en notes MIDI par voix (JSON)
  loom bake morceau.loom sortie.mid [N]        fichier MIDI standard (une piste par voix)
  loom bake morceau.loom sortie.musicxml [N]   partition (MuseScore, Sibelius, Dorico ;
                                               .ly pour LilyPond)
  loom loudness son.wav [cible]                LUFS intégré, LRA, crête vraie
                                               (cibles : spotify youtube apple deezer club)
  loom scala accordage.scl [tonique]           un fichier Scala → la ligne key: à coller
  loom contrepoint morceau.loom                 quintes/octaves parallèles, croisements, sauts
  loom reharm morceau.loom [jazz|tritone|iiv]  réharmonise prog: (ii-V, substitution de
                                               triton) et écrit le résultat sur la sortie
  loom plugins                                 les Audio Units installés (macOS)
  loom au \"Nom\" morceau.loom sortie.wav [voix] [N]
                                               jouer les notes des voix mélodiques (ou d'une
                                               voix) dans un instrument AU (prototype)
  loom-gui [morceau.loom]                      la même chose dans une fenêtre
";

const RENDER_USAGE: &str = "\
loom render — exporter un morceau en WAV (hors-ligne, sans carte son)

usage :
  loom render morceau.loom sortie.wav [N]
  loom render morceau.loom dossier --stems [N]

  N         nombre de mesures (par défaut : tout le song:, sinon 8)
  --24      WAV 24 bits (par défaut) · --16 : 16 bits avec dither · --32f : flottant
  --target spotify|youtube|apple|deezer|club   amène le morceau à la loudness
            de la plateforme (gain, sans dépasser sa crête vraie)
  --lufs X  même chose vers X LUFS (crête vraie max -1 dBTP)
  sortie    l'extension choisit le format : .wav .flac .mp3 .m4a .aiff .ogg
  --sr HZ   fréquence d'échantillonnage (par défaut 48000 ; 44100, 96000…)
  --stems   une piste WAV par voix (kick.wav, bass.wav…) dans le dossier,
            chacune passée par la même chaîne master que le morceau entier
            (pour mixer dans un logiciel audio)
";

/// What the command line asked for.
#[derive(Debug, PartialEq)]
enum Command {
    Help,
    RenderHelp,
    Version,
    Play(String),
    /// Bounce to `out` (a WAV, or a folder of stems), `bars` bars or the whole song.
    Render { file: String, out: String, bars: Option<u32>, stems: bool, format: Option<loom_audio::WavFormat>, sr: u32, target: Option<(f64, f64)> },
    /// Mastering report of an audio file, against a platform if named.
    Loudness { file: String, target: Option<String> },
    Reharm { file: String, how: String },
    Counterpoint { file: String },
    Scala { file: String, tonic: String },
    /// MIDI notes per voice, tempo, sections and tuning as JSON (for a DAW).
    Bake { file: String, out: String, bars: Option<u32> },
    /// Activer une clé (`Some`), la retirer (`retirer`), ou voir le statut.
    /// Les Audio Units installés.
    Plugins,
    /// Les notes de `voice` (ou des voix mélodiques) jouées dans l'instrument AU `plugin`.
    Au { plugin: String, file: String, out: String, voice: Option<String>, bars: Option<u32> },
    Tui(Option<PathBuf>),
}

/// Read the arguments (without the program name).
fn parse(args: &[&str]) -> Result<Command, String> {
    Ok(match args {
        ["-h" | "--help" | "help"] => Command::Help,
        ["-V" | "--version"] => Command::Version,
        ["play", file] => Command::Play(file.to_string()),
        ["render", rest @ ..] => {
            if rest.iter().any(|a| matches!(*a, "-h" | "--help")) {
                return Ok(Command::RenderHelp);
            }
            let stems = rest.contains(&"--stems");
            let format = rest.iter().find_map(|a| a.strip_prefix("--").and_then(loom_audio::WavFormat::from_name));
            let mut sr = 48_000;
            let mut target = None;
            let mut positional: Vec<&str> = Vec::new();
            let mut it = rest.iter().copied();
            while let Some(a) = it.next() {
                if a == "--target" {
                    let name = it.next().unwrap_or("");
                    target = Some(loom_app::studio::bs1770::target(name).ok_or(format!("cible inconnue : {name} (spotify youtube apple deezer club)"))?);
                } else if a == "--lufs" {
                    target = match it.next().and_then(|v| v.parse::<f64>().ok()) {
                        Some(v) if (-40.0..=0.0).contains(&v) => Some((v, -1.0)),
                        _ => return Err("--lufs attend une valeur entre -40 et 0".into()),
                    };
                } else if a == "--sr" {
                    sr = match it.next().and_then(|v| v.parse::<u32>().ok()) {
                        Some(v) if (8_000..=384_000).contains(&v) => v,
                        _ => return Err("--sr attend une fréquence entre 8000 et 384000".into()),
                    };
                } else if a != "--stems" && !(a.starts_with("--") && loom_audio::WavFormat::from_name(a).is_some()) {
                    positional.push(a);
                }
            }
            if let Some(flag) = positional.iter().find(|a| a.starts_with('-')) {
                return Err(format!("option inconnue : {flag}\n\n{RENDER_USAGE}"));
            }
            let (file, out, bars) = match positional.as_slice() {
                [file, out] => (file, out, None),
                [file, out, bars] => match bars.parse::<u32>() {
                    Ok(n) if n > 0 => (file, out, Some(n)),
                    _ => return Err(format!("nombre de mesures invalide : {bars}")),
                },
                _ => return Err(format!("il faut un morceau et une sortie\n\n{RENDER_USAGE}")),
            };
            Command::Render { file: file.to_string(), out: out.to_string(), bars, stems, format, sr, target }
        }
        ["bake", file, out] => Command::Bake { file: file.to_string(), out: out.to_string(), bars: None },
        ["bake", file, out, bars] => match bars.parse::<u32>() {
            Ok(n) if n > 0 => Command::Bake { file: file.to_string(), out: out.to_string(), bars: Some(n) },
            _ => return Err(format!("nombre de mesures invalide : {bars}")),
        },
        ["loudness", file] => Command::Loudness { file: file.to_string(), target: None },
        ["loudness", file, t] => Command::Loudness { file: file.to_string(), target: Some(t.to_string()) },
        ["scala", file] => Command::Scala { file: file.to_string(), tonic: "c".into() },
        ["scala", file, tonic] => Command::Scala { file: file.to_string(), tonic: tonic.to_string() },
        ["contrepoint" | "counterpoint", file] => Command::Counterpoint { file: file.to_string() },
        ["reharm", file] => Command::Reharm { file: file.to_string(), how: "jazz".into() },
        ["reharm", file, how] => Command::Reharm { file: file.to_string(), how: how.to_string() },
        ["plugins"] => Command::Plugins,
        ["au", plugin, file, out, rest @ ..] if rest.len() <= 2 => {
            let (mut voice, mut bars) = (None, None);
            for a in rest {
                match a.parse::<u32>() {
                    Ok(n) if n > 0 => bars = Some(n),
                    Ok(_) => return Err(format!("nombre de mesures invalide : {a}")),
                    Err(_) => voice = Some(a.to_string()),
                }
            }
            Command::Au { plugin: plugin.to_string(), file: file.to_string(), out: out.to_string(), voice, bars }
        }
        [] => Command::Tui(None),
        [file] if !file.starts_with('-') => Command::Tui(Some(PathBuf::from(file))),
        _ => return Err(format!("arguments non reconnus\n\n{USAGE}")),
    })
}


fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = parse(&args).and_then(|command| match command {
        Command::Help => { print!("{USAGE}"); Ok(()) }
        Command::RenderHelp => { print!("{RENDER_USAGE}"); Ok(()) }
        Command::Version => { println!("loom {}", env!("CARGO_PKG_VERSION")); Ok(()) }
        Command::Play(file) => play(&file),
        Command::Render { file, out, bars, stems, format, sr, target } => {
            if let Some(f) = format { loom_audio::WavFormat::set_default(f); }
            if stems { render_stems(&file, &out, bars, sr) } else { render(&file, &out, bars, sr, target) }
        }
        Command::Loudness { file, target } => loudness(&file, target.as_deref()),
        Command::Reharm { file, how } => reharm(&file, &how),
        Command::Scala { file, tonic } => {
            let scl = std::fs::read_to_string(&file).map_err(|e| format!("{file} : {e}"))?;
            println!("{}", loom_core::scala_to_key_line(&scl, &tonic)?);
            Ok(())
        }
        Command::Counterpoint { file } => {
            let src = std::fs::read_to_string(&file).map_err(|e| format!("{file} : {e}"))?;
            let lines = loom_core::prof::counterpoint_report(&src);
            if lines.is_empty() { println!("moins de deux lignes mélodiques : rien à corriger"); }
            for l in lines { println!("{l}"); }
            Ok(())
        }
        Command::Bake { file, out, bars } => bake(&file, &out, bars),
        Command::Plugins => plugins(),
        Command::Au { plugin, file, out, voice, bars } => au(&plugin, &file, &out, voice.as_deref(), bars),
        Command::Tui(path) => tui(path),
    });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => { eprintln!("loom : {e}"); ExitCode::FAILURE }
    }
}

fn read(file: &str) -> Result<String, String> {
    std::fs::read_to_string(file).map_err(|e| format!("impossible de lire {file} : {e}"))
}

fn tui(path: Option<PathBuf>) -> Result<(), String> {
    let mut app = App::new(path);
    let mut terminal = ratatui::init();
    let _ = execute!(std::io::stdout(), EnableBracketedPaste);
    let result = (|| -> std::io::Result<()> {
        while !app.quit {
            app.tick();
            terminal.draw(|f| ui::draw(f, &mut app))?;
            // ~30 fps keeps the playhead smooth without burning CPU
            if event::poll(Duration::from_millis(33))? {
                match event::read()? {
                    Event::Key(k) if k.kind != KeyEventKind::Release => app.on_key(k),
                    Event::Paste(text) => app.on_paste(&text),
                    _ => {}
                }
            }
        }
        Ok(())
    })();
    let _ = execute!(std::io::stdout(), DisableBracketedPaste);
    ratatui::restore();
    result.map_err(|e| e.to_string())
}

fn play(file: &str) -> Result<(), String> {
    let src = read(file)?;
    let program = loom_core::Program::parse(&src);
    if program.voices.is_empty() { return Err(format!("{file} ne contient aucune voix")); }
    let engine = loom_audio::Engine::start()?;
    engine.load(&src);
    engine.play();
    println!("▶ {file} — {} voix sur {} (Ctrl+C pour arrêter)", program.voices.len(), engine.device_name());
    let mut last = -1;
    loop {
        std::thread::sleep(Duration::from_millis(20));
        let s = engine.status();
        if s.playing() && s.bar() != last {
            last = s.bar();
            let bar = s.song_bar();
            let section = program.section_at(bar).map(|x| format!(" · {x}")).unwrap_or_default();
            println!("  mesure {:>4}{section} · {:.0} bpm", last + 1, program.bar_bpm(bar));
        }
    }
}

fn render(file: &str, out: &str, bars: Option<u32>, sr: u32, target: Option<(f64, f64)>) -> Result<(), String> {
    let src = read(file)?;
    let program = loom_core::Program::parse(&src);
    let bars = bars.unwrap_or_else(|| loom_app::studio::stems::bars_to_render(&program));
    let mut samples = loom_audio::render(&src, bars, sr);
    use loom_app::studio::bs1770;
    if let Some((lufs, tp)) = target {
        let before = bs1770::measure(&samples, sr);
        if let Some(g) = bs1770::normalize_gain(&before, lufs, tp) {
            for s in &mut samples { *s *= g as f32; }
            println!("  gain {:+.1} dB vers {lufs} LUFS (crête vraie max {tp} dBTP)", 20.0 * g.log10());
        }
    }
    let format = loom_app::export::write_audio(std::path::Path::new(out), &samples, sr)?;
    println!("{bars} mesures → {out} ({:.1} s, {format}, {sr} Hz)", samples.len() as f32 / 2.0 / sr as f32);
    let report = bs1770::measure(&samples, sr);
    println!("  {}", report.text());
    if let Some(advice) = report.against(if target.is_some_and(|(l, _)| l > -12.0) { "club" } else { "spotify" }) { println!("  {advice}"); }
    Ok(())
}

/// `loudness`: the mastering report of any audio file (WAV, or anything afconvert/ffmpeg reads).
/// Réharmonise la ligne `prog:` et écrit le morceau sur la sortie standard.
fn reharm(file: &str, how: &str) -> Result<(), String> {
    use loom_core::harmony::{reharm_source, Reharm};
    let how = Reharm::from_name(how).ok_or(format!("réharmonisation inconnue : {how} (jazz, tritone, iiv)"))?;
    let src = std::fs::read_to_string(file).map_err(|e| format!("{file} : {e}"))?;
    print!("{}", reharm_source(&src, how)?);
    Ok(())
}

fn loudness(file: &str, target: Option<&str>) -> Result<(), String> {
    use loom_app::studio::bs1770;
    let data = loom_audio::sample::SampleData::from_file(std::path::Path::new(file))
        .map_err(|e| format!("{file} : {e} (donne un WAV)"))?;
    let right = data.channels.get(1).unwrap_or(&data.channels[0]);
    let stereo: Vec<f32> = data.channels[0].iter().zip(right).flat_map(|(a, b)| [*a, *b]).collect();
    let report = bs1770::measure(&stereo, data.sample_rate as u32);
    println!("{file}\n  {}", report.text());
    let names: Vec<&str> = match target { Some(t) => vec![t], None => bs1770::TARGETS.iter().map(|t| t.0).collect() };
    for n in names {
        match report.against(n) {
            Some(a) => println!("  {a}"),
            None => return Err(format!("cible inconnue : {n}")),
        }
    }
    Ok(())
}

/// `bake`: the program as MIDI notes per voice (JSON), for `tools/bake_live.py`.
#[cfg(target_os = "macos")]
fn plugins() -> Result<(), String> {
    let all = loom_audio::au::list();
    for kind in [true, false] {
        println!("{}", if kind { "instruments :" } else { "effets :" });
        for a in all.iter().filter(|a| a.instrument == kind) { println!("  {}", a.name); }
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn plugins() -> Result<(), String> { Err("les Audio Units n'existent que sur macOS".into()) }

#[cfg(target_os = "macos")]
fn au(plugin: &str, file: &str, out: &str, voice: Option<&str>, bars: Option<u32>) -> Result<(), String> {
    use loom_audio::au::{find, render_notes, AuInstrument, AuNote};
    let info = find(plugin).ok_or(format!("aucun Audio Unit ne s'appelle « {plugin} » (voir loom plugins)"))?;
    let src = read(file)?;
    let program = loom_core::Program::parse(&src);
    let bars = bars.unwrap_or_else(|| loom_audio::bake::default_bars(&program));
    let baked = loom_audio::bake::bake(&src, bars);
    // du temps (en temps musicaux) aux secondes, mesure par mesure (tempo qui change compris)
    let mb = baked.meter_beats;
    let mut starts = vec![0.0f64];
    for bpm in &baked.bar_bpm { starts.push(starts.last().unwrap() + mb * 60.0 / bpm); }
    let sec = |beat: f64| {
        let k = ((beat / mb).floor().max(0.0) as usize).min(baked.bar_bpm.len().saturating_sub(1));
        starts[k] + (beat - k as f64 * mb) * 60.0 / baked.bar_bpm.get(k).copied().unwrap_or(120.0)
    };
    let tracks: Vec<_> = baked.tracks.iter().filter(|t| match voice { Some(v) => t.name == v, None => !t.drum }).collect();
    if tracks.is_empty() { return Err(match voice { Some(v) => format!("pas de voix « {v} » dans {file}"), None => format!("{file} n'a pas de voix mélodique") }); }
    let notes: Vec<AuNote> = tracks.iter().flat_map(|t| t.notes.iter()).map(|n| AuNote { on: sec(n.beat), off: sec(n.beat + n.dur), midi: n.pitch, vel: n.vel }).collect();
    let rate = 48_000.0;
    let mut inst = AuInstrument::open(&info, rate)?;
    // deux secondes de plus pour les queues de son
    let (l, r) = render_notes(&mut inst, &notes, starts.last().copied().unwrap_or(0.0) + 2.0)?;
    let stereo: Vec<f32> = l.iter().zip(&r).flat_map(|(a, b)| [*a, *b]).collect();
    loom_audio::wavout::write_wav_as(std::path::Path::new(out), &stereo, rate as u32, loom_audio::WavFormat::Pcm24).map_err(|e| e.to_string())?;
    let peak = stereo.iter().fold(0.0f32, |m, x| m.max(x.abs()));
    println!("{} · {} voix · {} notes · {bars} mesures → {out} ({:.1} s, crête {:.1} dBFS)",
        info.name, tracks.len(), notes.len(), l.len() as f64 / rate, 20.0 * peak.max(1e-9).log10());
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn au(_: &str, _: &str, _: &str, _: Option<&str>, _: Option<u32>) -> Result<(), String> { Err("les Audio Units n'existent que sur macOS".into()) }

fn bake(file: &str, out: &str, bars: Option<u32>) -> Result<(), String> {
    let src = read(file)?;
    let program = loom_core::Program::parse(&src);
    if program.voices.is_empty() { return Err(format!("{file} ne contient aucune voix")); }
    let bars = bars.unwrap_or_else(|| loom_audio::bake::default_bars(&program));
    let lower = out.to_lowercase();
    if lower.ends_with(".musicxml") || lower.ends_with(".xml") || lower.ends_with(".ly") {
        let title = std::path::Path::new(file).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let parts = loom_app::score::write(&src, &title, bars, std::path::Path::new(out))?;
        println!("{bars} mesures · {parts} portées → {out} (partition)");
        return Ok(());
    }
    let baked = loom_audio::bake::bake(&src, bars);
    // .mid = fichier MIDI standard pour n'importe quel DAW ; sinon le JSON de bake_live.py
    let octets = if out.to_lowercase().ends_with(".mid") { baked.to_midi() } else { baked.to_json().into_bytes() };
    std::fs::write(out, octets).map_err(|e| format!("impossible d'écrire {out} : {e}"))?;
    let notes: usize = baked.tracks.iter().map(|t| t.notes.len()).sum();
    println!("{bars} mesures · {} pistes · {notes} notes{} → {out}",
        baked.tracks.len(), if baked.ascl.is_some() { " · accordage" } else { "" });
    Ok(())
}

/// `--stems`: every voice to its own WAV in `dir` (the STUDIO's STEMS, from the command line).
fn render_stems(file: &str, dir: &str, bars: Option<u32>, sr: u32) -> Result<(), String> {
    let src = read(file)?;
    let program = loom_core::Program::parse(&src);
    if program.voices.is_empty() { return Err(format!("{file} ne contient aucune voix")); }
    let bars = bars.unwrap_or_else(|| loom_app::studio::stems::bars_to_render(&program));
    let dir = std::path::Path::new(dir);
    std::fs::create_dir_all(dir).map_err(|e| format!("impossible de créer le dossier {} : {e}", dir.display()))?;
    let stems = loom_audio::render_stems(&src, bars, sr);
    for (voice, samples) in &stems {
        let path = loom_app::studio::stems::stem_path(dir, voice);
        loom_audio::write_wav(&path, samples, sr).map_err(|e| format!("{} : {e}", path.display()))?;
        let peak = samples.iter().fold(0.0f32, |m, x| m.max(x.abs()));
        println!("  {voice:<12} → {} (crête {peak:.2})", path.display());
    }
    let seconds = stems.first().map_or(0.0, |(_, s)| s.len() as f32 / 2.0 / sr as f32);
    println!("{bars} mesures · {} pistes → {} ({seconds:.1} s chacune)", stems.len(), dir.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render_args(list: &[&str]) -> Result<Command, String> {
        let mut args = vec!["render"];
        args.extend_from_slice(list);
        parse(&args)
    }

    #[test]
    fn command_line() {
        assert_eq!(parse(&[]), Ok(Command::Tui(None)));
        assert_eq!(parse(&["techno.loom"]), Ok(Command::Tui(Some(PathBuf::from("techno.loom")))));
        assert_eq!(parse(&["--help"]), Ok(Command::Help));
        assert_eq!(parse(&["play", "a.loom"]), Ok(Command::Play("a.loom".into())));
        assert!(parse(&["--vite"]).is_err());
        assert!(USAGE.contains("--stems"));
    }

    #[test]
    fn render_and_stems() {
        let render = |out: &str, bars, stems| Ok(Command::Render { file: "a.loom".into(), out: out.into(), bars, stems, format: None, sr: 48_000, target: None });
        assert_eq!(render_args(&["a.loom", "a.wav"]), render("a.wav", None, false));
        assert_eq!(render_args(&["a.loom", "a.wav", "16"]), render("a.wav", Some(16), false));
        assert_eq!(render_args(&["a.loom", "pistes", "--stems"]), render("pistes", None, true));
        assert_eq!(render_args(&["a.loom", "pistes", "--stems", "4"]), render("pistes", Some(4), true));
        assert_eq!(render_args(&["--stems", "a.loom", "pistes", "4"]), render("pistes", Some(4), true));
        assert_eq!(render_args(&["--help"]), Ok(Command::RenderHelp));
        assert_eq!(render_args(&["a.loom", "a.wav", "16", "--32f", "--sr", "96000", "--target", "apple"]),
            Ok(Command::Render { file: "a.loom".into(), out: "a.wav".into(), bars: Some(16), stems: false, format: Some(loom_audio::WavFormat::Float32), sr: 96_000, target: Some((-16.0, -1.0)) }));
        assert!(render_args(&["a.loom", "a.wav", "--target", "radio"]).is_err());
        assert_eq!(parse(&["reharm", "a.loom"]), Ok(Command::Reharm { file: "a.loom".into(), how: "jazz".into() }));
        assert_eq!(parse(&["loudness", "a.wav", "club"]), Ok(Command::Loudness { file: "a.wav".into(), target: Some("club".into()) }));
        assert!(render_args(&["a.loom", "a.wav", "--sr", "12"]).is_err());
        assert_eq!(parse(&["plugins"]), Ok(Command::Plugins));
        assert_eq!(parse(&["au", "DLS", "a.loom", "a.wav", "lead", "8"]), Ok(Command::Au { plugin: "DLS".into(), file: "a.loom".into(), out: "a.wav".into(), voice: Some("lead".into()), bars: Some(8) }));
        assert_eq!(parse(&["au", "DLS", "a.loom", "a.wav"]), Ok(Command::Au { plugin: "DLS".into(), file: "a.loom".into(), out: "a.wav".into(), voice: None, bars: None }));
        assert_eq!(parse(&["bake", "a.loom", "a.json"]), Ok(Command::Bake { file: "a.loom".into(), out: "a.json".into(), bars: None }));
        assert_eq!(parse(&["bake", "a.loom", "a.json", "16"]), Ok(Command::Bake { file: "a.loom".into(), out: "a.json".into(), bars: Some(16) }));
        assert!(parse(&["bake", "a.loom", "a.json", "0"]).is_err());
        assert_eq!(render_args(&["a.loom", "a.wav", "-h"]), Ok(Command::RenderHelp));
        assert!(RENDER_USAGE.contains("--stems"));
        for bad in [&["a.loom"][..], &["a.loom", "a.wav", "zéro"], &["a.loom", "a.wav", "0"], &["a.loom", "a.wav", "--stem"], &["a", "b", "1", "2"]] {
            assert!(render_args(bad).is_err(), "{bad:?}");
        }
    }
}
