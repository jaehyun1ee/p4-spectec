//! Width-sensitive resolution into concrete lines and cells
//!
//! `resolve` starts at column zero;
//! `resolve_doc` reserves pending suffixes and `resolve_in_mode` selects breaks.
//! Fills pack greedily, while grid columns stabilize across all rows.

use super::{doc::*, link, width as measure};
use crate::backend::latex::error::{Error, Result};

/// Chooses how soft breaks render within one layout group.
#[derive(Clone, Copy)]
enum Mode {
    /// Keeps soft breaks as their empty or space spelling.
    Flat,
    /// Starts an indented continuation at each soft break.
    Broken,
}

// == Resolved lines

/// Builds continuation indentation from quads and an optional space.
fn doc_of_indent(column_next: usize) -> Doc {
    let mut docs = vec![Doc::Quad; column_next / 2];
    if !column_next.is_multiple_of(2) {
        docs.push(Doc::Space);
    }
    concat(docs)
}

/// Exposes concrete left-stack rows while retaining other wrappers.
fn lines_of_doc(doc: Doc) -> Vec<Doc> {
    match doc {
        Doc::LeftStack(docs) => docs,
        doc => vec![doc],
    }
}

/// Collapses zero or one resolved lines without filtering empty rows.
fn doc_of_lines(mut docs: Vec<Doc>) -> Doc {
    match docs.len() {
        0 => Doc::Empty,
        1 => docs.pop().unwrap(),
        _ => Doc::LeftStack(docs),
    }
}

/// Joins the last left line with the first right line.
fn concat_lines(mut lines_l: Vec<Doc>, mut lines_r: Vec<Doc>) -> Vec<Doc> {
    // Preserve the source's reversed-left result when the right side is empty
    if lines_r.is_empty() {
        lines_l.reverse();
        return lines_l;
    }
    // An empty left sequence contributes no prefix
    let Some(line_l) = lines_l.pop() else {
        return lines_r;
    };
    let line_r = lines_r.remove(0);
    lines_l.push(concat(vec![line_l, line_r]));
    lines_l.extend(lines_r);
    lines_l
}

/// Measures the final column, including the initial offset on one line.
fn column_after_lines(column: usize, lines: &[Doc]) -> usize {
    match lines {
        [] => column,
        [doc] => column + measure::flat(doc),
        docs => measure::flat(docs.last().unwrap()),
    }
}

// == Suffix widths

/// Finds the same-mode prefix width and whether a soft break ends it.
fn width_before_break(doc: &Doc) -> (usize, bool) {
    match doc {
        Doc::Concat(docs) => width_before_break_in_docs(docs.iter()),
        Doc::Group(doc)
        | Doc::Mathbin(doc)
        | Doc::Mathrel(doc)
        | Doc::Displaystyle(doc)
        | Doc::Link(_, doc)
        | Doc::Nest(_, doc) => width_before_break(doc),
        Doc::SoftBreak(_) => (0, true),
        Doc::Fill(_, separator, docs) => width_before_break_in_docs(interspersed(separator, docs)),
        _ => (measure::flat(doc), false),
    }
}

/// Counts only the prefix before the first same-mode soft break.
fn width_before_break_in_docs<'a>(docs: impl IntoIterator<Item = &'a Doc>) -> (usize, bool) {
    let mut width = 0;
    // Nested layout groups choose their own mode and remain atomic here
    for doc in docs {
        let (width_doc, has_break) = width_before_break(doc);
        width += width_doc;
        if has_break {
            return (width, true);
        }
    }
    (width, false)
}

/// Reserves either the full suffix or only its first unbroken prefix.
fn width_suffix_of_docs(mode: Mode, width_suffix: usize, docs: &[Doc]) -> usize {
    match mode {
        Mode::Flat => width_suffix + docs.iter().map(measure::flat).sum::<usize>(),
        Mode::Broken => {
            let (width, has_break) = width_before_break_in_docs(docs.iter());
            width + if has_break { 0 } else { width_suffix }
        }
    }
}

// == Document resolution

/// Resolves a document at a strictly positive line width.
pub(crate) fn resolve(width: usize, doc: &Doc) -> Result<Doc> {
    if width == 0 {
        return Err(Error::InvalidLayoutWidth);
    }
    Ok(resolve_doc(width, 0, 0, 0, doc))
}

