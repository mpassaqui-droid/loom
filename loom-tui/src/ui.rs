//! Drawing: transport bar, highlighted editor, live step grid, popups.

use crate::app::{App, Mode};
use crate::{studio_view, teacher_view};
use loom_app::{grid, Cell, Rgb};
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph};
use ratatui::Frame;
use std::time::Duration;

const GRID_STEPS: usize = 16;

pub(crate) fn c(rgb: Rgb) -> Color { Color::Rgb(rgb.0, rgb.1, rgb.2) }

pub fn draw(f: &mut Frame, app: &mut App) {
    let t = app.theme.clone();
    let base = Style::new().bg(c(t.background)).fg(c(t.foreground));
    f.render_widget(Block::new().style(base), f.area());
    if matches!(app.mode, Mode::Stage) {
        draw_stage(f, app);
        return;
    }

    // the arrangement line shows when the code has a `song:`
    let has_song = !loom_app::studio::songline::sections(&app.editor.text()).is_empty();
    let [top, body, timeline, bottom] = Layout::vertical([
        Constraint::Length(1), Constraint::Min(3), Constraint::Length(u16::from(has_song)), Constraint::Length(1),
    ]).areas(f.area());
    draw_transport(f, app, top);

    let grid_width = (GRID_STEPS as u16 * 2 + 14).min(body.width / 2);
    let [code, side] = Layout::horizontal([Constraint::Min(20), Constraint::Length(grid_width)]).areas(body);
    draw_editor(f, app, code);
    if app.studio.show_notation { studio_view::draw_notation(f, app, side) }
    else if app.studio.show_roll { studio_view::draw_roll(f, app, side) } else { draw_grid(f, app, side) }
    if matches!(app.mode, Mode::Mixer { .. }) { studio_view::draw_mixer(f, app, body); }
    if has_song { studio_view::draw_timeline(f, app, timeline); }
    draw_status(f, app, bottom);

    match &app.mode {
        Mode::Help { .. } => draw_help(f, app),
        Mode::Browser { .. } => draw_browser(f, app),
        Mode::Confirm { question, .. } => {
            let q = question.clone();
            draw_confirm(f, app, &q)
        }
        Mode::Prof { .. } => teacher_view::draw_prof(f, app),
        Mode::Ear => teacher_view::draw_ear(f, app),
        Mode::Hum => teacher_view::draw_hum(f, app),
        Mode::Lessons { .. } => teacher_view::draw_lessons(f, app),
        Mode::Transform { .. } => teacher_view::draw_transforms(f, app),
        Mode::Sounds { .. } => draw_sounds(f, app),
        Mode::Edit | Mode::Vibe { .. } | Mode::Stage | Mode::Mixer { .. } | Mode::Timeline { .. } | Mode::Import { .. } => {}
    }
}

