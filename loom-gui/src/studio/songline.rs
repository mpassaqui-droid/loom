//! Editing the code's directive lines from buttons: the `song:` arrangement
//! (TIMELINE) and the tempo `bpm:` (TAP). Each function takes the whole code
//! and returns the new code; the rest of the text is left exactly as it was.

/// One section of the arrangement: its name and length in bars.
pub type Section = (String, u32);

/// The arrangement written in the code (`song: intro*4 verse*8`).
pub fn sections(src: &str) -> Vec<Section> {
    loom_core::parse_song(src)
}

/// A TIMELINE button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SongEdit {
    /// −1 bar (never below 1).
    Shorter,
    /// +1 bar.
    Longer,
    /// Copy the section right after itself.
    Duplicate,
    Delete,
    MoveLeft,
    MoveRight,
}

/// Apply `edit` to the section at `selected`. Returns the new arrangement and
/// which section is selected afterwards (`None` once the last one is deleted).
/// Ported from the browser's `tlEdit`.
pub fn edit_sections(song: &[Section], selected: usize, edit: SongEdit) -> (Vec<Section>, Option<usize>) {
    let mut song = song.to_vec();
    if selected >= song.len() {
        let last = song.len().checked_sub(1);
        return (song, last);
    }
    let mut i = selected;
    match edit {
        SongEdit::Shorter => song[i].1 = song[i].1.saturating_sub(1).max(1),
        SongEdit::Longer => song[i].1 += 1,
        SongEdit::Duplicate => song.insert(i + 1, song[i].clone()),
        SongEdit::Delete => {
            song.remove(i);
            i = i.saturating_sub(1);
        }
        SongEdit::MoveLeft if i > 0 => {
            song.swap(i - 1, i);
            i -= 1;
        }
        SongEdit::MoveRight if i + 1 < song.len() => {
            song.swap(i, i + 1);
            i += 1;
        }
        SongEdit::MoveLeft | SongEdit::MoveRight => {}
    }
    let selected = (!song.is_empty()).then(|| i.min(song.len() - 1));
    (song, selected)
}

/// The `song:` line for an arrangement: `song:   intro*4 verse*8`.
pub fn song_line(song: &[Section]) -> String {
    let parts: Vec<String> = song.iter().map(|(name, bars)| format!("{name}*{bars}")).collect();
    format!("song:   {}", parts.join(" "))
}

/// Write `song` into the code: the first `song:` / `arrange:` line is
/// replaced (its end-of-line comment kept), or a new line goes at the top.
/// An empty arrangement removes the line.
pub fn set_song(src: &str, song: &[Section]) -> String {
    let mut lines: Vec<String> = src.split('\n').map(str::to_string).collect();
    let found = lines.iter().position(|l| matches!(directive(l).as_deref(), Some("song" | "arrange")));
    match (found, song.is_empty()) {
        (Some(i), true) => {
            lines.remove(i);
        }
        (Some(i), false) => {
            let comment = trailing_comment(&lines[i]);
            lines[i] = join_comment(song_line(song), comment);
        }
        (None, true) => {}
        (None, false) => lines.insert(0, song_line(song)),
    }
    lines.join("\n")
}

/// Write the tempo into the code: the `bpm:` line (or a `tempo: N` line) gets
/// the new number, keeping its comment; without one, `bpm: N` is added after
/// the comments at the top of the file. Ported from `setBpmDirective`.
pub fn set_bpm(src: &str, bpm: u32) -> String {
    let mut lines: Vec<String> = src.split('\n').map(str::to_string).collect();
    let is_tempo_line = |line: &str| match directive(line).as_deref() {
        Some("bpm") => true,
        // `tempo: 128` is the tempo; `tempo: drop 140` is a per-section map, left alone
        Some("tempo") => value(line).split_whitespace().next().is_some_and(|t| t.parse::<f64>().is_ok()),
        _ => false,
    };
    if let Some(i) = lines.iter().position(|l| is_tempo_line(l)) {
        let colon = lines[i].find(':').unwrap_or(0);
        let head = lines[i][..colon].to_string();
        let comment = trailing_comment(&lines[i]);
        lines[i] = join_comment(format!("{head}: {bpm}"), comment);
    } else {
        let after_comments = lines.iter().take_while(|l| l.trim_start().starts_with('#')).count();
        lines.insert(after_comments, format!("bpm: {bpm}"));
    }
    lines.join("\n")
}

/// The line number (0-based) where voice `name` is written (`name: …`),
/// for jumping to it from the piano roll or the mixer.
pub fn voice_line(src: &str, name: &str) -> Option<usize> {
    src.split('\n').position(|l| directive(l).is_some_and(|d| d == name.to_lowercase()))
}

