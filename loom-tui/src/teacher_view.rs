//! Drawing LE PROF & COMPOSITION in the terminal: the teacher's answer
//! popup (with scrolling), OREILLE, HUM and LEÇONS & RÉFÉRENCE.

use std::time::Instant;

use crate::app::{App, Mode};
use crate::teacher::browse::{self, Row, Tab};
use crate::teacher::ear::{level_name, Tone, LEVELS};
use crate::teacher::hum::Step;
use crate::ui::{c, highlight, popup};
use loom_app::lessons::{LESSONS, REFERENCE};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph};
use ratatui::Frame;

const SPIN: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Display width of a text in terminal cells (accents count 1, emoji 2).
fn width(text: &str) -> usize {
    Span::raw(text).width()
}

/// Cut `text` into lines of at most `max` cells, breaking between words
/// (a word longer than a line is cut). Newlines start a new line; an empty
/// text gives no line.
pub fn wrap(text: &str, max: usize) -> Vec<String> {
    let max = max.max(1);
    let mut out = Vec::new();
    for paragraph in text.split('\n') {
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            let mut word = word.to_string();
            while width(&word) > max {
                // a very long word: fill the line, then carry on
                if !line.is_empty() {
                    out.push(std::mem::take(&mut line));
                }
                let mut head = String::new();
                for ch in word.chars() {
                    if width(&head) + width(&ch.to_string()) > max {
                        break;
                    }
                    head.push(ch);
                }
                if head.is_empty() {
                    head = word.chars().next().map(String::from).unwrap_or_default();
                }
                word = word[head.len()..].to_string();
                out.push(head);
            }
            if word.is_empty() {
                continue;
            }
            if line.is_empty() {
                line = word;
            } else if width(&line) + 1 + width(&word) <= max {
                line.push(' ');
                line.push_str(&word);
            } else {
                out.push(std::mem::replace(&mut line, word));
            }
        }
        if !line.is_empty() || paragraph.trim().is_empty() && !text.is_empty() {
            out.push(line);
        }
    }
    out
}

/// The first line to show so that `scroll` stays within `total` lines in a `height`-line view.
pub fn clamp_scroll(scroll: usize, total: usize, height: usize) -> usize {
    scroll.min(total.saturating_sub(height))
}

/// `▮▮▮▯▯▯` — a level from 0 to 1 in `cells` cells.
pub fn meter(fraction: f32, cells: usize) -> String {
    let full = ((fraction.clamp(0.0, 1.0) * cells as f32).round() as usize).min(cells);
    format!("{}{}", "▮".repeat(full), "▯".repeat(cells - full))
}

/// The teacher's answer: title, spinner while thinking, wrapped text, scroll.
pub fn draw_prof(f: &mut Frame, app: &mut App) {
    let area = f.area();
    let t = app.theme.clone();
    let inner = popup(f, app, "le prof", (area.width * 3 / 4).max(40), (area.height * 2 / 3).max(10));
    let Some(answer) = &app.teacher.prof else { return };
    let text_width = inner.width.saturating_sub(2) as usize;
    let mut lines = vec![Line::styled(format!(" {}", answer.title), Style::new().fg(c(t.accent)).add_modifier(Modifier::BOLD)), Line::from("")];
    if let Some(job) = &app.teacher.prof_job {
        let elapsed = job.elapsed();
        let frame = SPIN[(elapsed.as_millis() / 100) as usize % SPIN.len()];
        lines.push(Line::styled(format!(" {frame} le prof réfléchit… {} s   (Échap annule)", elapsed.as_secs()), Style::new().fg(c(t.accent))));
    }
    let tint = if answer.is_error { t.error } else { t.foreground };
    for (i, text) in answer.lines.iter().enumerate() {
        if i > 0 {
            lines.push(Line::from(""));
        }
        lines.extend(wrap(text, text_width).into_iter().map(|l| Line::styled(format!(" {l}"), Style::new().fg(c(tint)))));
    }
    let [body, footer] = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(inner);
    let height = body.height as usize;
    let total = lines.len();
    let Mode::Prof { scroll } = &mut app.mode else { return };
    *scroll = clamp_scroll(*scroll, total, height);
    let shown = *scroll;
    f.render_widget(Paragraph::new(lines).scroll((shown as u16, 0)), body);
    let mut hints = String::from(" ↑↓ PgUp PgDn défiler · Entrée fermer · Échap annuler/fermer");
    if answer.demo.is_some() {
        hints.push_str(" · r réécouter ♪");
    }
    if total > height {
        hints.push_str(&format!("   [{}/{}]", (shown + height).min(total), total));
    }
    f.render_widget(Paragraph::new(Line::styled(hints, Style::new().fg(c(t.dim)))), footer);
}