fn draw_transport(f: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    let accent = Style::new().fg(c(t.accent)).add_modifier(Modifier::BOLD);
    let dim = Style::new().fg(c(t.dim));
    let mut spans = vec![Span::styled(" LOOM ", Style::new().bg(c(t.accent)).fg(c(t.background)).add_modifier(Modifier::BOLD)), Span::raw(" ")];
    if let Ok(engine) = &app.engine {
        let s = engine.status();
        if s.playing() {
            let bar = s.song_bar();
            spans.push(Span::styled("▶ ", accent));
            spans.push(Span::raw(format!("mesure {:>3}", s.bar() + 1)));
            if let Some(sec) = app.program.section_at(bar) {
                spans.push(Span::styled(" · ", dim));
                spans.push(Span::styled(sec.to_string(), accent));
            }
            spans.push(Span::styled(" · ", dim));
            spans.push(Span::raw(format!("{:.0} bpm", app.program.bar_bpm(bar))));
            // beat dots: which beat of the bar we're on
            let beats = app.program.meter_beats.round().clamp(1.0, 16.0) as usize;
            let beat = (s.phase() * beats as f32) as usize;
            spans.push(Span::raw("  "));
            for b in 0..beats {
                spans.push(Span::styled(if b == beat { "●" } else { "·" }, if b == beat { accent } else { dim }));
            }
        } else {
            spans.push(Span::styled("■ arrêté", dim));
        }
        // output level
        let level = (s.peak() * 8.0).round() as usize;
        spans.push(Span::styled("  niveau ", dim));
        spans.push(Span::styled("▮".repeat(level.min(8)), Style::new().fg(c(if level >= 8 { t.error } else { t.note }))));
        spans.push(Span::styled("▯".repeat(8 - level.min(8)), dim));
        spans.push(Span::styled(format!(" vol {:.0}%", app.volume * 100.0), dim));
        spans.extend(studio_view::transport_spans(app));
        if app.is_recording() { spans.push(Span::styled("  ● REC", Style::new().fg(c(t.error)).add_modifier(Modifier::BOLD))); }
        if app.metronome { spans.push(Span::styled("  ♩", accent)); }
        if app.lien { spans.push(Span::styled("  ⇄ lié", accent)); }
        if app.midi_clock { spans.push(Span::styled("  MIDI→", accent)); }
        if app.midi_follow { spans.push(Span::styled("  →MIDI", accent)); }
    }
    spans.push(Span::styled("  ", dim));
    spans.push(Span::styled(app.file_label(), Style::new().fg(c(t.foreground))));
    if let Some(label) = app.teacher.history.undo_label() {
        spans.push(Span::styled(format!("  F3 ↩ {label}"), dim));
    }
    if let Some(label) = app.teacher.history.redo_label() {
        spans.push(Span::styled(format!("  ^Y rétablir {label}"), dim));
    }
    if app.out_of_sync() {
        spans.push(Span::styled("  ⟳ modifié — Ctrl+R", Style::new().fg(c(t.accent))));
    }
    f.render_widget(Paragraph::new(Line::from(spans)).style(Style::new().bg(c(t.panel))), area);
}

/// The full-screen SCÈNE (F12), with the title, section and bar on top.
fn draw_stage(f: &mut Frame, app: &mut App) {
    let mut position = String::from("arrêté");
    if let Ok(engine) = &app.engine {
        let s = engine.status();
        if s.playing() {
            position = format!("mesure {}", s.bar() + 1);
            if let Some(section) = app.program.section_at(s.song_bar()) {
                position = format!("{section} · {position}");
            }
        }
    }
    let file = app.path.as_ref().and_then(|p| p.file_stem()).map(|n| n.to_string_lossy().into_owned());
    let title = app.stage.director.title.clone().or(file).unwrap_or_default();
    let theme = app.theme.clone();
    let area = f.area();
    app.stage.draw(f, area, &theme, &title, &position);
}

fn is_name(s: &str) -> bool {
    !s.is_empty() && !s.contains(char::is_whitespace)
}

/// Colour one line of LOOM source: comments, names/directives, verbs, hits.
pub(crate) fn highlight<'a>(line: &'a str, app: &App) -> Vec<Span<'a>> {
    let t = &app.theme;
    let (code, comment) = match line.find('#') { Some(i) => line.split_at(i), None => (line, "") };
    let mut spans = Vec::new();
    // `name:` / `bass@chorus:` / `bpm:` — a single word before the colon
    let (head, rest) = match code.find(':') {
        Some(i) if is_name(code[..i].trim()) => code.split_at(i + 1),
        _ => ("", code),
    };
    if !head.is_empty() { spans.push(Span::styled(head, Style::new().fg(c(t.accent)).add_modifier(Modifier::BOLD))); }
    let (pattern, verbs) = match rest.find('|') { Some(i) => rest.split_at(i), None => (rest, "") };
    // pattern: hits stand out, rests fade
    let mut word = String::new();
    let flush = |word: &mut String, spans: &mut Vec<Span<'a>>| {
        if word.is_empty() { return; }
        let style = match word.as_str() {
            "x" | "X" => Style::new().fg(c(t.drum)).add_modifier(Modifier::BOLD),
            "." | "~" => Style::new().fg(c(t.dim)),
            _ => Style::new().fg(c(t.foreground)),
        };
        spans.push(Span::styled(std::mem::take(word), style));
    };
    for ch in pattern.chars() {
        if ch.is_whitespace() { flush(&mut word, &mut spans); spans.push(Span::raw(ch.to_string())); } else { word.push(ch); }
    }
    flush(&mut word, &mut spans);
    if !verbs.is_empty() { spans.push(Span::styled(verbs, Style::new().fg(c(t.note)))); }
    if !comment.is_empty() { spans.push(Span::styled(comment, Style::new().fg(c(t.dim)).add_modifier(Modifier::ITALIC))); }
    spans
}

