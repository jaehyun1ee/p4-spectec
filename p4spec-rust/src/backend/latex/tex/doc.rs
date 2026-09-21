//! Mathematical document structure and normalization
//!
//! `concat`, `grid`, and `gathered` normalize structural composition;
//! layout and serialization interpret the retained mathematical intent.

use crate::backend::latex::error::{Error, Result};
use num_bigint::BigInt;

// == Document model

/// Selects a font and its escaping context.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Style {
    #[allow(dead_code)]
    Mathit,
    Mathrm,
    Mathsf,
    Mathbb,
    Mathtt,
    Text,
    Texttt,
}

/// Selects a balanced delimiter pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Delimiter {
    Paren,
    Bracket,
    Brace,
    Angle,
    Bar,
}

/// Aligns one grid column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Alignment {
    Left,
    Center,
    Right,
}

/// Specifies the flat representation of a break opportunity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Soft {
    #[allow(dead_code)]
    SoftCut,
    SoftSpace,
}

/// Names a fixed mathematical atom.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Symbol {
    Equal,
    NotEqual,
    Less,
    Greater,
    LessEqual,
    GreaterEqual,
    Plus,
    Minus,
    Question,
    Ast,
    Slash,
    Comma,
    Semicolon,
    Colon,
    DoubleColon,
    Cat,
    Production,
    VerticalBar,
    Dot,
    Dot2,
    Ellipsis,
    Epsilon,
    In,
    Neg,
    Land,
    Lor,
    Rightarrow,
    Leftrightarrow,
    Cdot,
    Bmod,
    Turnstile,
    Tilesturn,
    To,
    Longrightarrow,
    Hookrightarrow,
    Mapsto,
    Sim,
    Setminus,
    EmptySet,
    LeftParen,
    RightParen,
    LeftBracket,
    RightBracket,
    LeftBrace,
    RightBrace,
}

/// Identifies a local HTML anchor validated by `target_of_string`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Target(pub(super) String);

/// Retains mathematical and layout intent until interpretation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Doc {
    Empty,
    Styled(Style, String),
    Badge(String),
    Decimal(BigInt),
    Hexadecimal(BigInt),
    Fixed(Symbol),
    Space,
    ThinSpace,
    Quad,
    Concat(Vec<Doc>),
    Group(Box<Doc>),
    Mathbin(Box<Doc>),
    Mathrel(Box<Doc>),
    Displaystyle(Box<Doc>),
    Delimited(Delimiter, Box<Doc>),
    Subscript(Box<Doc>, Box<Doc>),
    Superscript(Box<Doc>, Box<Doc>),
    #[allow(dead_code)]
    Subsup(Box<Doc>, Box<Doc>, Box<Doc>),
    Fraction(Box<Doc>, Box<Doc>),
    Link(Target, Box<Doc>),
    SoftBreak(Soft),
    LayoutGroup(Box<Doc>),
    Nest(usize, Box<Doc>),
    Fill(usize, Box<Doc>, Vec<Doc>),
    Aligned(Vec<Vec<Doc>>),
    Grid(Vec<Alignment>, Vec<Row>),
    #[allow(dead_code)]
    Stacked(Vec<Doc>),
    LeftStack(Vec<Doc>),
    Numbered(Vec<Doc>),
    Gathered(Vec<Block>),
}

/// Supplies cells, a spanning document, or a vertical grid gap.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Row {
    Cells(Vec<Doc>),
    Spanning(Doc),
    Gap,
}

/// Supplies a gathered line or a vertical gap.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Block {
    Line(Doc),
    Gap,
}

// == Emptiness

/// Tests semantic emptiness without discarding explicit TeX groups.
pub(crate) fn is_empty(doc: &Doc) -> bool {
    match doc {
        Doc::Empty => true,
        Doc::Concat(docs)
        | Doc::Stacked(docs)
        | Doc::LeftStack(docs)
        | Doc::Numbered(docs)
        | Doc::Fill(_, _, docs) => docs.iter().all(is_empty),
        Doc::Displaystyle(doc) | Doc::LayoutGroup(doc) | Doc::Nest(_, doc) => is_empty(doc),
        _ => false,
    }
}

// == Normalizing composition

/// Flattens concatenation and removes empty atomic documents.
pub(crate) fn concat(docs: Vec<Doc>) -> Doc {
    // Expand nested sequences in their original order
    let mut docs_flat = Vec::new();
    let mut docs_pending = docs;
    docs_pending.reverse();
    while let Some(doc) = docs_pending.pop() {
        match doc {
            // Discard only atomic emptiness
            Doc::Empty => {}
            // Put nested children before the remaining siblings
            Doc::Concat(docs) => docs_pending.extend(docs.into_iter().rev()),
            // Preserve wrappers even when their content is empty
            doc => docs_flat.push(doc),
        }
    }
    match docs_flat.len() {
        0 => Doc::Empty,
        1 => docs_flat.pop().unwrap(),
        _ => Doc::Concat(docs_flat),
    }
}

/// Inserts separators only between semantically nonempty documents.
pub(crate) fn concat_intersperse(separator: Doc, docs: Vec<Doc>) -> Doc {
    let mut docs_separated = Vec::new();
    // Skip emptiness before deciding whether a separator is needed
    for doc in docs.into_iter().filter(|doc| !is_empty(doc)) {
        if !docs_separated.is_empty() {
            docs_separated.push(separator.clone());
        }
        docs_separated.push(doc);
    }
    concat(docs_separated)
}

