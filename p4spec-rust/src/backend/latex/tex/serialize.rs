//! TeX serialization through the shared printer
//!
//! `to_string` validates document and row structure before invoking `Print`.
//! `render_doc` dispatches to text, mathematical, and row-layout writers;
//! each writer preserves the canonical TeX spelling through `fmt::Write`.
//! Mixed grids render visible content once;
//! `render_grid_phantom` reserves its width with link-free invisible copies.

use super::{doc::*, link};
use crate::backend::latex::error::{Error, Result};
use crate::lang::traits::print::{Print, Printer};
use std::fmt::{self, Write};

// == Entry points

/// Validates a document and renders it through its context-free printer.
pub(crate) fn to_string(doc: &Doc) -> Result<String> {
    validate(doc)?;
    Ok(Print::to_string(doc))
}

impl Print for Doc {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        render_doc(printer, self)
    }
}

// == Document dispatch

/// Dispatches mathematical and layout nodes to balanced TeX output.
fn render_doc(output: &mut dyn Write, doc: &Doc) -> fmt::Result {
    match doc {
        Doc::Empty | Doc::SoftBreak(Soft::SoftCut) => Ok(()),
        Doc::Styled(style, text) => render_styled(output, *style, text),
        Doc::Badge(text) => render_badge(output, text),
        Doc::Decimal(num) => write!(output, "{num}"),
        Doc::Hexadecimal(num) => write!(output, "\\mathtt{{{num:#x}}}"),
        Doc::Fixed(symbol) => output.write_str(string_of_symbol(*symbol)),
        Doc::Space | Doc::SoftBreak(Soft::SoftSpace) => output.write_str(" "),
        Doc::ThinSpace => output.write_str(r"\,"),
        Doc::Quad => output.write_str(r"\quad"),
        Doc::Concat(docs) => render_docs(output, docs),
        Doc::Group(doc) => render_enclosed(output, "{", "}", doc),
        Doc::Mathbin(doc) => render_enclosed(output, r"\mathbin{", "}", doc),
        Doc::Mathrel(doc) => render_enclosed(output, r"\mathrel{", "}", doc),
        Doc::Displaystyle(doc) => render_enclosed(output, r"{\displaystyle ", "}", doc),
        Doc::Delimited(delimiter, doc) => render_delimited(output, *delimiter, doc),
        Doc::Subscript(doc_base, doc_sub) => {
            render_enclosed(output, "{", "}_{", doc_base)?;
            render_enclosed(output, "", "}", doc_sub)
        }
        Doc::Superscript(doc_base, doc_sup) => {
            render_enclosed(output, "{", "}^{", doc_base)?;
            render_enclosed(output, "", "}", doc_sup)
        }
        Doc::Subsup(doc_base, doc_sub, doc_sup) => {
            render_enclosed(output, "{", "}_{", doc_base)?;
            render_enclosed(output, "", "}^{", doc_sub)?;
            render_enclosed(output, "", "}", doc_sup)
        }
        Doc::Fraction(doc_num, doc_den) => {
            render_enclosed(output, r"\frac{", "}{", doc_num)?;
            render_enclosed(output, "", "}", doc_den)
        }
        Doc::Link(target, doc) => {
            write!(output, "\\href{{#{}}}{{", target.0)?;
            render_enclosed(output, "", "}", doc)
        }
        Doc::LayoutGroup(doc) | Doc::Nest(_, doc) => render_doc(output, doc),
        Doc::Fill(_, separator, docs) => render_docs(output, interspersed(separator, docs)),
        Doc::Aligned(rows) => render_aligned(output, rows),
        Doc::Grid(alignments, rows) => render_grid(output, alignments, rows),
        Doc::Stacked(docs) => render_stacked(output, docs),
        Doc::LeftStack(docs) => {
            render_array(output, &[Alignment::Left], docs.iter().map(std::slice::from_ref))
        }
        Doc::Numbered(docs) => render_numbered(output, docs),
        Doc::Gathered(blocks) => render_gathered(output, blocks),
    }
}