fn draw_editor(f: &mut Frame, app: &mut App, area: Rect) {
    let t = app.theme.clone();
    let title = if app.out_of_sync() { " code · modifié " } else { " code " };
    let block = Block::new().borders(Borders::ALL).border_type(BorderType::Rounded)
        .border_style(Style::new().fg(c(if matches!(app.mode, Mode::Edit) { t.accent } else { t.dim })))
        .title(Span::styled(title, Style::new().fg(c(t.foreground))));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let height = inner.height as usize;
    app.editor_height = height;
    app.editor.scroll_to_cursor(height);
    let gutter = 4u16;
    let scroll = app.editor.scroll;
    let lines: Vec<Line> = app.editor.lines.iter().enumerate().skip(scroll).take(height).map(|(i, l)| {
        let fautive = app.lints.iter().any(|x| x.line == i);
        let mut spans = if fautive {
            vec![Span::styled(format!("{:>3}!", i + 1), Style::new().fg(c(t.error)).add_modifier(Modifier::BOLD))]
        } else {
            vec![Span::styled(format!("{:>3} ", i + 1), Style::new().fg(c(t.dim)))]
        };
        if fautive {
            let rouge = Style::new().add_modifier(Modifier::UNDERLINED).underline_color(c(t.error));
            spans.extend(highlight(l, app).into_iter().map(|sp| sp.patch_style(rouge)));
        } else {
            spans.extend(highlight(l, app));
        }
        let line = Line::from(spans);
        if i == app.editor.row { line.style(Style::new().bg(c(t.panel))) } else { line }
    }).collect();
    f.render_widget(Paragraph::new(lines), inner);

    if matches!(app.mode, Mode::Edit) {
        let x = inner.x + gutter + app.editor.col as u16;
        let y = inner.y + (app.editor.row - scroll) as u16;
        if x < inner.right() && y < inner.bottom() { f.set_cursor_position(Position::new(x, y)); }
    }
}

