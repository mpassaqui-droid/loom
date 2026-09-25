//! Drawing the STUDIO in the terminal: the MIX view, the arrangement line,
//! the piano roll, the import prompt and the transport's extra meters.

use crate::app::{App, MixRow, Mode};
use crate::scene::hue_color;
use crate::scene::logic::voice_hue;
use loom_app::studio::knobs::Knob;
use loom_app::studio::loudness::Level;
use loom_app::studio::mixer::Param;
use loom_app::studio::{roll, songline};
use loom_app::Rgb;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;

fn c(rgb: Rgb) -> Color { Color::Rgb(rgb.0, rgb.1, rgb.2) }

/// `███████···` — a level from 0 to 1 in `width` cells.
fn gauge(fraction: f32, width: usize) -> String {
    let full = ((fraction.clamp(0.0, 1.0) * width as f32).round() as usize).min(width);
    format!("{}{}", "█".repeat(full), "·".repeat(width - full))
}

/// `···|·····` with the marker at `pan` (-1..1).
fn pan_gauge(pan: f32, width: usize) -> String {
    let at = (((pan.clamp(-1.0, 1.0) + 1.0) / 2.0) * (width - 1) as f32).round() as usize;
    (0..width).map(|i| if i == at { '|' } else { '·' }).collect()
}

/// Extra spans for the transport bar: loudness, stems progress, clips, demo.
pub fn transport_spans(app: &App) -> Vec<Span<'static>> {
    let t = &app.theme;
    let dim = Style::new().fg(c(t.dim));
    let loudness = &app.studio.loudness;
    let tint = match loudness.level() { Level::Clipping => t.error, Level::Loud => t.drum, Level::Normal => t.dim };
    let mut spans = vec![Span::styled(format!("  {}", loudness.text()), Style::new().fg(c(tint)))];
    if let Some(job) = &app.studio.stems {
        spans.push(Span::styled(format!("  ⬇ {}", job.progress_text()), Style::new().fg(c(t.accent))));
    }
    if app.studio.learn.on {
        spans.push(Span::styled("  LEARN", Style::new().fg(c(t.accent))));
    }
    if !app.studio.clips.is_empty() {
        spans.push(Span::styled("  clips", dim));
    }
    spans
}

