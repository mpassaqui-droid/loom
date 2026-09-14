//! LOOM in a native window: code editor, live step grid, transport, library.
//!
//! Usage: `loom-gui [morceau.loom]`
//!
//! - [`app`]: the window's state and what each frame draws and does.
//! - [`editor`]: syntax colouring for the code.
//! - [`grid`]: the step grid with its playhead.
//! - [`style`]: the Omarchy theme turned into egui colours.
//! - [`export`]: the background WAV export.

mod app;
mod editor;
mod export;
mod grid;
mod style;

use std::path::PathBuf;

use eframe::egui;

const USAGE: &str = "\
loom-gui — LOOM dans une fenêtre

Usage : loom-gui [morceau.loom]

Sans fichier, LOOM rouvre le dernier morceau (ou le morceau d'accueil).
Raccourcis : Ctrl+Entrée jouer · Échap/Ctrl+. arrêter · Ctrl+S enregistrer
             Ctrl+N nouveau · Ctrl+Maj+E exporter en WAV";

/// What the command line asked for.
#[derive(Debug, PartialEq)]
enum Command {
    /// Open the window, with this song if one was given.
    Open(Option<PathBuf>),
    Help,
}

/// Read the command-line arguments (without the program name).
fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Command, String> {
    let mut file = None;
    for arg in args {
        match arg.as_str() {
            "-h" | "--help" => return Ok(Command::Help),
            flag if flag.starts_with('-') => return Err(format!("option inconnue : {flag}")),
            _ if file.is_some() => return Err("un seul fichier à la fois".into()),
            path => file = Some(PathBuf::from(path)),
        }
    }
    Ok(Command::Open(file))
}

fn main() -> eframe::Result {
    let file = match parse_args(std::env::args().skip(1)) {
        Ok(Command::Open(file)) => file,
        Ok(Command::Help) => {
            println!("{USAGE}");
            return Ok(());
        }
        Err(e) => {
            eprintln!("loom-gui : {e}\n\n{USAGE}");
            std::process::exit(2);
        }
    };

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            // "loom" lets Hyprland rules target the window (class/app_id)
            .with_app_id("loom")
            .with_title("loom")
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([640.0, 420.0]),
        ..Default::default()
    };
    eframe::run_native("loom", options, Box::new(|cc| Ok(Box::new(app::LoomApp::new(cc, file)))))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Result<Command, String> {
        parse_args(list.iter().map(|s| s.to_string()))
    }

    #[test]
    fn command_line() {
        assert_eq!(args(&[]), Ok(Command::Open(None)));
        assert_eq!(args(&["techno.loom"]), Ok(Command::Open(Some(PathBuf::from("techno.loom")))));
        assert_eq!(args(&["--help"]), Ok(Command::Help));
        assert!(args(&["a.loom", "b.loom"]).is_err());
        assert!(args(&["--vite"]).is_err());
    }
}