/// Writes a borrowed document sequence without implicit separators.
fn render_docs<'a>(output: &mut dyn Write, docs: impl IntoIterator<Item = &'a Doc>) -> fmt::Result {
    for doc in docs {
        render_doc(output, doc)?;
    }
    Ok(())
}

// == Text and mathematical atoms

// - Escaping and styled runs

/// Writes one character with escaping for the selected TeX context.
fn escape_char(output: &mut dyn Write, char: char, math: bool) -> fmt::Result {
    match char {
        '#' => output.write_str(r"\#"),
        '$' => output.write_str(r"\$"),
        '%' => output.write_str(r"\%"),
        '&' => output.write_str(r"\&"),
        '_' => output.write_str(r"\_"),
        '{' => output.write_str(r"\{"),
        '}' => output.write_str(r"\}"),
        '\\' if math => output.write_str(r"\backslash{}"),
        '^' if math => output.write_str(r"\hat{}"),
        '~' if math => output.write_str(r"\sim{}"),
        char => output.write_char(char),
    }
}

/// Writes one styled run with the corresponding escaping rules.
fn render_command(output: &mut dyn Write, command: &str, text: &str, math: bool) -> fmt::Result {
    write!(output, "\\{command}{{")?;
    // Escape content without changing its font context
    for char in text.chars() {
        escape_char(output, char, math)?;
    }
    output.write_char('}')
}

/// Splits text around math-only backslash, hat, and tilde glyphs.
fn render_text(output: &mut dyn Write, command: &str, text: &str) -> fmt::Result {
    if text.is_empty() {
        return render_command(output, command, "", false);
    }
    let mut start = 0;
    // Flush ordinary text before emitting a math-only glyph
    for (idx, char) in text.char_indices() {
        if matches!(char, '\\' | '^' | '~') {
            if start < idx {
                render_command(output, command, &text[start..idx], false)?;
            }
            escape_char(output, char, true)?;
            start = idx + char.len_utf8();
        }
    }
    // A final special glyph has no empty trailing text command
    if start < text.len() {
        render_command(output, command, &text[start..], false)?;
    }
    Ok(())
}

/// Selects a font command and the corresponding escaping context.
fn render_styled(output: &mut dyn Write, style: Style, text: &str) -> fmt::Result {
    match style {
        Style::Mathit => render_command(output, "mathit", text, true),
        Style::Mathrm => render_command(output, "mathrm", text, true),
        Style::Mathsf => render_command(output, "mathsf", text, true),
        Style::Mathbb => render_command(output, "mathbb", text, true),
        Style::Mathtt => render_command(output, "mathtt", text, true),
        Style::Text => render_text(output, "text", text),
        Style::Texttt => render_text(output, "texttt", text),
    }
}

// - Rule badges and fixed symbols

/// Writes a shaded rule-label box in small monospace text.
fn render_badge(output: &mut dyn Write, text: &str) -> fmt::Result {
    output.write_str(r"{\definecolor{ellatexrulelabelbg}{rgb}{0.94,0.94,0.92}\fcolorbox{black}{ellatexrulelabelbg}{\scriptsize ")?;
    render_text(output, "texttt", text)?;
    output.write_str("}}")
}