/// The MIX view (Alt+M) in place of the editor and grid.
pub fn draw_mixer(f: &mut Frame, app: &App, area: Rect) {
    let Mode::Mixer { row: selected, param } = app.mode else { return };
    let t = &app.theme;
    f.render_widget(Clear, area);
    let block = Block::new().borders(Borders::ALL).border_type(BorderType::Rounded)
        .border_style(Style::new().fg(c(t.accent)))
        .style(Style::new().bg(c(t.background)).fg(c(t.foreground)))
        .title(Span::styled(" MIX · ↑↓ choisir · ←→ volume/pan · +/- régler · m mute · s solo · c clip · l LEARN · Échap ", Style::new().fg(c(t.accent)).add_modifier(Modifier::BOLD)));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let dim = Style::new().fg(c(t.dim));
    let accent = Style::new().fg(c(t.accent)).add_modifier(Modifier::BOLD);
    let strips = app.studio.mixer.visible();
    let mut lines = Vec::new();
    lines.push(Line::styled("  voix        niveau        volume              pan", dim));
    if strips.is_empty() {
        lines.push(Line::styled("  aucune voix — écris une voix, puis Ctrl+R", dim));
    }
    for (i, s) in strips.iter().enumerate() {
        let is_row = selected == i;
        let muted = app.muted.contains(&s.name);
        let hue = hue_color(voice_hue(&s.name, None), 0.7);
        let name: String = format!("{:<10}", s.name.chars().take(10).collect::<String>());
        let meter_colour = if s.meter > 1.0 { t.error } else if s.meter > 0.6 { t.drum } else { t.note };
        let pick = |p: Param| if is_row && param == p { accent } else { Style::new().fg(c(t.foreground)) };
        let mut spans = vec![
            Span::styled(if is_row { "▸ " } else { "  " }, accent),
            Span::styled(name, Style::new().fg(if muted { c(t.dim) } else { hue }).add_modifier(if muted { Modifier::CROSSED_OUT } else { Modifier::empty() })),
            Span::raw("  "),
            Span::styled(gauge(s.meter_fraction(), 10), Style::new().fg(c(meter_colour))),
            Span::raw("   "),
            Span::styled(format!("{} {:>5}", gauge(s.gain / 2.0, 10), s.gain_text()), pick(Param::Gain)),
            Span::raw("   "),
            Span::styled(format!("{} {:<5}", pan_gauge(s.pan, 9), s.pan_text()), pick(Param::Pan)),
            Span::raw("  "),
            Span::styled(" M ", if muted { Style::new().bg(c(t.error)).fg(c(t.background)) } else { dim }),
            Span::styled(" S ", if s.solo { Style::new().bg(c(t.drum)).fg(c(t.background)) } else { dim }),
        ];
        let clip = app.studio.clips.verb_for(&s.name);
        if !clip.is_empty() {
            spans.push(Span::styled(format!("  clip | {clip}"), Style::new().fg(c(t.note))));
        }
        let line = Line::from(spans);
        lines.push(if is_row { line.style(Style::new().bg(c(t.panel))) } else { line });
    }
    lines.push(Line::from(""));
    lines.push(Line::styled("  master", dim));
    for knob in Knob::ALL {
        let is_row = matches!(app.mix_row(selected), MixRow::Knob(sel) if sel == knob);
        let (min, max, _) = knob.range();
        let value = app.studio.knobs.get(knob);
        let mut spans = vec![
            Span::styled(if is_row { "▸ " } else { "  " }, accent),
            Span::styled(format!("{:<10}", knob.label()), if is_row { accent } else { Style::new().fg(c(t.foreground)) }),
            Span::raw("  "),
            Span::styled(gauge((value - min) / (max - min), 20), Style::new().fg(c(t.note))),
            Span::styled(format!(" {:>5}", knob.format(value)), Style::new().fg(c(t.accent))),
        ];
        if app.studio.learn.armed == Some(knob) {
            spans.push(Span::styled("  ← bouge un potard MIDI", accent));
        } else if let Some(cc) = app.studio.learn.bindings.describe(knob) {
            spans.push(Span::styled(format!("  {cc}"), dim));
        }
        let line = Line::from(spans);
        lines.push(if is_row { line.style(Style::new().bg(c(t.panel))) } else { line });
    }
    lines.push(Line::from(""));
    let solo = if app.studio.mixer.any_solo() { " · solo actif : seules les voix S jouent" } else { "" };
    lines.push(Line::styled(format!("  {}{solo}", app.studio.loudness.text()), dim));
    // keep the selected row visible in a short terminal
    let height = inner.height as usize;
    let selected_line = if selected < strips.len() { selected + 1 } else { selected + 3 };
    let scroll = selected_line.saturating_sub(height.saturating_sub(2)) as u16;
    f.render_widget(Paragraph::new(lines).scroll((scroll, 0)), inner);
}

/// One line showing the `song:` arrangement, proportional to the bars, with
/// the playhead. In TIMELINE mode the selected section is highlighted.
pub fn draw_timeline(f: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    let song = songline::sections(&app.editor.text());
    let selected = match app.mode { Mode::Timeline { selected } => Some(selected), _ => None };
    let total: u32 = song.iter().map(|(_, b)| b).sum();
    let label = if selected.is_some() { " song ▸ " } else { " song  " };
    let mut spans = vec![Span::styled(label, Style::new().fg(c(t.accent)).add_modifier(Modifier::BOLD))];
    let width = (area.width as usize).saturating_sub(label.chars().count() + 22).max(song.len());
    let playhead = app.engine.as_ref().ok().map(|e| e.status()).filter(|s| s.playing())
        .map(|s| (((s.song_bar() as f32 + s.phase()) / total.max(1) as f32).fract() * width as f32) as usize);
    let mut used = 0;
    for (i, (name, bars)) in song.iter().enumerate() {
        let last = i + 1 == song.len();
        let cells = if last { width.saturating_sub(used) } else { ((*bars as f32 / total.max(1) as f32) * width as f32).round().max(1.0) as usize };
        let text: String = format!("{name}{}", " ".repeat(cells)).chars().take(cells).collect();
        let is_selected = selected == Some(i);
        let bg = if is_selected { c(t.accent) } else { hue_color(voice_hue(name, None), 0.25) };
        let fg = if is_selected { c(t.background) } else { c(t.foreground) };
        for (k, ch) in text.chars().enumerate() {
            let mut style = Style::new().bg(bg).fg(fg);
            if playhead == Some(used + k) { style = Style::new().bg(c(t.foreground)).fg(c(t.background)); }
            if k == 0 && i > 0 { style = style.add_modifier(Modifier::BOLD); }
            spans.push(Span::styled(ch.to_string(), style));
        }
        used += cells;
    }
    let info = match selected.and_then(|s| song.get(s)) {
        Some((name, bars)) => format!(" « {name} » {bars} mes."),
        None => format!(" {total} mes. · Alt+A"),
    };
    spans.push(Span::styled(info, Style::new().fg(c(t.dim))));
    f.render_widget(Paragraph::new(Line::from(spans)).style(Style::new().bg(c(t.panel))), area);
}