/// Resolves children at their current column and reserved suffix width.
fn resolve_doc(
    width: usize,
    column: usize,
    column_next: usize,
    width_suffix: usize,
    doc: &Doc,
) -> Doc {
    match doc {
        Doc::Concat(docs) => {
            let mut lines = vec![Doc::Empty];
            for (idx, doc) in docs.iter().enumerate() {
                let width_suffix_doc =
                    width_suffix_of_docs(Mode::Flat, width_suffix, &docs[idx + 1..]);
                let column_doc = column_after_lines(column, &lines);
                let doc = resolve_doc(width, column_doc, column_next, width_suffix_doc, doc);
                lines = concat_lines(lines, lines_of_doc(doc));
            }
            doc_of_lines(lines)
        }
        Doc::Group(doc) => {
            Doc::Group(Box::new(resolve_doc(width, column, column_next, width_suffix, doc)))
        }
        Doc::Mathbin(doc) => {
            Doc::Mathbin(Box::new(resolve_doc(width, column, column_next, width_suffix, doc)))
        }
        Doc::Mathrel(doc) => {
            Doc::Mathrel(Box::new(resolve_doc(width, column, column_next, width_suffix, doc)))
        }
        Doc::Displaystyle(doc) => {
            Doc::Displaystyle(Box::new(resolve_doc(width, column, column_next, width_suffix, doc)))
        }
        Doc::Delimited(delimiter, doc) => {
            let width_child = width.saturating_sub(measure::DELIMITER_MARGIN).max(1);
            let doc = resolve_doc(width_child, column, column_next, width_suffix, doc);
            Doc::Delimited(*delimiter, Box::new(doc))
        }
        Doc::Subscript(doc_base, doc_sub) => {
            let doc_base = resolve_doc(
                width,
                column,
                column_next,
                width_suffix + measure::flat_script_width(doc_sub),
                doc_base,
            );
            let width_child = width_script_budget(width, column, width_suffix, &doc_base);
            let doc_sub = resolve_doc(width_child, 0, column_next, 0, doc_sub);
            Doc::Subscript(Box::new(doc_base), Box::new(doc_sub))
        }
        Doc::Superscript(doc_base, doc_sup) => {
            let doc_base = resolve_doc(
                width,
                column,
                column_next,
                width_suffix + measure::flat_script_width(doc_sup),
                doc_base,
            );
            let width_child = width_script_budget(width, column, width_suffix, &doc_base);
            let doc_sup = resolve_doc(width_child, 0, column_next, 0, doc_sup);
            Doc::Superscript(Box::new(doc_base), Box::new(doc_sup))
        }
        Doc::Subsup(doc_base, doc_sub, doc_sup) => {
            let width_script =
                measure::flat_script_width(doc_sub).max(measure::flat_script_width(doc_sup));
            let doc_base =
                resolve_doc(width, column, column_next, width_suffix + width_script, doc_base);
            let width_child = width_script_budget(width, column, width_suffix, &doc_base);
            let doc_sub = resolve_doc(width_child, 0, column_next, 0, doc_sub);
            let doc_sup = resolve_doc(width_child, 0, column_next, 0, doc_sup);
            Doc::Subsup(Box::new(doc_base), Box::new(doc_sub), Box::new(doc_sup))
        }
        Doc::Fraction(doc_num, doc_den) => {
            let width_child = width.saturating_sub(measure::FRACTION_MARGIN).max(1);
            let doc_num = resolve_doc(width_child, 0, 0, 0, doc_num);
            let doc_den = resolve_doc(width_child, 0, 0, 0, doc_den);
            Doc::Fraction(Box::new(doc_num), Box::new(doc_den))
        }
        Doc::Link(target, doc) => {
            let doc = resolve_doc(width, column, column_next, width_suffix, doc);
            link::normalize_after_layout(target, doc)
        }
        Doc::SoftBreak(Soft::SoftCut) => Doc::Empty,
        Doc::SoftBreak(Soft::SoftSpace) => Doc::Space,
        Doc::LayoutGroup(doc) => {
            resolve_layout_group(width, column, column_next, width_suffix, doc)
        }
        Doc::Nest(indent, doc) => {
            resolve_doc(width, column, column_next + indent, width_suffix, doc)
        }
        Doc::Fill(indent, separator, docs) => {
            resolve_fill(width, column, column_next + indent, width_suffix, separator, docs)
        }
        Doc::Aligned(rows) => {
            Doc::Aligned(resolve_rows(width, None, rows.iter().map(Vec::as_slice)))
        }
        Doc::Grid(alignments, rows) => resolve_grid(width, alignments, rows),
        Doc::Stacked(docs) => Doc::Stacked(
            docs.iter()
                .map(|doc| resolve_doc(width, 0, 0, 0, doc))
                .collect(),
        ),
        Doc::LeftStack(docs) => Doc::LeftStack(
            docs.iter()
                .map(|doc| resolve_doc(width, 0, 0, 0, doc))
                .collect(),
        ),
        Doc::Numbered(docs) => {
            let width_body = width
                .saturating_sub(measure::flat_numbered_gutter(docs.len()))
                .max(1);
            Doc::Numbered(
                docs.iter()
                    .map(|doc| resolve_doc(width_body, 0, 0, 0, doc))
                    .collect(),
            )
        }
        Doc::Gathered(blocks) => Doc::Gathered(
            blocks
                .iter()
                .map(|block| match block {
                    Block::Line(doc) => Block::Line(resolve_doc(width, 0, 0, 0, doc)),
                    Block::Gap => Block::Gap,
                })
                .collect(),
        ),
        doc => doc.clone(),
    }
}