/// Separates nonempty documents by spaces.
pub(crate) fn concat_spaced(docs: Vec<Doc>) -> Doc {
    concat_intersperse(Doc::Space, docs)
}

/// Separates nonempty documents by commas and spaces.
pub(crate) fn concat_comma_separated(docs: Vec<Doc>) -> Doc {
    let separator = concat(vec![Doc::Fixed(Symbol::Comma), Doc::Space]);
    concat_intersperse(separator, docs)
}

/// Separates nonempty documents by thin spaces.
pub(crate) fn concat_juxtaposed(docs: Vec<Doc>) -> Doc {
    concat_intersperse(Doc::ThinSpace, docs)
}

/// Groups a comma-separated list with breakable spaces.
pub(crate) fn layout_group_soft_comma_separated(docs: Vec<Doc>) -> Doc {
    let separator = concat(vec![Doc::Fixed(Symbol::Comma), Doc::SoftBreak(Soft::SoftSpace)]);
    let doc = concat_intersperse(separator, docs);
    layout_group(doc)
}

/// Omits a badge whose label is empty.
pub(crate) fn badge(text: String) -> Doc {
    if text.is_empty() { Doc::Empty } else { Doc::Badge(text) }
}

/// Omits display style around an empty document.
pub(crate) fn displaystyle(doc: Doc) -> Doc {
    if is_empty(&doc) { Doc::Empty } else { Doc::Displaystyle(Box::new(doc)) }
}

/// Omits a link around an empty document.
pub(crate) fn link(target: Target, doc: Doc) -> Doc {
    if is_empty(&doc) { Doc::Empty } else { Doc::Link(target, Box::new(doc)) }
}

/// Omits a layout group around an empty document.
pub(crate) fn layout_group(doc: Doc) -> Doc {
    if is_empty(&doc) { Doc::Empty } else { Doc::LayoutGroup(Box::new(doc)) }
}

/// Omits ineffective continuation indentation.
pub(crate) fn nest(indent: usize, doc: Doc) -> Doc {
    if indent == 0 || is_empty(&doc) { doc } else { Doc::Nest(indent, Box::new(doc)) }
}

/// Retains a fill only when multiple nonempty documents need packing.
pub(crate) fn fill(indent: usize, separator: Doc, docs: Vec<Doc>) -> Doc {
    let mut docs: Vec<_> = docs.into_iter().filter(|doc| !is_empty(doc)).collect();
    match docs.len() {
        0 => Doc::Empty,
        1 => docs.pop().unwrap(),
        _ => Doc::Fill(indent, Box::new(separator), docs),
    }
}

// == Multi-row composition

/// Validates grid arity and removes empty content rows.
pub(crate) fn grid(alignments: Vec<Alignment>, rows: Vec<Row>) -> Result<Doc> {
    // A nonempty row sequence needs a column specification
    if alignments.is_empty() {
        return if rows.is_empty() { Ok(Doc::Empty) } else { Err(Error::GridWithoutColumns) };
    }
    // Validate before filtering so malformed empty rows remain errors
    for row in &rows {
        if let Row::Cells(docs) = row
            && docs.len() != alignments.len()
        {
            return Err(Error::GridCellCount { expected: alignments.len(), actual: docs.len() });
        }
    }
    let rows: Vec<_> = rows
        .into_iter()
        .filter(|row| match row {
            Row::Cells(docs) => !docs.iter().all(is_empty),
            Row::Spanning(doc) => !is_empty(doc),
            Row::Gap => true,
        })
        .collect();
    if rows.is_empty() { Ok(Doc::Empty) } else { Ok(Doc::Grid(alignments, rows)) }
}

/// Removes empty documents from a centered stack.
#[allow(dead_code)]
pub(crate) fn stacked(docs: Vec<Doc>) -> Doc {
    let docs: Vec<_> = docs.into_iter().filter(|doc| !is_empty(doc)).collect();
    if docs.is_empty() { Doc::Empty } else { Doc::Stacked(docs) }
}

/// Collapses a left stack with at most one nonempty document.
pub(crate) fn left_stack(docs: Vec<Doc>) -> Doc {
    let mut docs: Vec<_> = docs.into_iter().filter(|doc| !is_empty(doc)).collect();
    match docs.len() {
        0 => Doc::Empty,
        1 => docs.pop().unwrap(),
        _ => Doc::LeftStack(docs),
    }
}

/// Numbers only nonempty premise documents.
pub(crate) fn numbered(docs: Vec<Doc>) -> Doc {
    let docs: Vec<_> = docs.into_iter().filter(|doc| !is_empty(doc)).collect();
    if docs.is_empty() { Doc::Empty } else { Doc::Numbered(docs) }
}

/// Removes outer gaps and coalesces gaps between nonempty lines.
pub(crate) fn gathered(blocks: Vec<Block>) -> Doc {
    let mut blocks_normalized = Vec::new();
    let mut gap_pending = false;
    // Defer gaps until a subsequent nonempty line uses them
    for block in blocks {
        match block {
            // Leading gaps have no preceding line
            Block::Gap => gap_pending = !blocks_normalized.is_empty(),
            // Empty lines preserve a pending interior gap
            Block::Line(doc) if is_empty(&doc) => {}
            // Emit at most one gap before this line
            Block::Line(doc) => {
                if gap_pending {
                    blocks_normalized.push(Block::Gap);
                }
                blocks_normalized.push(Block::Line(doc));
                gap_pending = false;
            }
        }
    }
    Doc::Gathered(blocks_normalized)
}