/// OREILLE: level, question, answer field, verdict, score, keys.
pub fn draw_ear(f: &mut Frame, app: &App) {
    let t = &app.theme;
    let inner = popup(f, app, "👂 oreille — entraînement", 78, 18);
    let Some(ear) = &app.teacher.ear else { return };
    let dim = Style::new().fg(c(t.dim));
    let mut level_spans = vec![Span::styled(" niveau ", dim)];
    for (level, _) in LEVELS {
        let name = level_name(level);
        if ear.level == level {
            level_spans.push(Span::styled(format!(" {name} "), Style::new().bg(c(t.accent)).fg(c(t.background)).add_modifier(Modifier::BOLD)));
        } else {
            level_spans.push(Span::raw(format!(" {name} ")));
        }
    }
    level_spans.push(Span::styled("  (←→)", dim));
    let mut lines = vec![Line::from(level_spans), Line::from("")];
    let text_width = inner.width.saturating_sub(2) as usize;
    lines.extend(wrap(&ear.prompt(), text_width).into_iter().map(|l| Line::from(format!(" {l}"))));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(" réponse › ", Style::new().fg(c(t.accent)).add_modifier(Modifier::BOLD)),
        Span::raw(ear.input.clone()),
        Span::styled("▏", Style::new().fg(c(t.accent))),
    ]));
    lines.push(Line::from(""));
    if let Some((text, tone)) = ear.verdict() {
        let colour = match tone {
            Tone::Good => t.note,
            Tone::Bad => t.error,
            Tone::Neutral => t.dim,
        };
        let style = Style::new().fg(c(colour)).add_modifier(Modifier::BOLD);
        lines.extend(wrap(&text, text_width).into_iter().map(|l| Line::styled(format!(" {l}"), style)));
    }
    if let Some(answer) = ear.revealed() {
        lines.push(Line::styled(format!(" réponse : {answer}"), Style::new().fg(c(t.accent))));
    }
    lines.push(Line::styled(format!(" {}", ear.progress()), dim));
    if app.teacher.demo.is_some() {
        lines.push(Line::styled(" ♪ écoute…", Style::new().fg(c(t.accent))));
    }
    let [body, footer] = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(inner);
    f.render_widget(Paragraph::new(lines), body);
    let keys = " Tab écouter · Entrée vérifier · ?+Entrée réponse · ↓ suivant · ←→ niveau · Échap";
    f.render_widget(Paragraph::new(Line::styled(keys, dim)), footer);
}

