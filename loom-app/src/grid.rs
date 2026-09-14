//! The step-grid view model: what each voice plays in a given bar, quantised
//! to columns, ready for a TUI or GUI to draw with a moving playhead.

use loom_core::Program;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Cell {
    Rest,
    /// A hit, with its velocity (accents are > 1).
    Hit(f32),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub name: String,
    pub is_drum: bool,
    pub cells: Vec<Cell>,
    /// The first note played in each column (melodic voices), for labels.
    pub notes: Vec<Option<String>>,
}

/// Grid for `bar` with `columns` steps. Every voice in the program gets a row
/// (in source order), even when it is silent this bar, so rows don't jump.
pub fn grid(program: &Program, bar: i64, columns: usize) -> Vec<Row> {
    let columns = columns.max(1);
    let mut rows: Vec<Row> = Vec::new();
    for v in &program.voices {
        if rows.iter().any(|r| r.name == v.name) { continue; }
        rows.push(Row { name: v.name.clone(), is_drum: v.is_drum, cells: vec![Cell::Rest; columns], notes: vec![None; columns] });
    }
    for ev in program.schedule_bar(bar) {
        let Some(row) = rows.iter_mut().find(|r| r.name == ev.voice) else { continue };
        let col = ((ev.phase.clamp(0.0, 0.9999)) * columns as f32) as usize;
        let vel = match row.cells[col] { Cell::Hit(v) => v.max(ev.vel), Cell::Rest => ev.vel };
        row.cells[col] = Cell::Hit(vel);
        if !ev.is_drum && row.notes[col].is_none() { row.notes[col] = Some(ev.token.clone()); }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn places_hits_in_columns() {
        let p = Program::parse("kick: x . . . x . . .\nbass: c2 . eb2 .");
        let g = grid(&p, 0, 8);
        assert_eq!(g.len(), 2);
        let hits: Vec<bool> = g[0].cells.iter().map(|c| matches!(c, Cell::Hit(_))).collect();
        assert_eq!(hits, [true, false, false, false, true, false, false, false]);
        assert!(g[0].is_drum && !g[1].is_drum);
        assert_eq!(g[1].notes[4].as_deref(), Some("eb2"));
    }
}
