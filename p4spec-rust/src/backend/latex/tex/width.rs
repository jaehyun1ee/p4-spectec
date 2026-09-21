//! Approximate widths of unresolved mathematical documents
//!
//! `flat` measures one unresolved line using byte lengths for text;
//! shared column and script measurements also guide layout budgets.

use super::doc::*;

// == Width parameters

pub(super) const INTERCOLUMN_SPACING: usize = 2;
pub(super) const DELIMITER_MARGIN: usize = 2;
pub(super) const FRACTION_MARGIN: usize = 2;

// == Flat widths

/// Measures the approximate width of one unresolved line.
pub(crate) fn flat(doc: &Doc) -> usize {
    match doc {
        Doc::Empty => 0,
        Doc::Styled(_, text) | Doc::Badge(text) => text.len(),
        Doc::Decimal(num) => num.to_string().len(),
        Doc::Hexadecimal(num) => format!("{num:#x}").len(),
        Doc::Fixed(symbol) => string_of_symbol(*symbol).len().max(1),
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
        Doc::Subscript(doc_base, doc_sub) | Doc::Superscript(doc_base, doc_sub) => {
            flat(doc_base) + flat_script_width(doc_sub)
        }
        Doc::Subsup(doc_base, doc_sub, doc_sup) => {
            flat(doc_base) + flat_script_width(doc_sub).max(flat_script_width(doc_sup))
        }
        Doc::Fraction(doc_num, doc_den) => FRACTION_MARGIN + flat(doc_num).max(flat(doc_den)),
        Doc::SoftBreak(Soft::SoftCut) => 0,
        Doc::SoftBreak(Soft::SoftSpace) => 1,
        Doc::Fill(_, separator, docs) => interspersed(separator, docs).map(flat).sum(),
        Doc::Aligned(rows) => flat_columns(&flat_column_widths(rows.iter().map(Vec::as_slice))),
        Doc::Grid(_, rows) => {
            let rows_cell = rows.iter().filter_map(|row| match row {
                Row::Cells(docs) => Some(docs.as_slice()),
                _ => None,
            });
            let width_spanning = rows
                .iter()
                .filter_map(|row| match row {
                    Row::Spanning(doc) => Some(flat(doc)),
                    _ => None,
                })
                .max()
                .unwrap_or(0);
            flat_columns(&flat_column_widths(rows_cell)).max(width_spanning)
        }
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

/// Selects the approximate textual spelling used to measure a symbol.
fn string_of_symbol(symbol: Symbol) -> &'static str {
    match symbol {
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
    }
}

/// Measures a script at half size, rounded upward.
pub(super) fn flat_script_width(doc: &Doc) -> usize {
    flat(doc).div_ceil(2)
}

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

/// Reserves the numeric label, parentheses, and intercolumn spacing.
pub(super) fn flat_numbered_gutter(count: usize) -> usize {
    count.to_string().len() + 2 + INTERCOLUMN_SPACING
}
