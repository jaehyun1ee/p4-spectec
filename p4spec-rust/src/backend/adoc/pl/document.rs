//! AsciiDoc prose, code spans, and document blocks
//!
//! Renderers build blocks before serialization so fallthrough links can use
//! the displayed list marker of their target arm.
//! `ser_block` resolves those markers and suppresses nested cross-references;
//! anchor callbacks resolve function and relation references in fragments.

use std::collections::{BTreeMap, BTreeSet};

use super::utils::*;

// == Documents

/// Inline prose and embedded code.
#[derive(Clone, Debug, PartialEq)]
pub enum Prose {
    Text(String),
    Code(Code),
    Link(Link, Box<Prose>),
    Fallthrough(String, FallthroughLabel),
    Seq(Vec<Prose>),
    Empty,
}

/// Code tokens with optional cross-references.
#[derive(Clone, Debug, PartialEq)]
pub enum Code {
    Token(String),
    Link(Link, Box<Code>),
    Seq(Vec<Code>),
    Empty,
}

/// A concrete target or a reference resolved by the enclosing document.
#[derive(Clone, Debug, PartialEq)]
pub enum Link {
    Direct(String),
    Subject(Subject),
}

/// A definition referenced by prose.
#[derive(Clone, Debug, PartialEq)]
pub enum Subject {
    Function(String),
    Relation(String),
}

/// A fallthrough marker inferred from its target or supplied by a group.
#[derive(Clone, Debug, PartialEq)]
pub enum FallthroughLabel {
    Derived,
    Explicit(String),
}

/// A list entry, optionally defining an ordered arm anchor.
#[derive(Clone, Debug, PartialEq)]
pub enum ItemKind {
    Ordered(Option<String>),
    Unordered,
}

/// A document fragment with explicit list nesting and table boundaries.
#[derive(Clone, Debug, PartialEq)]
pub enum Block {
    Empty,
    Raw(String),
    Inline(Prose),
    Item(usize, ItemKind, Prose, Box<Block>),
    Concat(Vec<Block>),
    Seq(Vec<Block>),
    Table(usize, Vec<Prose>, Vec<Vec<String>>),
}

// == Capitalization

/// Whether capitalization found text, can continue, or reached protected prose.
enum CapStep {
    Done,
    Skip,
    Stop,
}

/// Capitalizes text until a code span or link stops the search.
fn capitalize_prose(prose: &mut Prose) -> CapStep {
    match prose {
        Prose::Text(text) => {
            // Capitalize only an initial ASCII letter
            if text.starts_with(|character: char| character.is_ascii_alphabetic()) {
                text[..1].make_ascii_uppercase();
                CapStep::Done
            } else {
                CapStep::Skip
            }
        }
        Prose::Code(_) | Prose::Link(..) | Prose::Fallthrough(..) => CapStep::Stop,
        Prose::Seq(proses) => {
            // Empty or punctuation-only pieces leave the next piece eligible
            for prose in proses {
                match capitalize_prose(prose) {
                    CapStep::Skip => {}
                    step => return step,
                }
            }
            CapStep::Skip
        }
        Prose::Empty => CapStep::Skip,
    }
}

/// Capitalizes the first eligible text without changing embedded code or links.
pub fn capitalize_first_prose(mut prose: Prose) -> Prose {
    capitalize_prose(&mut prose);
    prose
}

/// Visits block headings in order until capitalization reaches protected text.
fn capitalize_block(block: &mut Block) -> bool {
    match block {
        Block::Empty | Block::Raw(_) => false,
        Block::Inline(prose) => !matches!(capitalize_prose(prose), CapStep::Skip),
        Block::Item(_, _, prose, block_body) => {
            matches!(capitalize_prose(prose), CapStep::Done | CapStep::Stop)
                || capitalize_block(block_body)
        }
        Block::Concat(blocks) | Block::Seq(blocks) => blocks.iter_mut().any(capitalize_block),
        Block::Table(..) => true,
    }
}

/// Capitalizes the first eligible block heading or inline text.
pub fn capitalize_first_block(mut block: Block) -> Block {
    capitalize_block(&mut block);
    block
}

// == Ordered-list markers