/// Selects the fixed TeX spelling of a mathematical symbol.
fn string_of_symbol(symbol: Symbol) -> &'static str {
    match symbol {
        Symbol::Equal => "=",
        Symbol::NotEqual => "\\ne",
        Symbol::Less => "<",
        Symbol::Greater => ">",
        Symbol::LessEqual => "\\le",
        Symbol::GreaterEqual => "\\ge",
        Symbol::Plus => "+",
        Symbol::Minus => "-",
        Symbol::Question => "?",
        Symbol::Ast => "\\ast",
        Symbol::Slash => "/",
        Symbol::Comma => ",",
        Symbol::Semicolon => ";",
        Symbol::Colon => ":",
        Symbol::DoubleColon => "::",
        Symbol::Cat => "+\\!\\!+",
        Symbol::Production => "::=",
        Symbol::VerticalBar => "|",
        Symbol::Dot => ".",
        Symbol::Dot2 => "..",
        Symbol::Ellipsis => "\\ldots",
        Symbol::Epsilon => "\\epsilon",
        Symbol::In => "\\in",
        Symbol::Neg => "\\neg",
        Symbol::Land => "\\land",
        Symbol::Lor => "\\lor",
        Symbol::Rightarrow => "\\Rightarrow",
        Symbol::Leftrightarrow => "\\Leftrightarrow",
        Symbol::Cdot => "\\cdot",
        Symbol::Bmod => "\\bmod",
        Symbol::Turnstile => "\\vdash",
        Symbol::Tilesturn => "\\dashv",
        Symbol::To => "\\to",
        Symbol::Longrightarrow => "\\Longrightarrow",
        Symbol::Hookrightarrow => "\\hookrightarrow",
        Symbol::Mapsto => "\\mapsto",
        Symbol::Sim => "\\sim",
        Symbol::Setminus => "\\setminus",
        Symbol::EmptySet => "\\varnothing",
        Symbol::LeftParen => "(",
        Symbol::RightParen => ")",
        Symbol::LeftBracket => "[",
        Symbol::RightBracket => "]",
        Symbol::LeftBrace => "\\{",
        Symbol::RightBrace => "\\}",
    }
}

// == Mathematical structure

/// Writes balanced opening and closing syntax around one document.
fn render_enclosed(output: &mut dyn Write, opening: &str, closing: &str, doc: &Doc) -> fmt::Result {
    output.write_str(opening)?;
    render_doc(output, doc)?;
    output.write_str(closing)
}

/// Sizes both delimiters to the enclosed mathematical document.
fn render_delimited(output: &mut dyn Write, delimiter: Delimiter, doc: &Doc) -> fmt::Result {
    // Select both sides together so the delimiter pair stays balanced
    let (text_l, text_r) = match delimiter {
        Delimiter::Paren => ("(", ")"),
        Delimiter::Bracket => ("[", "]"),
        Delimiter::Brace => (r"\{", r"\}"),
        Delimiter::Angle => (r"\langle", r"\rangle"),
        Delimiter::Bar => ("|", "|"),
    };
    // Enclose the content with automatically sized delimiters
    write!(output, "\\left{text_l}")?;
    render_doc(output, doc)?;
    write!(output, "\\right{text_r}")
}

// == Rows and arrays

/// Writes cells in their original order with TeX alignment separators.
fn render_cells(output: &mut dyn Write, docs: &[Doc]) -> fmt::Result {
    // Only intervening cells receive an alignment separator
    for (idx, doc) in docs.iter().enumerate() {
        if idx != 0 {
            output.write_str(" & ")?;
        }
        render_doc(output, doc)?;
    }
    Ok(())
}

/// Writes rows without a trailing line break.
fn render_array_rows<'a>(
    output: &mut dyn Write,
    rows: impl IntoIterator<Item = &'a [Doc]>,
) -> fmt::Result {
    // Array and aligned environments share the same row separators
    for (idx, docs) in rows.into_iter().enumerate() {
        if idx != 0 {
            output.write_str(" \\\\\n")?;
        }
        render_cells(output, docs)?;
    }
    Ok(())
}

/// Writes each column alignment in the supplied order.
fn render_alignments(output: &mut dyn Write, alignments: &[Alignment]) -> fmt::Result {
    // Preserve the declaration order of shared columns
    for alignment in alignments {
        output.write_str(match alignment {
            Alignment::Left => "l",
            Alignment::Center => "c",
            Alignment::Right => "r",
        })?;
    }
    Ok(())
}