/// The lower-case word before the `:` of a code line (`None` for comments
/// and lines without a colon).
fn directive(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    if trimmed.starts_with('#') {
        return None;
    }
    let colon = trimmed.find(':')?;
    let head = trimmed[..colon].trim();
    (!head.is_empty() && !head.contains(char::is_whitespace)).then(|| head.to_lowercase())
}

/// What comes after the `:` (without the comment).
fn value(line: &str) -> &str {
    let after = line.find(':').map_or("", |c| &line[c + 1..]);
    after.split(" #").next().unwrap_or(after).trim()
}

/// The `# comment` at the end of a line, if any (a `#` after a space).
fn trailing_comment(line: &str) -> Option<&str> {
    line.find(" #").or_else(|| line.find("\t#")).map(|i| line[i..].trim())
}

fn join_comment(line: String, comment: Option<&str>) -> String {
    match comment {
        Some(c) => format!("{line} {c}"),
        None => line,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song(pairs: &[(&str, u32)]) -> Vec<Section> {
        pairs.iter().map(|(n, b)| (n.to_string(), *b)).collect()
    }

    #[test]
    fn every_timeline_edit() {
        let s = song(&[("intro", 4), ("verse", 8), ("drop", 8)]);
        assert_eq!(edit_sections(&s, 1, SongEdit::Longer), (song(&[("intro", 4), ("verse", 9), ("drop", 8)]), Some(1)));
        assert_eq!(edit_sections(&song(&[("a", 1)]), 0, SongEdit::Shorter).0, song(&[("a", 1)]), "never below 1 bar");
        assert_eq!(edit_sections(&s, 0, SongEdit::Duplicate), (song(&[("intro", 4), ("intro", 4), ("verse", 8), ("drop", 8)]), Some(0)));
        assert_eq!(edit_sections(&s, 1, SongEdit::MoveLeft), (song(&[("verse", 8), ("intro", 4), ("drop", 8)]), Some(0)));
        assert_eq!(edit_sections(&s, 1, SongEdit::MoveRight), (song(&[("intro", 4), ("drop", 8), ("verse", 8)]), Some(2)));
        assert_eq!(edit_sections(&s, 2, SongEdit::MoveRight).1, Some(2), "the last one can't go further");
        assert_eq!(edit_sections(&s, 1, SongEdit::Delete), (song(&[("intro", 4), ("drop", 8)]), Some(0)));
        assert_eq!(edit_sections(&song(&[("a", 2)]), 0, SongEdit::Delete), (vec![], None));
    }

    #[test]
    fn song_line_is_rewritten_in_place() {
        let src = "# titre\nbpm: 120\nsong: intro*4 drop # le plan\nkick: x . x .\n";
        let new = set_song(src, &song(&[("drop", 4), ("intro", 5)]));
        assert_eq!(new, "# titre\nbpm: 120\nsong:   drop*4 intro*5 # le plan\nkick: x . x .\n");
        assert_eq!(sections(&new), song(&[("drop", 4), ("intro", 5)]));
        // no song yet: added at the top; empty: the line goes away
        assert_eq!(set_song("kick: x", &song(&[("a", 2)])), "song:   a*2\nkick: x");
        assert_eq!(set_song("song: a*2\nkick: x", &[]), "kick: x");
        // `arrange:` is the same directive; a commented-out song line is not
        assert_eq!(set_song("# song: old*1\narrange: a b", &song(&[("b", 4)])), "# song: old*1\nsong:   b*4");
    }

    #[test]
    fn bpm_line_is_rewritten_or_added() {
        assert_eq!(set_bpm("bpm: 120 # vite\nkick: x", 96), "bpm: 96 # vite\nkick: x");
        assert_eq!(set_bpm("tempo: drop 140\ntempo: 118\n", 100), "tempo: drop 140\ntempo: 100\n");
        assert_eq!(set_bpm("# mon morceau\n# 2026\nkick: x . x .\n", 128), "# mon morceau\n# 2026\nbpm: 128\nkick: x . x .\n");
        assert_eq!(loom_core::Program::parse(&set_bpm("kick: x", 90)).bpm, 90.0);
    }

    #[test]
    fn finds_a_voice_line() {
        let src = "# lead: pas ici\nbpm: 100\nlead: c4 e4\nlead@drop: g4";
        assert_eq!(voice_line(src, "lead"), Some(2));
        assert_eq!(voice_line(src, "bass"), None);
    }
}