/// The piano roll (Alt+R) in place of the grid.
pub fn draw_roll(f: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    let block = Block::new().borders(Borders::ALL).border_type(BorderType::Rounded)
        .border_style(Style::new().fg(c(t.dim)))
        .title(Span::styled(" piano-roll · Alt+R grille ", Style::new().fg(c(t.foreground))));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let status = app.engine.as_ref().ok().map(|e| e.status()).filter(|s| s.playing());
    let program = app.studio_program();
    let notes = roll::roll(&program, status.map_or(0, |s| s.song_bar()));
    if notes.notes.is_empty() || inner.width < 8 || inner.height < 2 {
        f.render_widget(Paragraph::new(Line::styled(" écris une mélodie (notes, degrés ou accords)", Style::new().fg(c(t.dim)))), inner);
        return;
    }
    let label_w = 4usize;
    let cols = inner.width as usize - label_w;
    let rows = inner.height as usize;
    // pitch → screen row (squeezed if the range is taller than the panel)
    let span = notes.rows() as usize;
    let row_of = |midi: i32| ((notes.high - midi) as usize * rows.min(span)) / span;
    let mut grid: Vec<Vec<(char, Color)>> = vec![vec![('·', c(t.background.mix(t.dim, 0.5))); cols]; rows.min(span)];
    for n in &notes.notes {
        let r = row_of(n.midi).min(grid.len() - 1);
        let start = ((n.start * cols as f32) as usize).min(cols - 1);
        let end = (((n.start + n.len) * cols as f32).ceil() as usize).clamp(start + 1, cols);
        for cell in &mut grid[r][start..end] {
            *cell = ('█', hue_color(n.hue, 0.6));
        }
    }
    let head = status.map(|s| ((s.phase() * cols as f32) as usize).min(cols - 1));
    const NAMES: [&str; 12] = ["c", "c#", "d", "d#", "e", "f", "f#", "g", "g#", "a", "a#", "b"];
    let lines: Vec<Line> = grid.iter().enumerate().map(|(r, cells)| {
        let midi = notes.high - (r * span / rows.min(span)) as i32;
        let name = format!("{}{}", NAMES[midi.rem_euclid(12) as usize], midi.div_euclid(12) - 1);
        let mut spans = vec![Span::styled(format!("{name:<4}"), Style::new().fg(c(t.dim)))];
        for (x, (ch, colour)) in cells.iter().enumerate() {
            let mut style = Style::new().fg(*colour);
            if head == Some(x) { style = style.bg(c(t.selection)); }
            spans.push(Span::styled(ch.to_string(), style));
        }
        Line::from(spans)
    }).collect();
    f.render_widget(Paragraph::new(lines), inner);
}

/// The import prompt on the status line (Alt+I).
pub fn draw_import(f: &mut Frame, app: &App, area: Rect) {
    let Mode::Import { input } = &app.mode else { return };
    let t = &app.theme;
    let label = " IMPORT › ";
    let shown = input.replace('\n', " ⏎ ");
    let mut spans = vec![Span::styled(label, Style::new().fg(c(t.accent)).add_modifier(Modifier::BOLD))];
    // show the end of a long paste
    let room = (area.width as usize).saturating_sub(label.len() + 30);
    let tail: String = { let n = shown.chars().count(); shown.chars().skip(n.saturating_sub(room)).collect() };
    spans.push(Span::raw(tail.clone()));
    if input.is_empty() {
        spans.push(Span::styled("colle un code LOOM1:, un lien ou du code LOOM", Style::new().fg(c(t.dim))));
    }
    spans.push(Span::styled("   Entrée charge · Échap ferme", Style::new().fg(c(t.dim))));
    f.render_widget(Paragraph::new(Line::from(spans)).style(Style::new().bg(c(t.panel))), area);
    let x = area.x + (label.chars().count() + tail.chars().count()) as u16;
    if x < area.right() { f.set_cursor_position(Position::new(x, area.y)); }
}