/// Derives the label displayed by AsciiDoc's five-level list style cycle.
fn ordered_marker(level: usize, idx: usize) -> String {
    let num = idx + 1;
    match level % 5 {
        // Decimal lists restart the style cycle
        0 => num.to_string(),
        // Alphabetic lists use an explicit fallback beyond the alphabet
        1 | 3 => {
            if idx < 26 {
                let character = if level % 5 == 1 { b'a' } else { b'A' };
                char::from(character + idx as u8).to_string()
            } else {
                format!("arm{num}")
            }
        }
        // Roman labels follow the source renderer's two-digit conversion
        _ => {
            let units = ["", "i", "ii", "iii", "iv", "v", "vi", "vii", "viii", "ix"];
            let tens = ["", "x", "xx", "xxx", "xl", "l", "lx", "lxx", "lxxx", "xc"];
            let text = format!("{}{}", tens[num / 10 % 10], units[num % 10]);
            if level % 5 == 4 { text.to_ascii_uppercase() } else { text }
        }
    }
}

/// Records arm labels in document order, resetting lists at shallower entries.
fn collect_markers(
    block: &Block,
    markers: &mut BTreeMap<String, String>,
    ordinals: &mut BTreeMap<usize, usize>,
) {
    match block {
        Block::Item(level, kind, _, block_body) => {
            // A shallower entry starts new nested lists
            ordinals.retain(|level_inner, _| level_inner <= level);
            match kind {
                ItemKind::Unordered => {
                    // End the ordered list at this level
                    ordinals.remove(level);
                }
                ItemKind::Ordered(anchor_opt) => {
                    // Anchored and unanchored entries both advance the ordinal
                    let ordinal = ordinals.entry(*level).or_default();
                    if let Some(anchor) = anchor_opt {
                        markers.insert(anchor.clone(), ordered_marker(*level, *ordinal));
                    }
                    *ordinal += 1;
                }
            }
            collect_markers(block_body, markers, ordinals);
        }
        Block::Concat(blocks) | Block::Seq(blocks) => {
            // Concatenation preserves the enclosing list's position
            for block in blocks {
                collect_markers(block, markers, ordinals);
            }
        }
        Block::Empty | Block::Raw(_) | Block::Inline(_) | Block::Table(..) => {}
    }
}

// == Anchor resolution

/// Resolves a subject to its unqualified definition name.
pub fn subject_name(subject: &Subject) -> Option<String> {
    match subject {
        Subject::Function(id) | Subject::Relation(id) => Some(id.clone()),
    }
}

fn target_of_link(link: &Link, anchor: &dyn Fn(&Subject) -> Option<String>) -> Option<String> {
    match link {
        Link::Direct(target) => Some(target.clone()),
        Link::Subject(subject) => anchor(subject),
    }
}

// == Serialization

/// A run of code tokens sharing one link target.
struct CodeSegment {
    target: Option<String>,
    text: String,
}

fn is_empty_code(code: &Code) -> bool {
    match code {
        Code::Token(text) => text.is_empty(),
        Code::Link(_, code) => is_empty_code(code),
        Code::Seq(codes) => codes.iter().all(is_empty_code),
        Code::Empty => true,
    }
}

/// Per-serialization anchor labels and warnings.
struct Serializer<'a> {
    anchor: &'a dyn Fn(&Subject) -> Option<String>,
    markers: BTreeMap<String, String>,
    warned: BTreeSet<String>,
}