/// Converts remaining normal-size columns into a positive script budget.
fn width_script_budget(width: usize, column: usize, width_suffix: usize, doc_base: &Doc) -> usize {
    (2 * width.saturating_sub(column + width_suffix + measure::flat(doc_base))).max(1)
}

/// Chooses a flat or broken mode including the pending suffix width.
fn resolve_layout_group(
    width: usize,
    column: usize,
    column_next: usize,
    width_suffix: usize,
    doc: &Doc,
) -> Doc {
    let mode =
        if column + measure::flat(doc) + width_suffix <= width { Mode::Flat } else { Mode::Broken };
    let lines = resolve_in_mode(mode, width, column, column_next, width_suffix, doc);
    doc_of_lines(lines)
}

// == Greedy fills

/// Packs subsequent items while reserving the enclosing suffix for the last.
fn resolve_fill(
    width: usize,
    column: usize,
    column_next: usize,
    width_suffix: usize,
    separator: &Doc,
    docs: &[Doc],
) -> Doc {
    // The first item starts on the current line without a separator
    let Some((doc_head, docs)) = docs.split_first() else {
        return Doc::Empty;
    };
    let width_suffix_head = if docs.is_empty() { width_suffix } else { 0 };
    let doc_head = resolve_doc(width, column, column_next, width_suffix_head, doc_head);
    let mut lines = lines_of_doc(doc_head);
    // Overflow starts a fresh indented line and drops the separator
    for (idx, doc) in docs.iter().enumerate() {
        let width_suffix_doc = if idx + 1 == docs.len() { width_suffix } else { 0 };
        let column_current = column_after_lines(column, &lines);
        let width_needed = measure::flat(separator) + measure::flat(doc) + width_suffix_doc;
        if column_current + width_needed <= width {
            let column_doc = column_current + measure::flat(separator);
            let doc = resolve_doc(width, column_doc, column_next, width_suffix_doc, doc);
            let doc = concat(vec![separator.clone(), doc]);
            lines = concat_lines(lines, lines_of_doc(doc));
        } else {
            let doc = resolve_doc(width, column_next, column_next, width_suffix_doc, doc);
            let mut lines_doc = lines_of_doc(doc);
            if let Some(line_head) = lines_doc.first_mut() {
                let doc_head = std::mem::replace(line_head, Doc::Empty);
                *line_head = concat(vec![doc_of_indent(column_next), doc_head]);
            }
            lines.extend(lines_doc);
        }
    }
    doc_of_lines(lines)
}

// == Shared grid columns