/// Encloses aligned rows in a TeX array environment.
fn render_array<'a>(
    output: &mut dyn Write,
    alignments: &[Alignment],
    rows: impl IntoIterator<Item = &'a [Doc]>,
) -> fmt::Result {
    // Declare columns before serializing their content
    output.write_str(r"\begin{array}{")?;
    render_alignments(output, alignments)?;
    output.write_str("}\n")?;
    // Rows own separators; the enclosing environment owns outer newlines
    render_array_rows(output, rows)?;
    output.write_str("\n\\end{array}")
}

/// Writes equation rows in a TeX aligned environment.
fn render_aligned(output: &mut dyn Write, rows: &[Vec<Doc>]) -> fmt::Result {
    output.write_str("\\begin{aligned}\n")?;
    render_array_rows(output, rows.iter().map(Vec::as_slice))?;
    output.write_str("\n\\end{aligned}")
}

/// Writes stacked documents after an empty alignment cell on each row.
fn render_stacked(output: &mut dyn Write, docs: &[Doc]) -> fmt::Result {
    output.write_str("\\begin{aligned}\n")?;
    // Keep each document in the same aligned column
    for (idx, doc) in docs.iter().enumerate() {
        if idx != 0 {
            output.write_str(" \\\\\n")?;
        }
        output.write_str("& ")?;
        render_doc(output, doc)?;
    }
    output.write_str("\n\\end{aligned}")
}

// == Grids

// - Spanning-row geometry

/// Fills every column after a spanning cell with an empty alignment cell.
fn render_grid_empty_cells(output: &mut dyn Write, columns: usize) -> fmt::Result {
    // Preserve the final separator's lack of a trailing space
    for idx in 1..columns {
        output.write_str(if idx + 1 == columns { " &" } else { " & " })?;
    }
    Ok(())
}

/// Places spanning content over a link-free first-column width copy.
fn render_grid_spanning_row(
    output: &mut dyn Write,
    alignments: &[Alignment],
    docs_column_head: &[Doc],
    doc: &Doc,
) -> fmt::Result {
    // A grid without cell rows needs no first-column geometry
    if docs_column_head.is_empty() {
        render_doc(output, doc)?;
    } else {
        output.write_str(r"\mathrlap{\displaystyle ")?;
        render_doc(output, doc)?;
        output.write_str(r"}\smash{\hphantom{")?;
        render_array(
            output,
            &[Alignment::Right],
            docs_column_head.iter().map(std::slice::from_ref),
        )?;
        output.write_str("}}")?;
    }
    render_grid_empty_cells(output, alignments.len())
}

// - Content rows

/// Writes content rows and consumes a following gap as a row separator.
fn render_grid_rows(
    output: &mut dyn Write,
    alignments: &[Alignment],
    docs_column_head: &[Doc],
    rows: &[GridRow],
) -> fmt::Result {
    let mut idx = 0;
    // A gap is valid only immediately after a content row
    while idx < rows.len() {
        match &rows[idx] {
            GridRow::Cells(docs) => render_cells(output, docs)?,
            GridRow::Spanning(doc) => {
                render_grid_spanning_row(output, alignments, docs_column_head, doc)?
            }
            GridRow::Gap => return Err(fmt::Error),
        }
        idx += 1;
        // Consume one gap with its preceding row; consecutive gaps stay invalid
        if matches!(rows.get(idx), Some(GridRow::Gap)) {
            output.write_str(" \\\\[1ex]\n")?;
            idx += 1;
        } else if idx < rows.len() {
            output.write_str(" \\\\\n")?;
        }
    }
    Ok(())
}

/// Writes a grid array using first-column widths from its ordinary rows.
fn render_grid_array(
    output: &mut dyn Write,
    alignments: &[Alignment],
    rows_cell: &[&[Doc]],
    rows: &[GridRow],
) -> fmt::Result {
    let docs_column_head: Vec<_> = rows_cell
        .iter()
        .filter_map(|docs| docs.first())
        .map(link::strip_links)
        .collect();
    output.write_str(r"\begin{array}{")?;
    render_alignments(output, alignments)?;
    output.write_str("}\n")?;
    render_grid_rows(output, alignments, &docs_column_head, rows)?;
    output.write_str("\n\\end{array}")
}

