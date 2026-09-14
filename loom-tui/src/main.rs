//! `loom` — LOOM in the terminal.
//!
//!   loom [song.loom]                     live-coding editor with native audio
//!   loom play song.loom                  play without the editor (Ctrl+C to stop)
//!   loom render song.loom out.wav [N]    bounce N bars (default: the song, or 8) to WAV
//!
//! Architecture: the audio thread lives inside `loom_audio::Engine` (cpal);
//! this thread draws with ratatui and sends it commands. Edits hot-swap at
//! the next bar line, so the music never stutters while you type.

mod app;
mod editor;
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
  loom [morceau.loom]                    éditeur live avec son natif
  loom play morceau.loom                 jouer sans l'éditeur (Ctrl+C pour arrêter)
  loom render morceau.loom sortie.wav [N]  exporter N mesures en WAV
  loom-gui [morceau.loom]                la même chose dans une fenêtre
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["-h" | "--help" | "help"] => { print!("{USAGE}"); Ok(()) }
        ["-V" | "--version"] => { println!("loom {}", env!("CARGO_PKG_VERSION")); Ok(()) }
        ["play", file] => play(file),
        ["render", file, out] => render(file, out, None),
        ["render", file, out, bars] => match bars.parse() {
            Ok(n) => render(file, out, Some(n)),
            Err(_) => Err(format!("nombre de mesures invalide : {bars}")),
        },
        [] => tui(None),
        [file] if !file.starts_with('-') => tui(Some(PathBuf::from(file))),
        _ => Err(format!("arguments non reconnus\n\n{USAGE}")),
    };
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

fn render(file: &str, out: &str, bars: Option<u32>) -> Result<(), String> {
    let src = read(file)?;
    let program = loom_core::Program::parse(&src);
    let bars = bars.unwrap_or(if program.len_bars() > 0 { program.len_bars() } else { 8 });
    let samples = loom_audio::render(&src, bars, 48000);
    loom_audio::write_wav(std::path::Path::new(out), &samples, 48000).map_err(|e| e.to_string())?;
    let peak = samples.iter().fold(0.0f32, |m, x| m.max(x.abs()));
    println!("{bars} mesures → {out} ({:.1} s, crête {:.2})", samples.len() as f32 / 2.0 / 48000.0, peak);
    Ok(())
}
