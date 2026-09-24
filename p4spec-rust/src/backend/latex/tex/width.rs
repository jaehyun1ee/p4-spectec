//! Approximate widths of unresolved mathematical documents
//!
//! `flat` measures one unresolved line using byte lengths for text;
//! shared column and script measurements also guide layout budgets.

use super::doc::*;

// == Width parameters

pub(super) const INTERCOLUMN_SPACING: usize = 2;
pub(super) const DELIMITER_MARGIN: usize = 2;
pub(super) const FRACTION_MARGIN: usize = 2;

// == Measurement helpers

// - Scripts

/// Measures a script at half size, rounded upward.
pub(super) fn flat_script_width(doc: &Doc) -> usize {
    flat(doc).div_ceil(2)
}

// - Columns

/// Measures columns including the gaps between adjacent columns.
pub(super) fn flat_columns(widths: &[usize]) -> usize {
    widths.iter().sum::<usize>() + INTERCOLUMN_SPACING * widths.len().saturating_sub(1)
}

/// Finds the maximum width of each existing column across ragged rows.
pub(super) fn flat_column_widths<'a>(rows: impl IntoIterator<Item = &'a [Doc]>) -> Vec<usize> {
    let mut widths = Vec::new();
    // Merge each row without discarding columns absent from a shorter row
    for docs in rows {
        for (idx, doc) in docs.iter().enumerate() {
            if idx == widths.len() {
                widths.push(flat(doc));
            } else {
                widths[idx] = widths[idx].max(flat(doc));
            }
        }
    }
    widths
}

// - Numbered gutters

/// Reserves the numeric label, parentheses, and intercolumn spacing.
pub(super) fn flat_numbered_gutter(count: usize) -> usize {
    count.to_string().len() + 2 + INTERCOLUMN_SPACING
}

// == Documents

// - Document

/// Measures the approximate width of one unresolved line.
pub(crate) fn flat(doc: &Doc) -> usize {
    match doc {
        Doc::Empty => 0,
        Doc::Styled(_, text) | Doc::Badge(text) => text.len(),
        Doc::Decimal(num) => num.to_string().len(),
        Doc::Hexadecimal(num) => format!("{num:#x}").len(),
        Doc::Fixed(symbol) => flat_fixed_doc(*symbol),
        Doc::Space | Doc::ThinSpace => 1,
        Doc::Quad => 2,
        Doc::Concat(docs) => docs.iter().map(flat).sum(),
        Doc::Group(doc)
        | Doc::Mathbin(doc)
        | Doc::Mathrel(doc)
        | Doc::Displaystyle(doc)
        | Doc::Link(_, doc)
        | Doc::LayoutGroup(doc)
        | Doc::Nest(_, doc) => flat(doc),
        Doc::Delimited(_, doc) => DELIMITER_MARGIN + flat(doc),
        Doc::Sub(doc_base, doc_sub) | Doc::Sup(doc_base, doc_sub) => {
            flat(doc_base) + flat_script_width(doc_sub)
        }
        Doc::Subsup(doc_base, doc_sub, doc_sup) => {
            flat(doc_base) + flat_script_width(doc_sub).max(flat_script_width(doc_sup))
        }
        Doc::Fraction(doc_num, doc_den) => FRACTION_MARGIN + flat(doc_num).max(flat(doc_den)),
        Doc::SoftBreak(Soft::SoftCut) => 0,
        Doc::SoftBreak(Soft::SoftSpace) => 1,
        Doc::Fill(_, separator, docs) => Doc::fill_line(separator, docs).map(flat).sum(),
        Doc::Aligned(rows) => flat_columns(&flat_column_widths(rows.iter().map(Vec::as_slice))),
        Doc::Grid(_, rows) => flat_grid_doc(rows),
        Doc::Stacked(docs) | Doc::LeftStack(docs) => docs.iter().map(flat).max().unwrap_or(0),
        Doc::Numbered(docs) => {
            flat_numbered_gutter(docs.len()) + docs.iter().map(flat).max().unwrap_or(0)
        }
        Doc::Gathered(blocks) => blocks
            .iter()
            .map(|block| match block {
                Block::Line(doc) => flat(doc),
                Block::Gap => 0,
            })
            .max()
            .unwrap_or(0),
    }
}

// - Fixed document

/// Measures a fixed symbol using its approximate textual spelling.
fn flat_fixed_doc(symbol: Symbol) -> usize {
    let text = match symbol {
        Symbol::Equal => "=",
        Symbol::NotEqual => "=/=",
        Symbol::Less => "<",
        Symbol::Greater => ">",
        Symbol::LessEqual => "<=",
        Symbol::GreaterEqual => ">=",
        Symbol::Plus => "+",
        Symbol::Minus => "-",
        Symbol::Question => "?",
        Symbol::Ast => "*",
        Symbol::Slash => "/",
        Symbol::Comma => ",",
        Symbol::Semicolon => ";",
        Symbol::Colon => ":",
        Symbol::DoubleColon => "::",
        Symbol::Cat => "++",
        Symbol::Production => "::=",
        Symbol::VerticalBar => "|",
        Symbol::Dot => ".",
        Symbol::Dot2 => "..",
        Symbol::Ellipsis => "...",
        Symbol::Epsilon => "e",
        Symbol::In => "in",
        Symbol::Neg => "~",
        Symbol::Land => "/\\",
        Symbol::Lor => "\\/",
        Symbol::Rightarrow => "=>",
        Symbol::Leftrightarrow => "<=>",
        Symbol::Cdot => "*",
        Symbol::Bmod => "\\",
        Symbol::Turnstile => "|-",
        Symbol::Tilesturn => "-|",
        Symbol::To => "->",
        Symbol::Longrightarrow => "==>",
        Symbol::Hookrightarrow => "~>",
        Symbol::Mapsto => "|->",
        Symbol::Sim => "~~",
        Symbol::Setminus => "\\",
        Symbol::EmptySet => "0",
        Symbol::LeftParen => "(",
        Symbol::RightParen => ")",
        Symbol::LeftBracket => "[",
        Symbol::RightBracket => "]",
        Symbol::LeftBrace => "{",
        Symbol::RightBrace => "}",
    };
    text.len().max(1)
}

// - Grid document

/// Measures the wider of shared columns and spanning rows.
fn flat_grid_doc(rows: &[GridRow]) -> usize {
    // Measure shared columns independently of spanning content
    let rows_cell = rows.iter().filter_map(|row| match row {
        GridRow::Cells(docs) => Some(docs.as_slice()),
        _ => None,
    });
    // Retain the widest span even when there are no cell rows
    let width_spanning = rows
        .iter()
        .filter_map(|row| match row {
            GridRow::Spanning(doc) => Some(flat(doc)),
            _ => None,
        })
        .max()
        .unwrap_or(0);
    flat_columns(&flat_column_widths(rows_cell)).max(width_spanning)
}
