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
    /// Italic math text, `\mathit{...}`.
    #[allow(dead_code)]
    Mathit,
    /// Upright math text, `\mathrm{...}`.
    Mathrm,
    /// Sans-serif math text, `\mathsf{...}`.
    Mathsf,
    /// Blackboard-bold math text, `\mathbb{...}`.
    Mathbb,
    /// Monospace math text, `\mathtt{...}`.
    Mathtt,
    /// Text-mode content, `\text{...}`, with math-only glyphs split out.
    Text,
    /// Monospace text, `\texttt{...}`, with math-only glyphs split out.
    Texttt,
}

/// Selects a balanced delimiter pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Delimiter {
    /// Parentheses, `\left(...\right)`.
    Paren,
    /// Square brackets, `\left[...\right]`.
    Bracket,
    /// Braces, `\left\{...\right\}`.
    Brace,
    /// Angle brackets, `\left\langle...\right\rangle`.
    Angle,
    /// Vertical bars, `\left|...\right|`.
    Bar,
}

/// Aligns one grid column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Alignment {
    /// Left-aligns the column.
    Left,
    /// Centers the column.
    Center,
    /// Right-aligns the column.
    Right,
}

/// Specifies the flat representation of a break opportunity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Soft {
    /// Emits nothing in flat mode and starts a new line in broken mode.
    #[allow(dead_code)]
    SoftCut,
    /// Emits one space in flat mode and starts a new line in broken mode.
    SoftSpace,
}

/// Names a fixed mathematical atom.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Symbol {
    /// Equality, `=`.
    Equal,
    /// Inequality, `\ne`.
    NotEqual,
    /// Less-than relation, `<`.
    Less,
    /// Greater-than relation, `>`.
    Greater,
    /// Less-than-or-equal relation, `\le`.
    LessEqual,
    /// Greater-than-or-equal relation, `\ge`.
    GreaterEqual,
    /// Addition or positive sign, `+`.
    Plus,
    /// Subtraction or negative sign, `-`.
    Minus,
    /// Optional iteration marker, `?`.
    Question,
    /// List iteration marker, `\ast`.
    Ast,
    /// Division slash, `/`.
    Slash,
    /// Comma separator, `,`.
    Comma,
    /// Semicolon separator, `;`.
    Semicolon,
    /// Colon separator, `:`.
    Colon,
    /// List construction, `::`.
    DoubleColon,
    /// Concatenation, `+\!\!+`.
    Cat,
    /// Grammar production, `::=`.
    Production,
    /// Grammar alternative separator, `|`.
    VerticalBar,
    /// Single dot, `.`.
    Dot,
    /// Two literal dots, `..`.
    Dot2,
    /// Ellipsis, `\ldots`.
    Ellipsis,
    /// Empty sequence, `\epsilon`.
    Epsilon,
    /// Membership, `\in`.
    In,
    /// Logical negation, `\neg`.
    Neg,
    /// Conjunction, `\land`.
    Land,
    /// Disjunction, `\lor`.
    Lor,
    /// Implication arrow, `\Rightarrow`.
    Rightarrow,
    /// Equivalence arrow, `\Leftrightarrow`.
    Leftrightarrow,
    /// Multiplication dot, `\cdot`.
    Cdot,
    /// Numeric remainder, `\bmod`.
    Bmod,
    /// Right-facing turnstile, `\vdash`.
    Turnstile,
    /// Left-facing turnstile, `\dashv`.
    Tilesturn,
    /// Single arrow, `\to`.
    To,
    /// Long double arrow, `\Longrightarrow`.
    Longrightarrow,
    /// Hooked arrow, `\hookrightarrow`.
    Hookrightarrow,
    /// Table mapping arrow, `\mapsto`.
    Mapsto,
    /// Similarity relation, `\sim`.
    Sim,
    /// Set difference, `\setminus`.
    Setminus,
    /// Empty set, `\varnothing`.
    EmptySet,
    /// Literal opening parenthesis, without automatic sizing.
    LeftParen,
    /// Literal closing parenthesis, without automatic sizing.
    RightParen,
    /// Literal opening square bracket, without automatic sizing.
    LeftBracket,
    /// Literal closing square bracket, without automatic sizing.
    RightBracket,
    /// Literal opening brace, without automatic sizing.
    LeftBrace,
    /// Literal closing brace, without automatic sizing.
    RightBrace,
}

/// Identifies a local HTML anchor validated by `target_of_string`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Target(pub(super) String);

