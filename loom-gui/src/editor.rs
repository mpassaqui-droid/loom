//! Syntax colouring for the code editor.
//!
//! Two layers, kept apart on purpose:
//! - [`spans`] is pure logic: it cuts LOOM source into coloured pieces
//!   ([`Kind`]) and is unit-tested without any window.
//! - [`Highlighter`] turns those pieces into an egui `LayoutJob` for the
//!   `TextEdit`, and caches it so we don't redo the work every frame.

use std::ops::Range;
use std::sync::Arc;

use eframe::egui::{self, text::LayoutJob, Color32, FontId, TextFormat};

/// What a piece of source text is, for colouring.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Anything without a special meaning (notes, rests, `:`, `|`, spaces…).
    Plain,
    /// `# …` up to the end of the line.
    Comment,
    /// The voice or directive name before the `:` (`kick`, `bpm`, `song`…).
    Name,
    /// A verb after a `|` (`rev`, `every 4 fast 2`, `saw`…).
    Verb,
    /// A drum hit `x` (or accented `X`) in the pattern.
    Hit,
}

/// Cut `text` into `(byte range, kind)` pieces that cover it entirely, in order.
/// Neighbouring pieces of the same kind are merged.
pub fn spans(text: &str) -> Vec<(Range<usize>, Kind)> {
    let mut out: Vec<(Range<usize>, Kind)> = Vec::new();
    let mut line_start = 0;
    for line in text.split_inclusive('\n') {
        for (range, kind) in line_spans(line) {
            push(&mut out, line_start + range.start..line_start + range.end, kind);
        }
        line_start += line.len();
    }
    out
}

/// The pieces of one line (it may end with its `\n`).
fn line_spans(line: &str) -> Vec<(Range<usize>, Kind)> {
    let mut out = Vec::new();
    let comment = comment_start(line).unwrap_or(line.len());
    let code = &line[..comment];

    match code.find(':') {
        // not a voice or directive line: nothing to colour before the comment
        None => push(&mut out, 0..comment, Kind::Plain),
        Some(colon) => {
            let name_start = code.len() - code.trim_start().len();
            let name_end = code[..colon].trim_end().len().max(name_start);
            push(&mut out, 0..name_start, Kind::Plain);
            push(&mut out, name_start..name_end, Kind::Name);
            push(&mut out, name_end..colon + 1, Kind::Plain);

            // after the colon: the pattern, then `| verb | verb`
            let mut seg_start = colon + 1;
            let mut first = true;
            for seg in code[colon + 1..].split('|') {
                let seg_end = seg_start + seg.len();
                if first {
                    pattern_spans(&mut out, code, seg_start..seg_end);
                } else {
                    push(&mut out, seg_start - 1..seg_start, Kind::Plain); // the `|` itself
                    let verb_start = seg_start + (seg.len() - seg.trim_start().len());
                    let verb_end = seg_start + seg.trim_end().len();
                    push(&mut out, seg_start..verb_start, Kind::Plain);
                    push(&mut out, verb_start..verb_end.max(verb_start), Kind::Verb);
                    push(&mut out, verb_end.max(verb_start)..seg_end, Kind::Plain);
                }
                first = false;
                seg_start = seg_end + 1;
            }
        }
    }
    push(&mut out, comment..line.len(), Kind::Comment);
    out
}

/// Colour the drum hits inside the pattern part `range` of `code`.
fn pattern_spans(out: &mut Vec<(Range<usize>, Kind)>, code: &str, range: Range<usize>) {
    let bytes = code.as_bytes();
    let is_word = |i: usize| bytes.get(i).is_some_and(|b| b.is_ascii_alphanumeric());
    let mut plain_from = range.start;
    for i in range.clone() {
        let hit = (bytes[i] == b'x' || bytes[i] == b'X')
            && !(i > range.start && is_word(i - 1))
            && !is_word(i + 1);
        if hit {
            push(out, plain_from..i, Kind::Plain);
            push(out, i..i + 1, Kind::Hit);
            plain_from = i + 1;
        }
    }
    push(out, plain_from..range.end, Kind::Plain);
}

/// Where a comment starts: a `#` at the line start or after a space/tab, like
/// `loom-core` does (so `f#3` stays a note).
pub fn comment_start(line: &str) -> Option<usize> {
    let b = line.as_bytes();
    (0..b.len()).find(|&i| b[i] == b'#' && (i == 0 || b[i - 1] == b' ' || b[i - 1] == b'\t'))
}