fn draw_grid(f: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    let block = Block::new().borders(Borders::ALL).border_type(BorderType::Rounded)
        .border_style(Style::new().fg(c(t.dim)))
        .title(Span::styled(" grille · Alt+1..9 coupe ", Style::new().fg(c(t.foreground))));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let (playing, bar, phase) = match &app.engine {
        Ok(e) => (e.status().playing(), e.status().song_bar(), e.status().phase()),
        Err(_) => (false, 0, 0.0),
    };
    if app.program.voices.is_empty() {
        f.render_widget(Paragraph::new(vec![
            Line::from(""),
            Line::styled(" Ctrl+R pour jouer", Style::new().fg(c(t.accent))),
            Line::styled(" F1 pour l'aide", Style::new().fg(c(t.dim))),
        ]), inner);
        return;
    }
    let head = if playing { (phase * GRID_STEPS as f32) as usize } else { usize::MAX };
    let cell_w = if inner.width as usize >= 12 + GRID_STEPS * 2 { 2 } else { 1 };
    let name_w = (inner.width as usize).saturating_sub(GRID_STEPS * cell_w + 3).clamp(3, 9);
    let mut lines = Vec::new();
    for (i, row) in grid(&app.program, bar, GRID_STEPS).iter().enumerate() {
        let muted = app.muted.contains(&row.name);
        let colour = if muted { t.dim } else if row.is_drum { t.drum } else { t.note };
        let mut name: String = row.name.chars().take(name_w).collect();
        while name.chars().count() < name_w { name.push(' '); }
        let mut spans = vec![
            Span::styled(if i < 9 { format!("{}", i + 1) } else { " ".into() }, Style::new().fg(c(t.dim))),
            Span::raw(" "),
            Span::styled(name, Style::new().fg(c(if muted { t.dim } else { t.foreground })).add_modifier(if muted { Modifier::CROSSED_OUT } else { Modifier::empty() })),
            Span::raw(" "),
        ];
        for (col, cell) in row.cells.iter().enumerate() {
            let (glyph, mut style) = match cell {
                Cell::Hit(v) if *v > 1.0 => ("█", Style::new().fg(c(colour))),
                Cell::Hit(_) => ("■", Style::new().fg(c(colour))),
                Cell::Rest if col % 4 == 0 => ("·", Style::new().fg(c(t.dim))),
                Cell::Rest => ("·", Style::new().fg(c(t.background.mix(t.dim, 0.5)))),
            };
            if col == head { style = style.bg(c(t.selection)); }
            let text = if cell_w == 2 { format!("{glyph} ") } else { glyph.to_string() };
            spans.push(Span::styled(text, style));
        }
        lines.push(Line::from(spans));
    }
    f.render_widget(Paragraph::new(lines), inner);
}

fn draw_status(f: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    let hints = " ^R jouer  Échap stop  F2 vibe/?prof  Alt+G générer  Alt+L leçons  Alt+O oreille  F3 annuler  Alt+M mix  ^S enregistrer  ^O ouvrir  F1 aide  ^Q quitter ";
    let accent = Style::new().fg(c(t.accent)).add_modifier(Modifier::BOLD);
    if matches!(app.mode, Mode::Import { .. }) {
        studio_view::draw_import(f, app, area);
        return;
    }
    // the VIBE prompt takes over the status line while typing…
    if let Mode::Vibe { input } = &app.mode {
        let label = " VIBE › ";
        let help = "   Entrée envoie · Ctrl+D dicte · ?question = le prof · Échap ferme";
        let mut spans = vec![Span::styled(label, accent), Span::raw(input.clone())];
        if let Some(d) = app.dictee_label() {
            spans.push(Span::styled(format!("  {d}"), Style::new().fg(c(t.error)).add_modifier(Modifier::BOLD)));
        } else if input.is_empty() {
            // les puces du web, une touche chacune
            for (i, (label, _)) in loom_app::vibe::CHIPS.iter().enumerate() {
                spans.push(Span::styled(format!("{}", (i + 1) % 10), accent));
                spans.push(Span::styled(format!(" {label}  "), Style::new().fg(c(t.dim))));
            }
            spans.push(Span::styled("· ou décris la musique", Style::new().fg(c(t.dim))));
        }
        spans.push(Span::styled(help, Style::new().fg(c(t.dim))));
        f.render_widget(Paragraph::new(Line::from(spans)).style(Style::new().bg(c(t.panel))), area);
        let x = area.x + (label.chars().count() + input.chars().count()) as u16;
        if x < area.right() { f.set_cursor_position(Position::new(x, area.y)); }
        return;
    }
    // …and shows a spinner while Claude thinks
    if let Some(job) = &app.vibe {
        const SPIN: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let elapsed = job.elapsed();
        let frame = SPIN[(elapsed.as_millis() / 100) as usize % SPIN.len()];
        let line = Line::from(vec![
            Span::styled(format!(" {frame} VIBE réfléchit… {} s ", elapsed.as_secs()), accent),
            Span::styled(format!("« {} »", job.words), Style::new().fg(c(t.foreground))),
            Span::styled("   Échap annule", Style::new().fg(c(t.dim))),
        ]);
        f.render_widget(Paragraph::new(line).style(Style::new().bg(c(t.panel))), area);
        return;
    }
    // …and the teacher's spinner, while the answer popup is closed
    if let (Some(line), false) = (teacher_view::thinking_line(app), matches!(app.mode, Mode::Prof { .. })) {
        f.render_widget(Paragraph::new(line).style(Style::new().bg(c(t.panel))), area);
        return;
    }
    let line = match &app.message {
        Some((msg, err, at)) if at.elapsed() < Duration::from_secs(6) => {
            Line::from(vec![Span::styled(format!(" {msg} "), Style::new().fg(c(if *err { t.error } else { t.foreground })))])
        }
        _ => Line::styled(hints, Style::new().fg(c(t.dim))),
    };
    f.render_widget(Paragraph::new(line).style(Style::new().bg(c(t.panel))), area);
}