/// Retains mathematical and layout intent until interpretation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Doc {
    /// No content and no width.
    Empty,
    /// Text escaped in the selected font and math or text context.
    Styled(Style, String),
    /// A boxed, shaded rule label containing small monospace text.
    Badge(String),
    /// A decimal integer in math mode.
    Decimal(BigInt),
    /// A hexadecimal integer with a `0x` prefix inside `\mathtt{...}`.
    Hexadecimal(BigInt),
    /// A mathematical atom with a predefined TeX spelling.
    Fixed(Symbol),
    /// A literal space, measured as one column.
    Space,
    /// A thin math space, `\,`, measured as one column.
    ThinSpace,
    /// A wide math space, `\quad`, measured as two columns.
    Quad,
    /// Documents concatenated without implicit spacing.
    Concat(Vec<Doc>),
    /// An explicit TeX group, `{...}`, retained even when empty.
    Group(Box<Doc>),
    /// Content classified as a binary operator, `\mathbin{...}`.
    Mathbin(Box<Doc>),
    /// Content classified as a relation, `\mathrel{...}`.
    Mathrel(Box<Doc>),
    /// Content forced to display-style math, `{\displaystyle ...}`.
    Displaystyle(Box<Doc>),
    /// A delimiter pair sized to its enclosed document.
    Delimited(Delimiter, Box<Doc>),
    /// A base and subscript, in that order: `{base}_{sub}`.
    Subscript(Box<Doc>, Box<Doc>),
    /// A base and superscript, in that order: `{base}^{sup}`.
    Superscript(Box<Doc>, Box<Doc>),
    /// A base, subscript, and superscript: `{base}_{sub}^{sup}`.
    #[allow(dead_code)]
    Subsup(Box<Doc>, Box<Doc>, Box<Doc>),
    /// A numerator and denominator: `\frac{num}{den}`.
    Fraction(Box<Doc>, Box<Doc>),
    /// A local anchor and its visible document: `\href{#target}{doc}`.
    Link(Target, Box<Doc>),
    /// A break opportunity whose flat spelling is selected by `Soft`.
    SoftBreak(Soft),
    /// Content whose soft breaks are selected together by available width.
    /// Nested layout groups choose their own mode; no TeX braces are added.
    LayoutGroup(Box<Doc>),
    /// Additional continuation indentation and its document, in that order.
    /// The first line keeps its current column.
    Nest(usize, Box<Doc>),
    /// Continuation indentation, separator, and greedily packed documents.
    /// A line break replaces the separator; `fill` removes empty items.
    Fill(usize, Box<Doc>, Vec<Doc>),
    /// Equation rows in `aligned`, with shared column widths.
    Aligned(Vec<Vec<Doc>>),
    /// Explicit column alignments and cell, spanning, or gap rows.
    Grid(Vec<Alignment>, Vec<GridRow>),
    /// Rows in `aligned`, each following an empty alignment cell.
    #[allow(dead_code)]
    Stacked(Vec<Doc>),
    /// Left-aligned rows, also used for resolved line breaks.
    LeftStack(Vec<Doc>),
    /// Premises with numeric labels; continuation rows have no label.
    Numbered(Vec<Doc>),
    /// Centered blocks with ordinary or enlarged vertical separation.
    Gathered(Vec<Block>),
}

/// Supplies cells, a spanning document, or a vertical grid gap.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum GridRow {
    /// One document per declared grid column.
    Cells(Vec<Doc>),
    /// Content spanning the grid, laid out with the full line-width budget.
    Spanning(Doc),
    /// Extra vertical space after a content row; never first or consecutive.
    Gap,
}

/// Supplies a gathered line or a vertical gap.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Block {
    /// One centered document in a gathered environment.
    Line(Doc),
    /// Extra space between lines; `gathered` trims and coalesces gaps.
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

/// Borrows nonempty documents with a separator between adjacent documents.
pub(super) fn interspersed<'a>(
    separator: &'a Doc,
    docs: &'a [Doc],
) -> impl Iterator<Item = &'a Doc> {
    // Filter emptiness before inserting borrowed separators
    docs.iter()
        .filter(|doc| !is_empty(doc))
        .enumerate()
        .flat_map(move |(idx, doc)| {
            let separator = (idx != 0).then_some(separator);
            separator.into_iter().chain(std::iter::once(doc))
        })
}

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
pub(crate) fn grid(alignments: Vec<Alignment>, rows: Vec<GridRow>) -> Result<Doc> {
    // A nonempty row sequence needs a column specification
    if alignments.is_empty() {
        return if rows.is_empty() { Ok(Doc::Empty) } else { Err(Error::GridWithoutColumns) };
    }
    // Validate before filtering so malformed empty rows remain errors
    for row in &rows {
        if let GridRow::Cells(docs) = row
            && docs.len() != alignments.len()
        {
            return Err(Error::GridCellCount { expected: alignments.len(), actual: docs.len() });
        }
    }
    let rows: Vec<_> = rows
        .into_iter()
        .filter(|row| match row {
            GridRow::Cells(docs) => !docs.iter().all(is_empty),
            GridRow::Spanning(doc) => !is_empty(doc),
            GridRow::Gap => true,
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
