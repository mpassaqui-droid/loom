//! LEÇONS & RÉFÉRENCE — the browsable list: two tabs, a search, and a
//! selection that skips the section headings.

use crate::lessons::{self, Lesson, RefRow};

/// The two tabs of the panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Lessons,
    Reference,
}

impl Tab {
    pub fn name(self) -> &'static str {
        match self {
            Tab::Lessons => "Leçons",
            Tab::Reference => "Référence",
        }
    }

    #[allow(dead_code)] // the terminal's Tab key; the window has clickable tabs
    pub fn other(self) -> Tab {
        match self {
            Tab::Lessons => Tab::Reference,
            Tab::Reference => Tab::Lessons,
        }
    }
}

/// One line of the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    Lesson(&'static Lesson),
    /// A reference section heading (not selectable).
    Section(&'static str),
    Reference(&'static RefRow),
}

impl Row {
    #[allow(dead_code)] // the terminal's keyboard selection
    pub fn selectable(self) -> bool {
        !matches!(self, Row::Section(_))
    }
}

/// The list for this tab and search: lessons, or reference rows under their section headings.
pub fn rows(tab: Tab, query: &str) -> Vec<Row> {
    match tab {
        Tab::Lessons => lessons::search_lessons(query).into_iter().map(Row::Lesson).collect(),
        Tab::Reference => lessons::by_section(&lessons::search_reference(query))
            .into_iter()
            .flat_map(|(section, list)| std::iter::once(Row::Section(section)).chain(list.into_iter().map(Row::Reference)))
            .collect(),
    }
}

/// Move the selection `delta` selectable rows (negative = up), staying in the list.
/// From a heading or out of range, it lands on the nearest selectable row.
#[allow(dead_code)] // the terminal's keyboard selection
pub fn move_selection(rows: &[Row], current: usize, delta: isize) -> usize {
    let selectable: Vec<usize> = (0..rows.len()).filter(|&i| rows[i].selectable()).collect();
    if selectable.is_empty() {
        return 0;
    }
    let here = selectable.iter().position(|&i| i >= current).unwrap_or(selectable.len() - 1);
    let target = (here as isize + delta).clamp(0, selectable.len() as isize - 1);
    selectable[target as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tabs_and_search() {
        assert_eq!(rows(Tab::Lessons, "").len(), lessons::LESSONS.len());
        let reference = rows(Tab::Reference, "");
        assert_eq!(reference.iter().filter(|r| matches!(r, Row::Reference(_))).count(), lessons::REFERENCE.len());
        assert!(matches!(reference[0], Row::Section(_)));
        let found = rows(Tab::Lessons, "rythme");
        assert!(!found.is_empty() && found.len() < lessons::LESSONS.len());
        assert!(rows(Tab::Reference, "zzzz-rien").is_empty());
        assert_eq!(Tab::Lessons.other(), Tab::Reference);
    }

    #[test]
    fn selection_skips_headings() {
        let list = rows(Tab::Reference, "");
        let first = move_selection(&list, 0, 0);
        assert_eq!(first, 1, "the first row is a heading");
        let mut i = first;
        for _ in 0..list.len() {
            i = move_selection(&list, i, 1);
            assert!(list[i].selectable());
        }
        assert_eq!(i, list.len() - 1);
        assert_eq!(move_selection(&list, i, 1000), i);
        assert_eq!(move_selection(&list, i, -1000), first);
        assert_eq!(move_selection(&list, list.len() + 5, 0), list.len() - 1);
        assert_eq!(move_selection(&[], 3, 1), 0);
    }
}