pub(crate) fn popup(f: &mut Frame, app: &App, title: &str, w: u16, h: u16) -> Rect {
    let area = f.area();
    let (w, h) = (w.min(area.width.saturating_sub(2)), h.min(area.height.saturating_sub(2)));
    let rect = Rect::new(area.x + (area.width - w) / 2, area.y + (area.height - h) / 2, w, h);
    f.render_widget(Clear, rect);
    let block = Block::new().borders(Borders::ALL).border_type(BorderType::Rounded)
        .border_style(Style::new().fg(c(app.theme.accent)))
        .style(Style::new().bg(c(app.theme.panel)).fg(c(app.theme.foreground)))
        .title(Span::styled(format!(" {title} "), Style::new().fg(c(app.theme.accent)).add_modifier(Modifier::BOLD)));
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    inner
}

fn draw_help(f: &mut Frame, app: &mut App) {
    let t = &app.theme;
    // sur Mac, Alt s'appelle Option (⌥)
    let key = |k: &str, what: &str| Line::from(vec![
        Span::styled(format!("  {:<17}", if cfg!(target_os = "macos") { k.replace("Alt+", "⌥") } else { k.to_string() }), Style::new().fg(c(t.accent))),
        Span::raw(what.to_string()),
    ]);
    let section = |s: &str| Line::styled(format!(" {s}"), Style::new().fg(c(t.dim)).add_modifier(Modifier::BOLD));
    let alt_note = if cfg!(target_os = "macos") {
        "⌥ = Option. Option ne marche pas ? Ctrl+G puis la lettre"
    } else {
        "Alt ne passe pas ? Ctrl+G puis la lettre fait pareil"
    };
    let lines = vec![
        Line::styled(format!(" {alt_note}"), Style::new().fg(c(t.accent)).add_modifier(Modifier::BOLD)),
        Line::from(""),
        section("Jouer"),
        key("Ctrl+R · F5", "jouer / appliquer le code (à la prochaine mesure)"),
        key("Échap · F6", "stop"),
        key("Alt+1…9", "couper / rétablir la voix n° N de la grille"),
        key("F7 · F8", "volume − / +"),
        key("F4", "REC : enregistrer ce qui joue (~/Music/loom/enregistrements)"),
        key("Ctrl+T", "métronome"),
        key("F9", "envoyer l'horloge MIDI (port « LOOM »)"),
        key("F10", "suivre une horloge MIDI externe"),
        key("clavier MIDI", "joue en direct avec le son de la voix lead"),
        Line::from(""),
        section("Scène (visuels)"),
        key("F12", "scène : visuels plein écran (Échap ou F12 pour revenir)"),
        key("  N · V", "scène suivante · mode VJ (change toutes les 4 mesures)"),
        key("scene: tunnel", "choisir la scène (scene@drop: tunnel par section)"),
        Line::from(""),
        section("Studio"),
        key("Alt+M", "mixeur : volume/pan/mute/solo/clip par voix + potards master"),
        key("  ↑↓ ←→ +/-", "choisir · volume ou pan · régler (m mute, s solo, c clip, l LEARN)"),
        key("Alt+A", "arrangement : ←→ section, +/- mesures, d dup, x suppr, < > déplacer"),
        key("Alt+R · Alt+T", "piano-roll · TAP tempo (réécrit bpm:)"),
        key("Alt+S", "STEMS : une piste WAV par voix dans <morceau>-stems/"),
        key("Alt+P · Alt+I", "partager (code LOOM1: → <morceau>.share.txt) · importer"),
        key("Alt+D", "DÉMO : visite des morceaux d'exemple (une touche arrête)"),
        Line::from(""),
        section("Fichiers"),
        key("Ctrl+O", "ouvrir un morceau"),
        key("Ctrl+S", "enregistrer (dans ~/Music/loom si nouveau)"),
        key("Ctrl+N", "nouveau morceau"),
        key("Ctrl+E", "exporter en WAV à côté du morceau"),
        key("Ctrl+Q", "quitter"),
        Line::from(""),
        section("IA — VIBE"),
        key("Alt+N / Alt+Z", "morceau suivant / au hasard (comme les presets du web)"),
        key("Ctrl+O puis Suppr", "mettre un de tes morceaux à la corbeille"),
        key("RUN", "le correcteur souligne en rouge les lignes fautives (n° suivi de !) et explique en bas"),
        key("Alt+V", "dicte à VIBE : parle, whisper écrit en local (Ctrl+D dans VIBE pour arrêter ou reprendre)"),
        key("Alt+U", "mes sons : importe tes fichiers audio (glisser-déposer), | sample <nom>, Suppr, Ctrl+X vide"),
        key("Alt+F", "partition : la mesure en cours sur une grande portée (clé de sol et de fa)"),
        key("Alt+C", "thème suivant : les 62 thèmes du web, gardé au prochain lancement"),
        key("Alt+K", "lien Live : chaque RUN enregistre le fichier, le device Loom de Live le suit"),
        key("Alt+X", "exporter .mid (pour un DAW) et .musicxml (partition pour MuseScore, Sibelius, Dorico)"),
        key("F2", "décrire la musique en mots, Claude écrit le code"),
        key("F2 puis 1..0", "les puces : + batterie, + basse, + nappe, + triste, + énergie, + exotique, varier, simplifier, continuer, corriger"),
        key("  Entrée · Échap", "envoyer · fermer (Échap annule s'il réfléchit)"),
        key("F2 ?question", "demander au prof : ?quinte ?raga ?pourquoi c'est triste"),
        key("F3 · Ctrl+Y", "annuler · rétablir VIBE/GÉNÉRER/DÉVELOPPE/HUM/leçon"),
        Line::from(""),
        section("Prof & composition"),
        key("Alt+G", "GÉNÉRER un morceau neuf (genre au hasard)"),
        key("Alt+B", "DÉVELOPPE : faire évoluer les voix (nouveau seed)"),
        key("Alt+E", "EXPLIQUE : le prof explique ton morceau"),
        key("Alt+O", "OREILLE : entraîner l'oreille (Tab écoute l'exemple)"),
        key("Alt+H", "HUM : chante 16 temps, LOOM écrit la mélodie"),
        key("Alt+L", "LEÇONS & RÉFÉRENCE (tape pour chercher)"),
        Line::from(""),
        section("Écrire"),
        key("Ctrl+/", "commenter / décommenter la ligne"),
        key("kick: x . x .", "une voix par ligne — x = coup, . = silence"),
        key("bass: c2 eb2", "notes, ou degrés 1 3 5 avec  key: c minor"),
        key("| rev | fast 2", "verbes : rev fast slow every euclid…"),
        Line::from(""),
        Line::styled("  ↑↓ PgUp PgDn défiler · autre touche : fermer · Alt+L : leçons et référence du langage", Style::new().fg(c(t.dim))),
    ];
    let total = lines.len();
    let inner = popup(f, app, "LOOM — aide", 80, total as u16 + 2);
    let Mode::Help { scroll } = &mut app.mode else { return };
    *scroll = teacher_view::clamp_scroll(*scroll, total, inner.height as usize);
    f.render_widget(Paragraph::new(lines).scroll((*scroll as u16, 0)), inner);
}