/// HUM: countdown, recording with level meter, result.
pub fn draw_hum(f: &mut Frame, app: &App) {
    let t = &app.theme;
    let inner = popup(f, app, "🎤 hum — chante, LOOM écrit la mélodie", 72, 14);
    let Some(take) = &app.teacher.hum else { return };
    let dim = Style::new().fg(c(t.dim));
    let accent = Style::new().fg(c(t.accent)).add_modifier(Modifier::BOLD);
    let mic = Line::from(vec![Span::styled(" micro ", dim), Span::styled(meter(take.level(), 30), Style::new().fg(c(t.note)))]);
    let mut lines = vec![Line::styled(format!(" {}", take.prompt()), dim), Line::from("")];
    let keys = match take.step(Instant::now()) {
        Step::NoMic(text) => {
            let width = inner.width.saturating_sub(2) as usize;
            lines.extend(wrap(text, width).into_iter().map(|l| Line::styled(format!(" {l}"), Style::new().fg(c(t.error)))));
            " r recommencer · Échap fermer"
        }
        Step::Countdown(n) => {
            lines.push(Line::styled(format!(" {n}…  prépare-toi, l'enregistrement commence à zéro"), accent));
            lines.push(Line::from(""));
            lines.push(mic);
            " Échap annuler"
        }
        Step::Recording(progress) => {
            lines.push(Line::styled(" ⏺ enregistrement — chante !", Style::new().fg(c(t.error)).add_modifier(Modifier::BOLD)));
            lines.push(Line::from(vec![Span::styled(" temps ", dim), Span::styled(meter(progress, 30), accent)]));
            lines.push(mic);
            " Échap annuler"
        }
        Step::Heard(hum) => {
            lines.push(Line::from(" mélodie entendue :"));
            let width = inner.width.saturating_sub(2) as usize;
            lines.push(Line::styled(format!(" {}", hum.tokens.join(" ")), accent));
            lines.extend(wrap(&hum.line, width).into_iter().map(|l| Line::styled(format!(" {l}"), dim)));
            " Entrée ajouter au morceau · r recommencer · Échap annuler"
        }
        Step::Silent(hum) => {
            let width = inner.width.saturating_sub(2) as usize;
            lines.extend(wrap(&hum.message(), width).into_iter().map(|l| Line::styled(format!(" {l}"), Style::new().fg(c(t.error)))));
            " r recommencer · Échap fermer"
        }
    };
    let [body, footer] = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(inner);
    f.render_widget(Paragraph::new(lines), body);
    f.render_widget(Paragraph::new(Line::styled(keys, dim)), footer);
}

/// LEÇONS & RÉFÉRENCE: tabs, search, list on the left, details on the right.
pub fn draw_lessons(f: &mut Frame, app: &App) {
    let Mode::Lessons { tab, query, selected } = &app.mode else { return };
    let t = &app.theme;
    let area = f.area();
    let inner = popup(f, app, "leçons & référence", area.width.saturating_sub(4).min(120), area.height.saturating_sub(2));
    let dim = Style::new().fg(c(t.dim));
    let accent = Style::new().fg(c(t.accent)).add_modifier(Modifier::BOLD);
    let [head, search, body, footer] =
        Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Min(3), Constraint::Length(1)]).areas(inner);

    let mut tabs = vec![Span::raw(" ")];
    for (each, count) in [(Tab::Lessons, LESSONS.len()), (Tab::Reference, REFERENCE.len())] {
        let label = format!(" {} ({count}) ", each.name());
        tabs.push(if each == *tab { Span::styled(label, Style::new().bg(c(t.accent)).fg(c(t.background)).add_modifier(Modifier::BOLD)) } else { Span::raw(label) });
        tabs.push(Span::raw(" "));
    }
    tabs.push(Span::styled(" Tab change d'onglet", dim));
    f.render_widget(Paragraph::new(Line::from(tabs)), head);
    let mut search_line = vec![Span::styled(" chercher › ", accent), Span::raw(query.clone()), Span::styled("▏", accent)];
    if query.is_empty() {
        search_line.push(Span::styled("tape un mot : rythme, développe, raga…", dim));
    }
    f.render_widget(Paragraph::new(Line::from(search_line)), search);

    let rows = browse::rows(*tab, query);
    let [list_area, detail_area] = Layout::horizontal([Constraint::Percentage(42), Constraint::Percentage(58)]).areas(body);
    if rows.is_empty() {
        f.render_widget(Paragraph::new(Line::styled(" rien ne correspond", dim)), list_area);
    } else {
        let name_width = list_area.width.saturating_sub(3) as usize;
        let items: Vec<ListItem> = rows
            .iter()
            .map(|row| match row {
                Row::Lesson(lesson) => ListItem::new(format!(" {}", clip(lesson.title, name_width))),
                Row::Section(section) => ListItem::new(Line::styled(format!(" {}", clip(section, name_width)), accent)),
                Row::Reference(reference) => ListItem::new(format!("   {}", clip(reference.syntax, name_width.saturating_sub(2)))),
            })
            .collect();
        let list = List::new(items).highlight_style(Style::new().bg(c(t.accent)).fg(c(t.background)).add_modifier(Modifier::BOLD));
        let mut state = ListState::default().with_selected(Some((*selected).min(rows.len() - 1)));
        f.render_stateful_widget(list, list_area, &mut state);
    }

    let detail_width = detail_area.width.saturating_sub(2) as usize;
    let mut detail: Vec<Line> = Vec::new();
    match rows.get(*selected) {
        Some(Row::Lesson(lesson)) => {
            detail.push(Line::styled(format!(" {}", lesson.title), accent));
            detail.push(Line::from(""));
            detail.extend(wrap(lesson.description, detail_width).into_iter().map(|l| Line::from(format!(" {l}"))));
            detail.push(Line::from(""));
            for code in lesson.code.lines() {
                let mut spans = vec![Span::raw(" ")];
                spans.extend(highlight(code, app));
                detail.push(Line::from(spans));
            }
        }
        Some(Row::Reference(reference)) => {
            detail.push(Line::styled(format!(" {}", reference.section), dim));
            detail.push(Line::styled(format!(" {}", reference.syntax), Style::new().fg(c(t.note)).add_modifier(Modifier::BOLD)));
            detail.push(Line::from(""));
            detail.extend(wrap(reference.explanation, detail_width).into_iter().map(|l| Line::from(format!(" {l}"))));
        }
        _ => {}
    }
    f.render_widget(Paragraph::new(detail), detail_area);
    let keys = match rows.get(*selected) {
        Some(Row::Lesson(_)) => " ↑↓ choisir · Entrée essayer (F3 rend ton morceau) · Tab onglet · tape pour chercher · Échap fermer",
        _ => " ↑↓ choisir · Tab onglet · tape pour chercher · Échap fermer",
    };
    f.render_widget(Paragraph::new(Line::styled(keys, dim)), footer);
}