// - Mixed-grid width reservation

/// Gives mixed grids the maximum of their cell and spanning widths.
fn render_grid(output: &mut dyn Write, alignments: &[Alignment], rows: &[GridRow]) -> fmt::Result {
    let rows_cell: Vec<_> = rows
        .iter()
        .filter_map(|row| match row {
            GridRow::Cells(docs) => Some(docs.as_slice()),
            _ => None,
        })
        .collect();
    let docs_spanning: Vec<_> = rows
        .iter()
        .filter_map(|row| match row {
            GridRow::Spanning(doc) => Some(doc),
            _ => None,
        })
        .collect();
    // Spanning-only grids use one left-aligned column
    if rows_cell.is_empty() {
        return render_grid_array(output, &[Alignment::Left], &[], rows);
    }
    if docs_spanning.is_empty() {
        return render_grid_array(output, alignments, &rows_cell, rows);
    }
    // Emit visible content once, then reserve its complete geometry
    output.write_str(r"\mathrlap{\displaystyle ")?;
    render_grid_array(output, alignments, &rows_cell, rows)?;
    output.write_str("}")?;
    render_grid_phantom(output, alignments, &rows_cell, &docs_spanning)
}

/// Reserves the maximum width of link-free cell and spanning documents.
fn render_grid_phantom(
    output: &mut dyn Write,
    alignments: &[Alignment],
    rows_cell: &[&[Doc]],
    docs_spanning: &[&Doc],
) -> fmt::Result {
    output.write_str("\\smash{\\hphantom{\\begin{array}{l}\n")?;
    // Remove links from the invisible copy of the ordinary rows
    let rows_cell: Vec<Vec<_>> = rows_cell
        .iter()
        .map(|docs| docs.iter().map(link::strip_links).collect())
        .collect();
    render_array(output, alignments, rows_cell.iter().map(Vec::as_slice))?;
    // Spanning documents can be wider than the shared cell columns
    for doc in docs_spanning {
        output.write_str(" \\\\\n")?;
        let doc = link::strip_links(doc);
        render_doc(output, &doc)?;
    }
    output.write_str("\n\\end{array}}}")
}

// == Numbered premises

/// Writes an optional premise label followed by its body cell.
fn render_numbered_row(output: &mut dyn Write, num: Option<usize>, doc: &Doc) -> fmt::Result {
    if let Some(num) = num {
        write!(output, "{{\\scriptstyle\\mathtt{{({num})}}}}")?;
    }
    output.write_str(" & ")?;
    render_doc(output, doc)
}

/// Prints each premise number once, leaving continuation labels empty.
fn render_numbered(output: &mut dyn Write, docs: &[Doc]) -> fmt::Result {
    output.write_str("\\begin{array}{r@{\\quad}l}\n")?;
    // Empty left stacks still consume a number in the raw document model
    for (idx, doc) in docs.iter().enumerate() {
        match doc {
            // Expand a multiline premise into one numbered and several plain rows
            Doc::LeftStack(lines) => {
                let Some((line_head, lines)) = lines.split_first() else {
                    continue;
                };
                render_numbered_row(output, Some(idx + 1), line_head)?;
                for line in lines {
                    output.write_str(" \\\\\n")?;
                    render_numbered_row(output, None, line)?;
                }
            }
            // Ordinary premises occupy one numbered row
            doc => render_numbered_row(output, Some(idx + 1), doc)?,
        }
        if idx + 1 < docs.len() {
            output.write_str(" \\\\\n")?;
        }
    }
    output.write_str("\n\\end{array}")
}

// == Gathered blocks