fn draw_browser(f: &mut Frame, app: &App) {
    let Mode::Browser { items, selected } = &app.mode else { return };
    let t = &app.theme;
    let area = f.area();
    let inner = popup(f, app, "ouvrir un morceau", 60, area.height.saturating_sub(4));
    let mut list_items = Vec::new();
    let mut last_label = "";
    let mut index_of_selected = 0;
    for (i, (label, path)) in items.iter().enumerate() {
        if label != last_label {
            list_items.push(ListItem::new(Line::styled(label.to_string(), Style::new().fg(c(t.dim)).add_modifier(Modifier::BOLD))));
            last_label = label;
        }
        if i == *selected { index_of_selected = list_items.len(); }
        let name = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        list_items.push(ListItem::new(format!("  {name}")));
    }
    let list = List::new(list_items).highlight_style(Style::new().bg(c(t.accent)).fg(c(t.background)).add_modifier(Modifier::BOLD));
    let mut state = ListState::default().with_selected(Some(index_of_selected));
    f.render_stateful_widget(list, inner, &mut state);
}

/// « Mes sons » (Alt+U).
fn draw_sounds(f: &mut Frame, app: &App) {
    let Mode::Sounds { input, selected, mine } = &app.mode else { return };
    let t = &app.theme;
    let dim = Style::new().fg(c(t.dim));
    let h = (mine.len() as u16 + 9).clamp(12, f.area().height.saturating_sub(4));
    let inner = popup(f, app, "mes sons · tes propres samples", 72, h);
    let [head, list, foot] = Layout::vertical([Constraint::Length(3), Constraint::Min(1), Constraint::Length(3)]).areas(inner);
    let cursor = if input.is_empty() { Span::styled("glisse un fichier audio ici, ou tape son chemin", dim) } else { Span::raw(input.clone()) };
    f.render_widget(Paragraph::new(vec![
        Line::from(vec![Span::styled(" fichier ▸ ", Style::new().fg(c(t.accent))), cursor]),
        Line::styled(format!(" fournis : {}", app.builtin_sounds().join(" ")), dim),
    ]).wrap(ratatui::widgets::Wrap { trim: false }), head);
    if mine.is_empty() {
        f.render_widget(Paragraph::new(Line::styled(" aucun pour l'instant : WAV, AIFF, MP3, M4A… puis | sample <nom>", dim)), list);
    } else {
        let items: Vec<ListItem> = mine.iter().map(|n| ListItem::new(format!("  ★ {n}"))).collect();
        let l = List::new(items).highlight_style(Style::new().bg(c(t.accent)).fg(c(t.background)).add_modifier(Modifier::BOLD));
        let mut state = ListState::default().with_selected(Some(*selected));
        f.render_stateful_widget(l, list, &mut state);
    }
    f.render_widget(Paragraph::new(vec![
        Line::styled(" Entrée : importe, ou ajoute | sample <nom> à la ligne du curseur", dim),
        Line::styled(" Suppr : ce son à la corbeille · Ctrl+X : VIDER · Échap : fermer", dim),
    ]), foot);
}

fn draw_confirm(f: &mut Frame, app: &App, question: &str) {
    let inner = popup(f, app, "attention", (question.chars().count() as u16 + 8).max(36), 5);
    f.render_widget(Paragraph::new(vec![
        Line::from(format!(" {question}")),
        Line::from(""),
        Line::styled(" o / Entrée = oui    autre touche = annuler", Style::new().fg(c(app.theme.dim))),
    ]), inner);
}