impl Serializer<'_> {
    fn warn(&mut self, text: String) {
        if self.warned.insert(text.clone()) {
            eprintln!("Warning: prose: {text}");
        }
    }

    /// Flattens code spans and coalesces adjacent tokens with the same target.
    fn collect_code(
        &mut self,
        code: &Code,
        target: Option<&str>,
        link_ctx: Option<&str>,
        lint: bool,
        segments: &mut Vec<CodeSegment>,
    ) {
        match code {
            Code::Token(text) => {
                // Empty tokens cannot split an existing span
                if text.is_empty() {
                    return;
                }
                if let Some(segment) = segments
                    .last_mut()
                    .filter(|segment| segment.target.as_deref() == target)
                {
                    segment.text.push_str(text);
                } else {
                    segments.push(CodeSegment {
                        target: target.map(str::to_owned),
                        text: text.clone(),
                    });
                }
            }
            Code::Link(link, code_inner) => {
                // Unresolved subjects retain the surrounding link context
                let Some(target_inner) = target_of_link(link, self.anchor) else {
                    self.collect_code(code_inner, target, link_ctx, lint, segments);
                    return;
                };
                if lint && target_inner.is_empty() {
                    self.warn("link with empty target".into());
                }
                if let Some(target_outer) = link_ctx {
                    // Cross-references cannot nest in AsciiDoc
                    if lint {
                        self.warn(format!("nested link: cross-reference to {target_inner:?} is dropped inside the link to {target_outer:?} (asciidoc cannot nest cross-references)"));
                    }
                    self.collect_code(code_inner, target, link_ctx, lint, segments);
                } else {
                    // The outermost resolved link owns its entire code span
                    if lint && is_empty_code(code_inner) {
                        self.warn(format!("link to {target_inner:?} has empty body"));
                    }
                    self.collect_code(
                        code_inner,
                        Some(&target_inner),
                        Some(&target_inner),
                        lint,
                        segments,
                    );
                }
            }
            Code::Seq(codes) => {
                // Adjacent sequences participate in the same span coalescing
                for code in codes {
                    self.collect_code(code, target, link_ctx, lint, segments);
                }
            }
            Code::Empty => {}
        }
    }

    /// Serializes coalesced code, applying monospace only for inline prose.
    fn code(&mut self, code: &Code, link_ctx: Option<&str>, lint: bool, mono: bool) -> String {
        let mut segments = Vec::new();
        self.collect_code(code, None, link_ctx, lint, &mut segments);
        // Formatting after coalescing keeps adjacent tokens in one code span
        segments
            .into_iter()
            .map(|segment| {
                let text = if mono { adoc_mono_chopped(&segment.text) } else { segment.text };
                match segment.target {
                    Some(target) => adoc_link(&target, &text),
                    None => text,
                }
            })
            .collect()
    }

    /// Serializes inline prose with the enclosing cross-reference context.
    fn prose(&mut self, prose: &Prose, link_ctx: Option<&str>, lint: bool) -> String {
        match prose {
            Prose::Text(text) => text.clone(),
            Prose::Code(code) => self.code(code, link_ctx, lint, true),
            Prose::Link(link, prose_inner) => {
                // Preserve the body when the enclosing document has no target
                let Some(target) = target_of_link(link, self.anchor) else {
                    return self.prose(prose_inner, link_ctx, lint);
                };
                if lint && target.is_empty() {
                    self.warn("link with empty target".into());
                }
                if let Some(target_outer) = link_ctx {
                    // An outer link takes precedence over nested links
                    if lint {
                        self.warn(format!("nested link: cross-reference to {target:?} is dropped inside the link to {target_outer:?} (asciidoc cannot nest cross-references)"));
                    }
                    self.prose(prose_inner, link_ctx, lint)
                } else {
                    // Format the complete body before choosing link delimiters
                    let text = self.prose(prose_inner, Some(&target), lint);
                    if lint && text.is_empty() {
                        self.warn(format!("link to {target:?} has empty body"));
                    }
                    adoc_link(&target, &text)
                }
            }
            Prose::Fallthrough(target, label) => {
                // Derived labels refer to the target arm's displayed ordinal
                let text = match label {
                    FallthroughLabel::Derived => self.markers.get(target).unwrap_or_else(|| {
                        panic!("no ordered-list marker for arm anchor {target:?}")
                    }),
                    FallthroughLabel::Explicit(text) => text,
                };
                format!("+++<sub class=\"bk-mark\">[<a href=\"#{target}\">→ {text}</a>]</sub>+++")
            }
            Prose::Seq(proses) => proses
                .iter()
                .map(|prose| self.prose(prose, link_ctx, lint))
                .collect(),
            Prose::Empty => String::new(),
        }
    }

    /// Serializes blocks using the fragment's collected arm markers.
    fn block(&mut self, block: &Block) -> String {
        match block {
            Block::Empty => String::new(),
            Block::Raw(text) => text.clone(),
            Block::Inline(prose) => self.prose(prose, None, true),
            Block::Concat(blocks) => blocks.iter().map(|block| self.block(block)).collect(),
            Block::Seq(blocks) => blocks
                .iter()
                .map(|block| self.block(block))
                .collect::<Vec<_>>()
                .join("\n"),
            Block::Item(level, kind, prose, block_body) => {
                // Only ordered arms emit anchors
                let (bullet, anchor) = match kind {
                    ItemKind::Unordered => (adoc_unordered_bullet(*level), String::new()),
                    ItemKind::Ordered(anchor_opt) => (
                        adoc_ordered_bullet(*level),
                        anchor_opt
                            .as_ref()
                            .map(|anchor| {
                                format!(
                                    "+++<span class=\"bk-arm-anchor\" id=\"{anchor}\"></span>+++"
                                )
                            })
                            .unwrap_or_default(),
                    ),
                };
                let mut text = format!("{bullet}{anchor}{}", self.prose(prose, None, true));
                // Empty bodies leave no trailing newline
                let text_body = self.block(block_body);
                if !text_body.is_empty() {
                    text.push('\n');
                    text.push_str(&text_body);
                }
                text
            }
            Block::Table(cols, header, rows) => {
                // Render header links above the serialized row cells
                let text_header = header
                    .iter()
                    .map(|prose| self.prose(prose, None, true))
                    .collect::<Vec<_>>()
                    .join(" | ");
                let text_rows = rows
                    .iter()
                    .map(|row| format!("| {}", row.join(" | ")))
                    .collect::<Vec<_>>()
                    .join("\n");
                format!(
                    "[cols=\"{cols}\", options=\"header\"]\n|===\n| {text_header} \n\n{text_rows}\n\n|==="
                )
            }
        }
    }
}