/// Resolves each cell with space reserved for the other columns.
fn resolve_cells(
    width: usize,
    column_widths: &[usize],
    alignments: Option<&[Alignment]>,
    docs: &[Doc],
) -> Vec<Doc> {
    let mut column = 0;
    let mut docs_resolved = Vec::new();
    // Candidate widths determine both alignment padding and remaining space
    for (idx, doc) in docs.iter().enumerate() {
        let width_cell = column_widths
            .get(idx)
            .copied()
            .unwrap_or_else(|| measure::flat(doc));
        let widths_remaining = column_widths.get(idx + 1..);
        let width_remaining = match widths_remaining {
            // Shared columns include one gap each after the current cell
            Some(widths) => {
                widths.iter().sum::<usize>() + measure::INTERCOLUMN_SPACING * widths.len()
            }
            // A ragged row can introduce columns absent from the candidate
            None => {
                docs[idx + 1..].iter().map(measure::flat).sum::<usize>()
                    + measure::INTERCOLUMN_SPACING * (docs.len() - idx - 1)
            }
        };
        let padding = match alignments.and_then(|alignments| alignments.get(idx)) {
            Some(Alignment::Center) => width_cell.saturating_sub(measure::flat(doc)) / 2,
            Some(Alignment::Right) => width_cell.saturating_sub(measure::flat(doc)),
            _ => 0,
        };
        let width_local = width.saturating_sub(column + width_remaining).max(1);
        docs_resolved.push(resolve_doc(width_local, padding, 0, 0, doc));
        column += width_cell + measure::INTERCOLUMN_SPACING;
    }
    docs_resolved
}

/// Stabilizes shared widths and retains the narrowest candidate on a cycle.
fn resolve_rows<'a>(
    width: usize,
    alignments: Option<&[Alignment]>,
    rows: impl Iterator<Item = &'a [Doc]> + Clone,
) -> Vec<Vec<Doc>> {
    let mut column_widths = measure::flat_column_widths(rows.clone());
    let mut seen = vec![column_widths.clone()];
    let mut best: Option<(Vec<usize>, Vec<Vec<Doc>>)> = None;
    // Always resolve the original rows at the current candidate widths
    loop {
        let rows_resolved: Vec<_> = rows
            .clone()
            .map(|docs| resolve_cells(width, &column_widths, alignments, docs))
            .collect();
        let column_widths_resolved =
            measure::flat_column_widths(rows_resolved.iter().map(Vec::as_slice));
        if best.as_ref().is_none_or(|(widths_best, _)| {
            measure::flat_columns(&column_widths_resolved) < measure::flat_columns(widths_best)
        }) {
            best = Some((column_widths_resolved.clone(), rows_resolved.clone()));
        }
        // A fixed point wins even if an earlier candidate was narrower
        if column_widths_resolved == column_widths {
            return rows_resolved;
        }
        if seen.contains(&column_widths_resolved) {
            return best.unwrap().1;
        }
        seen.push(column_widths_resolved.clone());
        column_widths = column_widths_resolved;
    }
}

/// Replaces cell rows in order and independently resolves spanning rows.
fn resolve_grid(width: usize, alignments: &[Alignment], rows: &[GridRow]) -> Doc {
    let rows_cell = rows.iter().filter_map(|row| match row {
        GridRow::Cells(docs) => Some(docs.as_slice()),
        _ => None,
    });
    let rows_cell = resolve_rows(width, Some(alignments), rows_cell);
    let mut rows_cell = rows_cell.into_iter();
    // The same cell-row filter determines both production and consumption
    let rows = rows
        .iter()
        .map(|row| match row {
            GridRow::Cells(_) => GridRow::Cells(
                rows_cell
                    .next()
                    .expect("each input cell row has one resolved row"),
            ),
            GridRow::Spanning(doc) => GridRow::Spanning(resolve_doc(width, 0, 0, 0, doc)),
            GridRow::Gap => GridRow::Gap,
        })
        .collect();
    Doc::Grid(alignments.to_vec(), rows)
}

// == Mode-specific resolution

