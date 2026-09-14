//! A small multi-line text editor buffer — just what live-coding needs.
//!
//! The cursor column counts *characters*, not bytes, so accents typed in
//! comments (`# thème du refrain`) never split a UTF-8 sequence.

pub struct Editor {
    pub lines: Vec<String>,
    pub row: usize,
    pub col: usize,
    /// First visible line (kept up to date by `scroll_to_cursor`).
    pub scroll: usize,
    /// Changed since the last load/save.
    pub dirty: bool,
}

fn byte_at(s: &str, col: usize) -> usize {
    s.char_indices().nth(col).map_or(s.len(), |(i, _)| i)
}

fn char_len(s: &str) -> usize { s.chars().count() }

impl Editor {
    pub fn new(text: &str) -> Editor {
        let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
        if lines.is_empty() { lines.push(String::new()); }
        Editor { lines, row: 0, col: 0, scroll: 0, dirty: false }
    }

    pub fn text(&self) -> String {
        let mut s = self.lines.join("\n");
        s.push('\n');
        s
    }

    fn line_len(&self) -> usize { char_len(&self.lines[self.row]) }
    fn clamp_col(&mut self) { self.col = self.col.min(self.line_len()); }

    pub fn insert_char(&mut self, c: char) {
        let line = &mut self.lines[self.row];
        let at = byte_at(line, self.col);
        line.insert(at, c);
        self.col += 1;
        self.dirty = true;
    }

    pub fn insert_str(&mut self, s: &str) {
        for c in s.chars() { if c == '\n' { self.newline() } else { self.insert_char(c) } }
    }

    pub fn newline(&mut self) {
        let line = &mut self.lines[self.row];
        let at = byte_at(line, self.col);
        let rest = line.split_off(at);
        self.lines.insert(self.row + 1, rest);
        self.row += 1;
        self.col = 0;
        self.dirty = true;
    }

    pub fn backspace(&mut self) {
        if self.col > 0 {
            let line = &mut self.lines[self.row];
            let at = byte_at(line, self.col - 1);
            line.remove(at);
            self.col -= 1;
        } else if self.row > 0 {
            let line = self.lines.remove(self.row);
            self.row -= 1;
            self.col = self.line_len();
            self.lines[self.row].push_str(&line);
        } else {
            return;
        }
        self.dirty = true;
    }

    pub fn delete(&mut self) {
        if self.col < self.line_len() {
            let line = &mut self.lines[self.row];
            let at = byte_at(line, self.col);
            line.remove(at);
        } else if self.row + 1 < self.lines.len() {
            let next = self.lines.remove(self.row + 1);
            self.lines[self.row].push_str(&next);
        } else {
            return;
        }
        self.dirty = true;
    }

    pub fn left(&mut self) {
        if self.col > 0 { self.col -= 1 } else if self.row > 0 { self.row -= 1; self.col = self.line_len(); }
    }
    pub fn right(&mut self) {
        if self.col < self.line_len() { self.col += 1 } else if self.row + 1 < self.lines.len() { self.row += 1; self.col = 0; }
    }
    pub fn up(&mut self, n: usize) { self.row = self.row.saturating_sub(n); self.clamp_col(); }
    pub fn down(&mut self, n: usize) { self.row = (self.row + n).min(self.lines.len() - 1); self.clamp_col(); }
    pub fn home(&mut self) { self.col = 0; }
    pub fn end(&mut self) { self.col = self.line_len(); }

    /// Comment / uncomment the current line with `# ` — the quickest way to
    /// silence a voice while performing.
    pub fn toggle_comment(&mut self) {
        let line = &mut self.lines[self.row];
        let indent = line.len() - line.trim_start().len();
        if line[indent..].starts_with("# ") {
            line.replace_range(indent..indent + 2, "");
            self.col = self.col.saturating_sub(2);
        } else if line[indent..].starts_with('#') {
            line.replace_range(indent..indent + 1, "");
            self.col = self.col.saturating_sub(1);
        } else {
            line.insert_str(indent, "# ");
            self.col += 2;
        }
        self.dirty = true;
    }

    /// Adjust `scroll` so the cursor row is inside a view of `height` lines.
    pub fn scroll_to_cursor(&mut self, height: usize) {
        let height = height.max(1);
        if self.row < self.scroll { self.scroll = self.row; }
        if self.row >= self.scroll + height { self.scroll = self.row + 1 - height; }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typing_with_accents_and_newlines() {
        let mut e = Editor::new("");
        e.insert_str("# thème\nkick: x");
        assert_eq!(e.text(), "# thème\nkick: x\n");
        e.up(1);
        e.end();
        e.backspace(); // removes the è's neighbour `e`, not half a byte
        assert_eq!(e.lines[0], "# thèm");
        e.left();
        e.backspace();
        assert_eq!(e.lines[0], "# thm");
        assert!(e.dirty);
    }

    #[test]
    fn joins_and_splits_lines() {
        let mut e = Editor::new("ab\ncd");
        e.down(1);
        e.backspace();
        assert_eq!(e.lines, ["abcd"]);
        e.newline();
        assert_eq!(e.lines, ["ab", "cd"]);
        e.up(1);
        e.end();
        e.delete();
        assert_eq!(e.lines, ["abcd"]);
    }

    #[test]
    fn comment_toggle_and_scroll() {
        let mut e = Editor::new("  hat: x x");
        e.toggle_comment();
        assert_eq!(e.lines[0], "  # hat: x x");
        e.toggle_comment();
        assert_eq!(e.lines[0], "  hat: x x");
        let mut e = Editor::new(&"x\n".repeat(50));
        e.down(30);
        e.scroll_to_cursor(10);
        assert_eq!(e.scroll, 21);
    }
}