/// Serializes prose with definition names as link targets.
pub fn ser_prose(prose: &Prose) -> String {
    ser_prose_with_anchor(prose, &subject_name)
}

/// Serializes prose using the enclosing document's anchor resolver.
pub fn ser_prose_with_anchor(prose: &Prose, anchor: &dyn Fn(&Subject) -> Option<String>) -> String {
    Serializer { anchor, markers: BTreeMap::new(), warned: BTreeSet::new() }
        .prose(prose, None, true)
}

/// Serializes a link label without creating nested cross-references.
pub fn ser_prose_in_link(prose: &Prose) -> String {
    Serializer { anchor: &|_| None, markers: BTreeMap::new(), warned: BTreeSet::new() }.prose(
        prose,
        Some(""),
        false,
    )
}

/// Serializes code without adding monospace markup.
pub fn ser_code(code: &Code) -> String {
    ser_code_with_anchor(code, &subject_name)
}

/// Serializes code using the enclosing document's anchor resolver.
pub fn ser_code_with_anchor(code: &Code, anchor: &dyn Fn(&Subject) -> Option<String>) -> String {
    Serializer { anchor, markers: BTreeMap::new(), warned: BTreeSet::new() }
        .code(code, None, false, false)
}

/// Serializes a complete block with definition names as link targets.
pub fn ser_block(block: &Block) -> String {
    ser_block_with_anchor(block, &subject_name)
}

/// Resolves arm labels before serializing a fragment with custom anchors.
pub fn ser_block_with_anchor(block: &Block, anchor: &dyn Fn(&Subject) -> Option<String>) -> String {
    let mut serializer = Serializer { anchor, markers: BTreeMap::new(), warned: BTreeSet::new() };
    collect_markers(block, &mut serializer.markers, &mut BTreeMap::new());
    serializer.block(block)
}

// == Visible widths

/// Measures prose before adding code and link markup.
pub fn width_prose(prose: &Prose) -> usize {
    match prose {
        Prose::Text(text) => text.len(),
        Prose::Code(code) => width_code(code),
        Prose::Link(_, prose) => width_prose(prose),
        Prose::Fallthrough(_, FallthroughLabel::Derived) | Prose::Empty => 0,
        Prose::Fallthrough(_, FallthroughLabel::Explicit(text)) => text.len() + 4,
        Prose::Seq(proses) => proses.iter().map(width_prose).sum(),
    }
}

/// Measures code before adding monospace and link markup.
pub fn width_code(code: &Code) -> usize {
    match code {
        Code::Token(text) => text.len(),
        Code::Link(_, code) => width_code(code),
        Code::Seq(codes) => codes.iter().map(width_code).sum(),
        Code::Empty => 0,
    }
}