/// At most `max` characters, with `…` when cut.
fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        let mut s: String = text.chars().take(max.saturating_sub(1)).collect();
        s.push('…');
        s
    }
}

/// The status line while the teacher thinks (like VIBE's).
pub fn thinking_line(app: &App) -> Option<Line<'static>> {
    let job = app.teacher.prof_job.as_ref()?;
    let t = &app.theme;
    let elapsed = job.elapsed();
    let frame = SPIN[(elapsed.as_millis() / 100) as usize % SPIN.len()];
    Some(Line::from(vec![
        Span::styled(format!(" {frame} le prof réfléchit… {} s ", elapsed.as_secs()), Style::new().fg(c(t.accent)).add_modifier(Modifier::BOLD)),
        Span::styled("   Échap annule", Style::new().fg(c(t.dim))),
    ]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_between_words() {
        assert_eq!(wrap("le prof explique la gamme", 10), ["le prof", "explique", "la gamme"]);
        assert_eq!(wrap("un deux", 80), ["un deux"]);
        assert_eq!(wrap("", 10), Vec::<String>::new());
        // accents are one cell each
        assert_eq!(wrap("écoute très épuré", 11), ["écoute très", "épuré"]);
        // an explicit newline and a blank line are kept
        assert_eq!(wrap("a\n\nb", 10), ["a", "", "b"]);
        // a word longer than the line is cut
        assert_eq!(wrap("abcdefghij xy", 4), ["abcd", "efgh", "ij", "xy"]);
        for line in wrap("[ 👂 ✅ ] bravo — c'était bien « quinte » ! (la quinte : le rapport 3/2)", 12) {
            assert!(width(&line) <= 12, "{line}");
        }
        assert_eq!(wrap("x", 0), ["x"], "a zero width still makes progress");
    }

    #[test]
    fn scroll_stays_in_the_text() {
        assert_eq!(clamp_scroll(0, 50, 10), 0);
        assert_eq!(clamp_scroll(45, 50, 10), 40);
        assert_eq!(clamp_scroll(usize::MAX / 2, 50, 10), 40);
        assert_eq!(clamp_scroll(7, 5, 10), 0, "short text: no scroll");
    }

    #[test]
    fn meters_and_clipping() {
        assert_eq!(meter(0.5, 4), "▮▮▯▯");
        assert_eq!(meter(2.0, 3), "▮▮▮");
        assert_eq!(meter(-1.0, 2), "▯▯");
        assert_eq!(clip("Verbes (génératif)", 8), "Verbes …");
        assert_eq!(clip("abc", 8), "abc");
    }
}
