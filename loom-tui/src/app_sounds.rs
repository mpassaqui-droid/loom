//! « Mes sons » (Alt+U), comme la carte du web : importer ses fichiers audio
//! (glisser-déposer depuis le Finder, ou taper un chemin), les insérer dans le
//! code avec `| sample NOM`, en jeter un, tout vider. Les fichiers vivent dans
//! `~/.local/share/loom/samples/` ; le moteur les entend tout de suite.

use super::{App, Mode, Pending};
use loom_app::samples;
use ratatui::crossterm::event::{KeyCode, KeyEvent};

impl App {
    pub fn open_sounds(&mut self) {
        self.mode = Mode::Sounds { input: String::new(), selected: 0, mine: samples::list() };
    }

    /// Les sons fournis (pas les miens), pour la ligne d'aide et contre les doublons.
    pub fn builtin_sounds(&self) -> Vec<String> {
        let mine = samples::list();
        loom_audio::sample::merged_bank().names().into_iter().filter(|n| !mine.contains(n)).collect()
    }

    fn reload_sounds(&mut self, selected: usize) {
        if let Ok(e) = &self.engine { e.reload_samples(); }
        let mine = samples::list();
        let selected = selected.min(mine.len().saturating_sub(1));
        self.mode = Mode::Sounds { input: String::new(), selected, mine };
    }

    pub(super) fn sounds_key(&mut self, key: KeyEvent, ctrl: bool) {
        let Mode::Sounds { input, selected, mine } = &mut self.mode else { return };
        match key.code {
            KeyCode::Esc => self.mode = Mode::Edit,
            KeyCode::Up => *selected = selected.saturating_sub(1),
            KeyCode::Down => *selected = (*selected + 1).min(mine.len().saturating_sub(1)),
            KeyCode::Enter if !input.trim().is_empty() => {
                let paths = samples::split_paths(input);
                let sel = *selected;
                self.import_sounds(&paths, sel);
            }
            KeyCode::Enter => {
                let Some(name) = mine.get(*selected).cloned() else { return };
                self.mode = Mode::Edit;
                let row = self.editor.row;
                if let Some(line) = self.editor.lines.get_mut(row) {
                    if line.trim().is_empty() { line.push_str(&format!("{name}: x . . . | sample {name}")); }
                    else { line.push_str(&format!(" | sample {name}")); }
                    self.editor.col = line.chars().count();
                    self.editor.dirty = true;
                }
                self.info(format!("| sample {name} ajouté à la ligne {}", row + 1));
            }
            KeyCode::Delete | KeyCode::Backspace if input.is_empty() => {
                if let Some(name) = mine.get(*selected).cloned() {
                    self.guarded(Pending::TrashSound(name));
                }
            }
            KeyCode::Backspace => { input.pop(); }
            KeyCode::Char('x') if ctrl && !mine.is_empty() => self.guarded(Pending::EmptySounds),
            KeyCode::Char('u') if ctrl => input.clear(),
            KeyCode::Char(c) if !ctrl => input.push(c),
            _ => {}
        }
    }

    fn import_sounds(&mut self, paths: &[std::path::PathBuf], selected: usize) {
        let builtin = self.builtin_sounds();
        let (mut ok, mut errs) = (Vec::new(), Vec::new());
        for p in paths {
            match samples::import(p, &builtin) {
                Ok(n) => ok.push(n),
                Err(e) => errs.push(e),
            }
        }
        self.reload_sounds(selected);
        if let (Mode::Sounds { selected, mine, .. }, Some(last)) = (&mut self.mode, ok.last()) {
            *selected = mine.iter().position(|n| n == last).unwrap_or(0);
        }
        match (ok.is_empty(), errs.is_empty()) {
            (_, false) => self.error(format!("importé : {} · {}", if ok.is_empty() { "rien".into() } else { ok.join(", ") }, errs.join(" · "))),
            (false, true) => self.info(format!("importé : {} · Entrée l'ajoute au code (| sample {})", ok.join(", "), ok[0])),
            (true, true) => {}
        }
    }

    pub(super) fn trash_sound(&mut self, name: &str) {
        match samples::trash(name) {
            Ok(()) => self.info(format!("« {name} » à la corbeille")),
            Err(e) => self.error(format!("corbeille : {e}")),
        }
        self.reload_sounds(0);
    }

    pub(super) fn empty_sounds(&mut self) {
        let mine = samples::list();
        let errs: Vec<String> = mine.iter().filter_map(|n| samples::trash(n).err()).collect();
        if errs.is_empty() { self.info(format!("{} son{} à la corbeille", mine.len(), if mine.len() > 1 { "s" } else { "" })); } else { self.error(errs.join(" · ")); }
        self.reload_sounds(0);
    }
}