/// Append a piece, skipping empty ones and merging with the previous piece of the same kind.
fn push(out: &mut Vec<(Range<usize>, Kind)>, range: Range<usize>, kind: Kind) {
    if range.is_empty() { return; }
    if let Some((last, last_kind)) = out.last_mut() {
        if *last_kind == kind && last.end == range.start {
            last.end = range.end;
            return;
        }
    }
    out.push((range, kind));
}

/// The colours and font the editor uses, taken from the theme.
#[derive(Debug, Clone, PartialEq)]
pub struct Palette {
    pub plain: Color32,
    pub comment: Color32,
    pub name: Color32,
    pub verb: Color32,
    pub hit: Color32,
    pub font_size: f32,
}

impl Palette {
    fn color(&self, kind: Kind) -> Color32 {
        match kind {
            Kind::Plain => self.plain,
            Kind::Comment => self.comment,
            Kind::Name => self.name,
            Kind::Verb => self.verb,
            Kind::Hit => self.hit,
        }
    }
}

/// Build the coloured text layout for the editor.
pub fn layout_job(text: &str, palette: &Palette) -> LayoutJob {
    let font = FontId::monospace(palette.font_size);
    let mut job = LayoutJob::default();
    for (range, kind) in spans(text) {
        job.append(&text[range], 0.0, TextFormat::simple(font.clone(), palette.color(kind)));
    }
    job.wrap.max_width = f32::INFINITY; // code lines never wrap: columns stay aligned
    job
}

/// Remembers the last layout so typing only re-colours when the text changed.
#[derive(Default)]
pub struct Highlighter {
    text: String,
    palette: Option<Palette>,
    job: LayoutJob,
}

impl Highlighter {
    /// The layout for `text`, rebuilt only if the text or the colours changed.
    pub fn galley(&mut self, ui: &egui::Ui, text: &str, palette: &Palette) -> Arc<egui::Galley> {
        if self.text != text || self.palette.as_ref() != Some(palette) {
            self.text = text.to_string();
            self.palette = Some(palette.clone());
            self.job = layout_job(text, palette);
        }
        ui.fonts_mut(|fonts| fonts.layout_job(self.job.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pieces as `(text, kind)`, easier to read in assertions.
    fn pieces(text: &str) -> Vec<(&str, Kind)> {
        spans(text).into_iter().map(|(r, k)| (&text[r], k)).collect()
    }

    #[test]
    fn covers_the_whole_text() {
        let src = "# titre\nbpm: 120\nkick: x . x . | every 4 rev # fin\n\nlead: c4 é | saw\n";
        let s = spans(src);
        assert_eq!(s.first().unwrap().0.start, 0);
        assert_eq!(s.last().unwrap().0.end, src.len());
        assert!(s.windows(2).all(|w| w[0].0.end == w[1].0.start));
    }

    #[test]
    fn voice_line() {
        use Kind::*;
        assert_eq!(
            pieces("hat:   x x . | every 4 fast 2"),
            [("hat", Name), (":   ", Plain), ("x", Hit), (" ", Plain), ("x", Hit), (" . | ", Plain), ("every 4 fast 2", Verb)]
        );
    }

    #[test]
    fn several_verbs_and_a_comment() {
        use Kind::*;
        assert_eq!(
            pieces("  bass: c2 . | saw |oct 2  # grave\n"),
            [("  ", Plain), ("bass", Name), (": c2 . | ", Plain), ("saw", Verb), (" |", Plain), ("oct 2", Verb), ("  ", Plain), ("# grave\n", Comment)]
        );
    }

    #[test]
    fn directives_and_comments() {
        use Kind::*;
        assert_eq!(pieces("bpm: 120"), [("bpm", Name), (": 120", Plain)]);
        assert_eq!(pieces("# kick: x x"), [("# kick: x x", Comment)]);
        assert_eq!(pieces("song: intro*4 drop*8"), [("song", Name), (": intro*4 drop*8", Plain)]);
        assert_eq!(pieces("juste du texte"), [("juste du texte", Plain)]);
    }

    #[test]
    fn hits_only_as_standalone_x() {
        let hits = |src: &str| pieces(src).iter().filter(|(_, k)| *k == Kind::Hit).count();
        assert_eq!(hits("snare: [x x] x! X? . x*3"), 5);
        assert_eq!(hits("lead: xenakis max x2"), 0);
        assert_eq!(hits("kick: . . | every 2 x"), 0, "no hits in verbs");
    }

    #[test]
    fn sharp_notes_are_not_comments() {
        assert_eq!(comment_start("lead: f#3 c#4"), None);
        assert_eq!(comment_start("lead: f#3 # note"), Some(10));
    }
}