/// Writes gathered blocks with ordinary or enlarged row separation.
fn render_gathered(output: &mut dyn Write, blocks: &[Block]) -> fmt::Result {
    output.write_str("\\begin{gathered}\n")?;
    let mut idx = 0;
    // Consume gaps together with the preceding content line
    while idx < blocks.len() {
        let Block::Line(doc) = &blocks[idx] else {
            return Err(fmt::Error);
        };
        render_doc(output, doc)?;
        idx += 1;
        // A following gap replaces the ordinary row separator
        if matches!(blocks.get(idx), Some(Block::Gap)) {
            output.write_str(" \\\\[1ex]\n")?;
            idx += 1;
        } else if idx < blocks.len() {
            output.write_str(" \\\\\n")?;
        }
    }
    output.write_str("\n\\end{gathered}")
}

// == Structural validation

/// Rejects row sequences that cannot be serialized as content and separators.
fn validate(doc: &Doc) -> Result<()> {
    match doc {
        Doc::Concat(docs) | Doc::Stacked(docs) | Doc::LeftStack(docs) | Doc::Numbered(docs) => {
            validate_docs(docs)?;
        }
        Doc::Group(doc)
        | Doc::Mathbin(doc)
        | Doc::Mathrel(doc)
        | Doc::Displaystyle(doc)
        | Doc::Delimited(_, doc)
        | Doc::Link(_, doc)
        | Doc::LayoutGroup(doc)
        | Doc::Nest(_, doc) => validate(doc)?,
        Doc::Subscript(doc_l, doc_r)
        | Doc::Superscript(doc_l, doc_r)
        | Doc::Fraction(doc_l, doc_r) => {
            validate(doc_l)?;
            validate(doc_r)?;
        }
        Doc::Subsup(doc_base, doc_sub, doc_sup) => {
            validate(doc_base)?;
            validate(doc_sub)?;
            validate(doc_sup)?;
        }
        Doc::Fill(_, separator, docs) => validate_docs(interspersed(separator, docs))?,
        Doc::Aligned(rows) => validate_docs(rows.iter().flatten())?,
        Doc::Grid(_, rows) => validate_grid_rows(rows)?,
        Doc::Gathered(blocks) => validate_blocks(blocks)?,
        _ => {}
    }
    Ok(())
}

/// Validates documents in serialization order, stopping at the first error.
fn validate_docs<'a>(docs: impl IntoIterator<Item = &'a Doc>) -> Result<()> {
    for doc in docs {
        validate(doc)?;
    }
    Ok(())
}

/// Rejects leading or consecutive grid gaps and validates row content.
fn validate_grid_rows(rows: &[GridRow]) -> Result<()> {
    let mut gap_allowed = false;
    // A content row permits at most one subsequent gap
    for row in rows {
        match row {
            // Validate cells before accepting a separator after their row
            GridRow::Cells(docs) => {
                validate_docs(docs)?;
                gap_allowed = true;
            }
            // Spanning content follows the same gap rule as ordinary cells
            GridRow::Spanning(doc) => {
                validate(doc)?;
                gap_allowed = true;
            }
            // Leading and consecutive gaps have no preceding content row
            GridRow::Gap => {
                if !gap_allowed {
                    return Err(Error::MalformedGridGap);
                }
                gap_allowed = false;
            }
        }
    }
    Ok(())
}

/// Rejects leading or consecutive gathered gaps and validates each line.
fn validate_blocks(blocks: &[Block]) -> Result<()> {
    let mut gap_allowed = false;
    // A line permits at most one subsequent gap
    for block in blocks {
        match block {
            // Validate the line before accepting its following separator
            Block::Line(doc) => {
                validate(doc)?;
                gap_allowed = true;
            }
            // Leading and consecutive gaps have no preceding line
            Block::Gap => {
                if !gap_allowed {
                    return Err(Error::MalformedGathered);
                }
                gap_allowed = false;
            }
        }
    }
    Ok(())
}