/// Propagates one mode through wrappers while nested groups choose afresh.
fn resolve_in_mode(
    mode: Mode,
    width: usize,
    column: usize,
    column_next: usize,
    width_suffix: usize,
    doc: &Doc,
) -> Vec<Doc> {
    match doc {
        Doc::Concat(docs) => {
            let mut lines = vec![Doc::Empty];
            for (idx, doc) in docs.iter().enumerate() {
                let width_suffix_doc = width_suffix_of_docs(mode, width_suffix, &docs[idx + 1..]);
                let column_doc = column_after_lines(column, &lines);
                let lines_doc =
                    resolve_in_mode(mode, width, column_doc, column_next, width_suffix_doc, doc);
                lines = concat_lines(lines, lines_doc);
            }
            lines
        }
        Doc::Group(doc) => resolve_in_mode(mode, width, column, column_next, width_suffix, doc)
            .into_iter()
            .map(|doc| Doc::Group(Box::new(doc)))
            .collect(),
        Doc::Mathbin(doc) => resolve_in_mode(mode, width, column, column_next, width_suffix, doc)
            .into_iter()
            .map(|doc| Doc::Mathbin(Box::new(doc)))
            .collect(),
        Doc::Mathrel(doc) => resolve_in_mode(mode, width, column, column_next, width_suffix, doc)
            .into_iter()
            .map(|doc| Doc::Mathrel(Box::new(doc)))
            .collect(),
        Doc::Displaystyle(doc) => {
            let lines = resolve_in_mode(mode, width, column, column_next, width_suffix, doc);
            vec![Doc::Displaystyle(Box::new(doc_of_lines(lines)))]
        }
        Doc::Delimited(delimiter, doc) => {
            let width_child = width.saturating_sub(measure::DELIMITER_MARGIN).max(1);
            let lines = resolve_in_mode(mode, width_child, column, column_next, width_suffix, doc);
            vec![Doc::Delimited(*delimiter, Box::new(doc_of_lines(lines)))]
        }
        Doc::Subscript(doc_base, doc_sub) => {
            let width_script = measure::flat_script_width(doc_sub);
            let lines = resolve_in_mode(
                mode,
                width,
                column,
                column_next,
                width_suffix + width_script,
                doc_base,
            );
            let doc_base = doc_of_lines(lines);
            let width_child = width_script_budget(width, column, width_suffix, &doc_base);
            let lines = resolve_in_mode(mode, width_child, 0, column_next, 0, doc_sub);
            let doc_sub = doc_of_lines(lines);
            vec![Doc::Subscript(Box::new(doc_base), Box::new(doc_sub))]
        }
        Doc::Superscript(doc_base, doc_sup) => {
            let width_script = measure::flat_script_width(doc_sup);
            let lines = resolve_in_mode(
                mode,
                width,
                column,
                column_next,
                width_suffix + width_script,
                doc_base,
            );
            let doc_base = doc_of_lines(lines);
            let width_child = width_script_budget(width, column, width_suffix, &doc_base);
            let lines = resolve_in_mode(mode, width_child, 0, column_next, 0, doc_sup);
            let doc_sup = doc_of_lines(lines);
            vec![Doc::Superscript(Box::new(doc_base), Box::new(doc_sup))]
        }
        Doc::Subsup(doc_base, doc_sub, doc_sup) => {
            let width_script =
                measure::flat_script_width(doc_sub).max(measure::flat_script_width(doc_sup));
            let lines = resolve_in_mode(
                mode,
                width,
                column,
                column_next,
                width_suffix + width_script,
                doc_base,
            );
            let doc_base = doc_of_lines(lines);
            let width_child = width_script_budget(width, column, width_suffix, &doc_base);
            let lines = resolve_in_mode(mode, width_child, 0, column_next, 0, doc_sub);
            let doc_sub = doc_of_lines(lines);
            let lines = resolve_in_mode(mode, width_child, 0, column_next, 0, doc_sup);
            let doc_sup = doc_of_lines(lines);
            vec![Doc::Subsup(Box::new(doc_base), Box::new(doc_sub), Box::new(doc_sup))]
        }
        Doc::Fraction(doc_num, doc_den) => {
            let width_child = width.saturating_sub(measure::FRACTION_MARGIN).max(1);
            let lines = resolve_in_mode(mode, width_child, 0, 0, 0, doc_num);
            let doc_num = doc_of_lines(lines);
            let lines = resolve_in_mode(mode, width_child, 0, 0, 0, doc_den);
            let doc_den = doc_of_lines(lines);
            vec![Doc::Fraction(Box::new(doc_num), Box::new(doc_den))]
        }
        Doc::Link(target, doc) => {
            resolve_in_mode(mode, width, column, column_next, width_suffix, doc)
                .into_iter()
                .map(|doc| link::link_unowned_doc(target, doc))
                .collect()
        }
        Doc::SoftBreak(soft) => match (mode, soft) {
            (Mode::Flat, Soft::SoftCut) => vec![Doc::Empty],
            (Mode::Flat, Soft::SoftSpace) => vec![Doc::Space],
            (Mode::Broken, _) => vec![Doc::Empty, doc_of_indent(column_next)],
        },
        Doc::LayoutGroup(doc) => {
            lines_of_doc(resolve_layout_group(width, column, column_next, width_suffix, doc))
        }
        Doc::Nest(indent, doc) => {
            resolve_in_mode(mode, width, column, column_next + indent, width_suffix, doc)
        }
        doc => vec![resolve_doc(width, column, column_next, width_suffix, doc)],
    }
}
