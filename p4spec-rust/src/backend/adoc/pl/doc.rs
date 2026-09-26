//! AsciiDoc prose, code spans, and document blocks
//!
//! ```text
//! Code(Seq([Token("a "), Link(Direct("f"), Token("b"))]))
//! -> ``a`` xref:f[``b``]
//!
//! Link(Direct("t"), Text("a[b]"))
//! -> <<t,a[b]>>
//!
//! Item { 0, Ordered(Some("arm")), Text("Done"), Empty }
//! -> . +++<span class="bk-arm-anchor" id="arm"></span>+++Done
//! ```

use std::collections::{BTreeMap, BTreeSet};

use super::utils::{adoc_link, adoc_mono_chopped, adoc_ordered_bullet, adoc_unordered_bullet};

// == Documents

// - Prose
//
//   Seq([Text("the "), Code(Token("x y"))])   -> the ``x`` ``y``
//   PlainCode(Token("x y"))                   -> x y

/// Inline prose and embedded code.
#[derive(Clone, Debug, PartialEq)]
pub enum Prose {
    /// Emits AsciiDoc text verbatim, including any existing markup.
    Text(String),
    /// Renders linked code with monospace markup around each word.
    Code(Code),
    /// Renders linked code without adding monospace markup.
    PlainCode(Code),
    /// Links the body, suppressing resolved links nested inside it.
    Link(Link, Box<Prose>),
    /// Links to an arm or group using its anchor and displayed label.
    Fallthrough(String, FallthroughLabel),
    /// Concatenates prose without inserting separators.
    Seq(Vec<Prose>),
    /// Emits no text and permits capitalization to continue.
    Empty,
}

// - Code
//
//   Seq([Token("a"), Token("b")])       -> ``ab``
//   Link(Direct("f"), Token("f(x)"))   -> xref:f[``f(x)``]

/// Code tokens with optional cross-references.
#[derive(Clone, Debug, PartialEq)]
pub enum Code {
    /// Emits pre-escaped code text, coalescing adjacent compatible tokens.
    Token(String),
    /// Links the enclosed code unless an outer link already owns the span.
    Link(Link, Box<Code>),
    /// Concatenates code without inserting separators or span boundaries.
    Seq(Vec<Code>),
    /// Emits no tokens and does not split a code span.
    Empty,
}

// - Links
//
//   Link(Direct("t"), Text("x"))              -> xref:t[x]
//   Link(Subject(Function("f")), Text("x"))   -> xref:f[x]
//   ... with an unresolving anchor            -> x

/// A concrete target or a reference resolved by the enclosing document.
#[derive(Clone, Debug, PartialEq)]
pub enum Link {
    /// Uses the target verbatim, bypassing the subject resolver.
    Direct(String),
    /// Resolves the subject, preserving only the body if unresolved.
    Subject(Subject),
}

/// A definition referenced by prose.
#[derive(Clone, Debug, PartialEq)]
pub enum Subject {
    /// Identifies a function by its source name without the dollar prefix.
    Function(String),
    /// Identifies a relation by its source name.
    Relation(String),
}

// - Fallthrough labels
//
//   Fallthrough("t", Explicit("else"))
//   -> +++<sub class="bk-mark">[<a href="#t">→ else</a>]</sub>+++

/// A fallthrough marker inferred from its target or supplied by a group.
#[derive(Clone, Debug, PartialEq)]
pub enum FallthroughLabel {
    /// Uses the target arm's list marker from the same serialized block.
    ///
    /// Block serialization panics if the target has no ordered arm anchor.
    /// Standalone prose serialization cannot resolve derived labels.
    Derived,
    /// Uses the supplied label, such as a group name or otherwise marker.
    Explicit(String),
}

// - Blocks
//
//   Item { 0, Ordered(None), Text("A"), Empty }   -> . A
//   Item { 1, Unordered, Text("B"), Empty }       ->  ** B
//   Seq([Raw("x"), Raw("y")])                     -> x
//                                                    y
//   Concat([Raw("x"), Raw("y")])                  -> xy

/// A list entry, optionally defining an ordered arm anchor.
#[derive(Clone, Debug, PartialEq)]
pub enum ItemKind {
    /// Advances the ordered list, optionally defining an arm anchor.
    Ordered(Option<String>),
    /// Emits a bullet and resets the ordered counter at this level.
    Unordered,
}

/// A document fragment with explicit list nesting and table boundaries.
#[derive(Clone, Debug, PartialEq)]
pub enum Block {
    /// Emits no text and does not advance list counters.
    Empty,
    /// Emits AsciiDoc verbatim without inspecting links or list markers.
    Raw(String),
    /// Renders prose without adding line breaks or a list marker.
    Inline(Prose),
    /// Renders a list heading followed by a nonempty body on the next line.
    Item {
        /// The zero-based nesting level used for bullets and arm labels.
        level: usize,
        /// The list style and optional ordered arm anchor.
        kind: ItemKind,
        /// The prose following the list marker.
        prose_head: Prose,
        /// The continuation, with nested entries carrying their own levels.
        block_body: Box<Block>,
    },
    /// Concatenates blocks without inserting separators.
    Concat(Vec<Block>),
    /// Joins blocks with one newline between adjacent blocks.
    Seq(Vec<Block>),
    /// Renders a table with its column count derived from the header.
    Table {
        /// The header cells, including their inline prose formatting.
        header: Vec<Prose>,
        /// Code cells resolved at serialization without monospace markup.
        ///
        /// Each row must have as many cells as the header.
        rows: Vec<Vec<Code>>,
    },
}

// == Constructors

// - Interleaving
//
//   join_items([a, b, c], |idx| s_idx)   -> [a, s_1, b, s_2, c]

/// Interleaves items with separators selected by the following item's index.
pub(super) fn join_items<Item>(
    items: impl IntoIterator<Item = Item>,
    mut separator: impl FnMut(usize) -> Item,
) -> Vec<Item> {
    let mut items_joined = Vec::new();
    // Move each item into one buffer without allocating intermediate lists
    for (idx, item) in items.into_iter().enumerate() {
        // A separator belongs only between two items
        if idx > 0 {
            items_joined.push(separator(idx));
        }
        items_joined.push(item);
    }
    items_joined
}

impl Prose {
    // - Prose constructors
    //
    //   Prose::text("x")            -> Text("x")
    //   Prose::link(link, prose)    -> Link(link, prose)
    //   Prose::fallthrough(a, l)    -> Fallthrough(a, l)
    //   Prose::join(", ", [x, y])   -> Seq([x, Text(", "), y])

    pub fn text(text: impl Into<String>) -> Prose {
        Prose::Text(text.into())
    }

    pub fn code(code: Code) -> Prose {
        Prose::Code(code)
    }

    pub fn link(link: Link, prose: Prose) -> Prose {
        Prose::Link(link, Box::new(prose))
    }

    pub fn fallthrough(anchor: String, label: FallthroughLabel) -> Prose {
        Prose::Fallthrough(anchor, label)
    }

    pub fn seq(proses: impl IntoIterator<Item = Prose>) -> Prose {
        Prose::Seq(proses.into_iter().collect())
    }

    pub fn join(separator: &str, proses: impl IntoIterator<Item = Prose>) -> Prose {
        let proses_joined = join_items(proses, |_| Prose::text(separator));
        Prose::seq(proses_joined)
    }
}

impl Code {
    // - Code constructors
    //
    //   Code::token("x")           -> Token("x")
    //   Code::join(", ", [x, y])   -> Seq([x, Token(", "), y])

    pub fn token(text: impl Into<String>) -> Code {
        Code::Token(text.into())
    }

    pub fn link(link: Link, code: Code) -> Code {
        Code::Link(link, Box::new(code))
    }

    pub fn seq(codes: impl IntoIterator<Item = Code>) -> Code {
        Code::Seq(codes.into_iter().collect())
    }

    pub fn join(separator: &str, codes: impl IntoIterator<Item = Code>) -> Code {
        let codes_joined = join_items(codes, |_| Code::token(separator));
        Code::seq(codes_joined)
    }
}

impl Block {
    // - Block constructors
    //
    //   Block::item_ordered(0, prose)     -> Item { 0, Ordered(None), prose, Empty }
    //   Block::item_unordered(1, prose)   -> Item { 1, Unordered, prose, Empty }

    pub fn raw(text: impl Into<String>) -> Block {
        Block::Raw(text.into())
    }

    pub fn inline(prose: Prose) -> Block {
        Block::Inline(prose)
    }

    pub fn concat(blocks: impl IntoIterator<Item = Block>) -> Block {
        Block::Concat(blocks.into_iter().collect())
    }

    pub fn seq(blocks: impl IntoIterator<Item = Block>) -> Block {
        Block::Seq(blocks.into_iter().collect())
    }

    pub fn item_ordered(level: usize, prose_head: Prose) -> Block {
        Block::item_ordered_body(level, None, prose_head, Block::Empty)
    }

    pub fn item_ordered_body(
        level: usize,
        anchor: Option<String>,
        prose_head: Prose,
        block_body: Block,
    ) -> Block {
        let kind = ItemKind::Ordered(anchor);
        Block::Item { level, kind, prose_head, block_body: Box::new(block_body) }
    }

    pub fn item_unordered(level: usize, prose_head: Prose) -> Block {
        let kind = ItemKind::Unordered;
        Block::Item { level, kind, prose_head, block_body: Box::new(Block::Empty) }
    }
}

// == Capitalization

/// Whether capitalization found text, can continue, or reached protected prose.
enum CapStep {
    /// Capitalized an initial letter, ending the search.
    Done,
    /// Found no eligible initial letter, allowing the next piece to be tried.
    Skip,
    /// Reached protected code or a link, ending the search without changes.
    Stop,
}

impl Prose {
    // - Prose capitalization
    //
    //   Seq([Empty, Text("hello")])               -> Hello
    //   Seq([Text("("), Text("a")])               -> (A
    //   Seq([Code(Token("x")), Text(" stays")])   -> ``x`` stays

    fn capitalize_step(&mut self) -> CapStep {
        match self {
            Prose::Text(text) => {
                // Capitalize only an initial ASCII letter
                if text.starts_with(|character: char| character.is_ascii_alphabetic()) {
                    text[..1].make_ascii_uppercase();
                    CapStep::Done
                } else {
                    CapStep::Skip
                }
            }
            Prose::Code(_) | Prose::PlainCode(_) | Prose::Link(..) | Prose::Fallthrough(..) => {
                CapStep::Stop
            }
            Prose::Seq(proses) => {
                // Empty or punctuation-only pieces leave the next piece eligible
                for prose in proses {
                    match prose.capitalize_step() {
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
    pub fn capitalize_first(mut self) -> Prose {
        self.capitalize_step();
        self
    }
}

impl Block {
    // - Block capitalization
    //
    //   Item { 0, Ordered(None), Text("if x"), Empty }                      -> . If x
    //   Seq([Item { 0, Ordered(None), Empty, Empty }, Inline(Text("x"))])   -> .
    //                                                                          X
    //   Seq([Table { .. }, Inline(Text("x"))])                              -> x unchanged

    fn capitalize_step(&mut self) -> CapStep {
        match self {
            Block::Empty | Block::Raw(_) => CapStep::Skip,
            Block::Inline(prose) => prose.capitalize_step(),
            Block::Item { prose_head, block_body, .. } => match prose_head.capitalize_step() {
                // An item without eligible heading text continues into its body
                CapStep::Skip => block_body.capitalize_step(),
                step => step,
            },
            Block::Concat(blocks) | Block::Seq(blocks) => {
                for block in blocks {
                    match block.capitalize_step() {
                        CapStep::Skip => {}
                        step => return step,
                    }
                }
                CapStep::Skip
            }
            Block::Table { .. } => CapStep::Stop,
        }
    }

    /// Capitalizes the first eligible block heading or inline text.
    pub fn capitalize_first(mut self) -> Block {
        self.capitalize_step();
        self
    }
}

// == Ordered-list markers

// - Roman numerals
//
//   4    -> iv
//   14   -> xiv
//   90   -> xc

fn roman_of_num(num: usize) -> String {
    // Labels follow the source renderer's two-digit conversion
    let units = ["", "i", "ii", "iii", "iv", "v", "vi", "vii", "viii", "ix"];
    let tens = ["", "x", "xx", "xxx", "xl", "l", "lx", "lxx", "lxxx", "xc"];
    format!("{}{}", tens[num / 10 % 10], units[num % 10])
}

// - Ordered-list styles
//
//   level 0, idx 1    -> 2
//   level 1, idx 0    -> a
//   level 1, idx 26   -> arm27
//   level 2, idx 3    -> iv
//   level 3, idx 1    -> B
//   level 4, idx 0    -> I
//   level 5, idx 0    -> 1

/// AsciiDoc's five-level ordered-list style cycle.
#[derive(Clone, Copy)]
enum OrderedStyle {
    Arabic,
    LowerAlpha,
    LowerRoman,
    UpperAlpha,
    UpperRoman,
}

impl OrderedStyle {
    fn of_level(level: usize) -> OrderedStyle {
        match level % 5 {
            0 => OrderedStyle::Arabic,
            1 => OrderedStyle::LowerAlpha,
            2 => OrderedStyle::LowerRoman,
            3 => OrderedStyle::UpperAlpha,
            _ => OrderedStyle::UpperRoman,
        }
    }

    fn marker(self, idx: usize) -> String {
        let num = idx + 1;
        match self {
            OrderedStyle::Arabic => num.to_string(),
            OrderedStyle::LowerAlpha if idx < 26 => char::from(b'a' + idx as u8).to_string(),
            OrderedStyle::UpperAlpha if idx < 26 => char::from(b'A' + idx as u8).to_string(),
            // Alphabetic lists use an explicit fallback beyond the alphabet
            OrderedStyle::LowerAlpha | OrderedStyle::UpperAlpha => format!("arm{num}"),
            OrderedStyle::LowerRoman => roman_of_num(num),
            OrderedStyle::UpperRoman => roman_of_num(num).to_ascii_uppercase(),
        }
    }
}

impl Block {
    // - Arm markers
    //
    //   Item { 0, Ordered(None), Text("Choose"), Seq([
    //       Item { 1, Ordered(Some("one")), Fallthrough("two", Derived), Empty },
    //       Item { 1, Ordered(Some("two")), Text("Done"), Empty },
    //   ]) }
    //   -> {"one": "a", "two": "b"}

    /// Maps each ordered arm anchor to the marker displayed for its item.
    fn anchor_markers(&self) -> BTreeMap<String, String> {
        let mut markers = BTreeMap::new();
        self.collect_anchor_markers(&mut markers, &mut BTreeMap::new());
        markers
    }

    /// Records arm labels in document order, resetting lists at shallower entries.
    fn collect_anchor_markers(
        &self,
        markers: &mut BTreeMap<String, String>,
        ordinals: &mut BTreeMap<usize, usize>,
    ) {
        match self {
            Block::Item { level, kind, block_body, .. } => {
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
                            let style = OrderedStyle::of_level(*level);
                            markers.insert(anchor.clone(), style.marker(*ordinal));
                        }
                        *ordinal += 1;
                    }
                }
                block_body.collect_anchor_markers(markers, ordinals);
            }
            Block::Concat(blocks) | Block::Seq(blocks) => {
                // Concatenation preserves the enclosing list's position
                for block in blocks {
                    block.collect_anchor_markers(markers, ordinals);
                }
            }
            Block::Empty | Block::Raw(_) | Block::Inline(_) | Block::Table { .. } => {}
        }
    }
}

// == Anchor resolution
//
//   subject_name(Function("f"))        -> Some("f")
//   Link::Direct("t").target(anchor)   -> Some("t")

/// Resolves a subject to its unqualified definition name.
pub fn subject_name(subject: &Subject) -> Option<String> {
    match subject {
        Subject::Function(id) | Subject::Relation(id) => Some(id.clone()),
    }
}

impl Link {
    fn target(&self, anchor: &dyn Fn(&Subject) -> Option<String>) -> Option<String> {
        match self {
            Link::Direct(target) => Some(target.clone()),
            Link::Subject(subject) => anchor(subject),
        }
    }
}

// == Serialization

/// A run of code tokens sharing one link target.
struct CodeSegment {
    target: Option<String>,
    text: String,
}

/// Selects whether serialized code receives monospace markup.
#[derive(Clone, Copy)]
enum CodeStyle {
    /// Wraps each word in monospace markup, as in inline prose.
    Mono,
    /// Emits code text as is, as in link labels and table cells.
    Plain,
}

impl Code {
    fn is_empty(&self) -> bool {
        match self {
            Code::Token(text) => text.is_empty(),
            Code::Link(_, code) => code.is_empty(),
            Code::Seq(codes) => codes.iter().all(Code::is_empty),
            Code::Empty => true,
        }
    }
}

/// Per-serialization anchor labels and warnings.
struct Serializer<'a> {
    anchor: &'a dyn Fn(&Subject) -> Option<String>,
    markers: BTreeMap<String, String>,
    warned: BTreeSet<String>,
}

impl<'a> Serializer<'a> {
    fn new(
        anchor: &'a dyn Fn(&Subject) -> Option<String>,
        markers: BTreeMap<String, String>,
    ) -> Self {
        Serializer { anchor, markers, warned: BTreeSet::new() }
    }

    // - Warnings
    //
    //   Link(Direct("a"), Link(Direct("b"), Text("x")))
    //   -> xref:a[x], warning that "b" is dropped inside "a"
    //
    //   Link(Direct(""), Text("x"))
    //   -> xref:[x], warning about the empty target

    fn warn(&mut self, text: String) {
        if self.warned.insert(text.clone()) {
            eprintln!("Warning: prose: {text}");
        }
    }

    fn warn_empty_target(&mut self, lint: bool, target: &str) {
        if lint && target.is_empty() {
            self.warn("link with empty target".into());
        }
    }

    fn warn_nested(&mut self, lint: bool, target_outer: &str, target_inner: &str) {
        if lint {
            self.warn(format!(
                "nested link: cross-reference to {target_inner:?} is dropped inside the link \
                 to {target_outer:?} (asciidoc cannot nest cross-references)"
            ));
        }
    }

    // - Code segments
    //
    //   Seq([Token("a "), Link(Direct("f"), Token("b")), Token(" c")])
    //   -> [(None, "a "), (Some("f"), "b"), (None, " c")]

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
            Code::Token(text) => Serializer::collect_token_code(text, target, segments),
            Code::Link(link, code_inner) => {
                self.collect_link_code(link, code_inner, target, link_ctx, lint, segments)
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

    fn collect_token_code(text: &str, target: Option<&str>, segments: &mut Vec<CodeSegment>) {
        // Empty tokens cannot split an existing span
        if text.is_empty() {
            return;
        }
        let segment_last = segments
            .last_mut()
            .filter(|segment| segment.target.as_deref() == target);
        if let Some(segment) = segment_last {
            segment.text.push_str(text);
        } else {
            let target = target.map(str::to_owned);
            segments.push(CodeSegment { target, text: text.to_owned() });
        }
    }

    fn collect_link_code(
        &mut self,
        link: &Link,
        code_inner: &Code,
        target: Option<&str>,
        link_ctx: Option<&str>,
        lint: bool,
        segments: &mut Vec<CodeSegment>,
    ) {
        // Unresolved subjects retain the surrounding link context
        let Some(target_inner) = link.target(self.anchor) else {
            self.collect_code(code_inner, target, link_ctx, lint, segments);
            return;
        };
        self.warn_empty_target(lint, &target_inner);
        if let Some(target_outer) = link_ctx {
            // Cross-references cannot nest in AsciiDoc
            self.warn_nested(lint, target_outer, &target_inner);
            self.collect_code(code_inner, target, link_ctx, lint, segments);
            return;
        }

        // The outermost resolved link owns its entire code span
        if lint && code_inner.is_empty() {
            self.warn(format!("link to {target_inner:?} has empty body"));
        }
        let target_inner = Some(target_inner.as_str());
        self.collect_code(code_inner, target_inner, target_inner, lint, segments);
    }

    // - Code
    //
    //   Seq([Token("a "), Link(Direct("f"), Token("b")), Token(" c")]) in Mono
    //   -> ``a`` xref:f[``b``] ``c``
    //
    //   Seq([Token("a "), Link(Direct("f"), Token("b")), Token(" c")]) in Plain
    //   -> a xref:f[b] c

    /// Serializes coalesced code, applying monospace only for inline prose.
    fn ser_code(
        &mut self,
        style: CodeStyle,
        code: &Code,
        link_ctx: Option<&str>,
        lint: bool,
    ) -> String {
        let mut segments = Vec::new();
        self.collect_code(code, None, link_ctx, lint, &mut segments);
        // Formatting after coalescing keeps adjacent tokens in one code span
        segments
            .into_iter()
            .map(|segment| {
                let text = match style {
                    CodeStyle::Mono => adoc_mono_chopped(&segment.text),
                    CodeStyle::Plain => segment.text,
                };
                match segment.target {
                    Some(target) => adoc_link(&target, &text),
                    None => text,
                }
            })
            .collect()
    }

    // - Prose
    //
    //   Seq([Text("the "), Code(Token("x"))])   -> the ``x``
    //   PlainCode(Token("x y"))                 -> x y

    /// Serializes inline prose with the enclosing cross-reference context.
    fn ser_prose(&mut self, prose: &Prose, link_ctx: Option<&str>, lint: bool) -> String {
        match prose {
            Prose::Text(text) => Serializer::ser_text_prose(text),
            Prose::Code(code) => self.ser_code(CodeStyle::Mono, code, link_ctx, lint),
            Prose::PlainCode(code) => self.ser_code(CodeStyle::Plain, code, link_ctx, lint),
            Prose::Link(link, prose_inner) => {
                self.ser_link_prose(link, prose_inner, link_ctx, lint)
            }
            Prose::Fallthrough(target, label) => self.ser_fallthrough_prose(target, label),
            Prose::Seq(proses) => self.ser_seq_prose(proses, link_ctx, lint),
            Prose::Empty => Serializer::ser_empty_prose(),
        }
    }

    // - Text prose
    //
    //   Text("a *b*")   -> a *b*

    fn ser_text_prose(text: &str) -> String {
        text.to_owned()
    }

    // - Linked prose
    //
    //   Link(Direct("t"), Text("x"))                             -> xref:t[x]
    //   Link(Direct("t"), Text("a[b]"))                          -> <<t,a[b]>>
    //   Link(Direct("a"), Code(Link(Direct("b"), Token("x"))))   -> xref:a[``x``]

    fn ser_link_prose(
        &mut self,
        link: &Link,
        prose_inner: &Prose,
        link_ctx: Option<&str>,
        lint: bool,
    ) -> String {
        // Preserve the body when the enclosing document has no target
        let Some(target) = link.target(self.anchor) else {
            return self.ser_prose(prose_inner, link_ctx, lint);
        };
        self.warn_empty_target(lint, &target);
        if let Some(target_outer) = link_ctx {
            // An outer link takes precedence over nested links
            self.warn_nested(lint, target_outer, &target);
            return self.ser_prose(prose_inner, link_ctx, lint);
        }

        // Format the complete body before choosing link delimiters
        let text = self.ser_prose(prose_inner, Some(&target), lint);
        if lint && text.is_empty() {
            self.warn(format!("link to {target:?} has empty body"));
        }
        adoc_link(&target, &text)
    }

    // - Fallthrough prose
    //
    //   Fallthrough("t", Explicit("else"))
    //   -> +++<sub class="bk-mark">[<a href="#t">→ else</a>]</sub>+++
    //
    //   Fallthrough("t", Derived), where the arm t has marker b
    //   -> +++<sub class="bk-mark">[<a href="#t">→ b</a>]</sub>+++

    fn ser_fallthrough_prose(&self, target: &str, label: &FallthroughLabel) -> String {
        // Derived labels refer to the target arm's displayed ordinal
        let text = match label {
            FallthroughLabel::Derived => self
                .markers
                .get(target)
                .unwrap_or_else(|| panic!("no ordered-list marker for arm anchor {target:?}")),
            FallthroughLabel::Explicit(text) => text,
        };
        format!("+++<sub class=\"bk-mark\">[<a href=\"#{target}\">→ {text}</a>]</sub>+++")
    }

    // - Prose sequences
    //
    //   Seq([Text("a"), Text("b")])   -> ab

    fn ser_seq_prose(&mut self, proses: &[Prose], link_ctx: Option<&str>, lint: bool) -> String {
        proses
            .iter()
            .map(|prose| self.ser_prose(prose, link_ctx, lint))
            .collect()
    }

    // - Empty prose
    //
    //   Empty   -> (empty)

    fn ser_empty_prose() -> String {
        String::new()
    }

    // - Block
    //
    //   Inline(Text("x"))           -> x
    //   Seq([Raw("x"), Raw("y")])   -> x
    //                                  y

    /// Serializes blocks using the fragment's collected arm markers.
    fn ser_block(&mut self, block: &Block) -> String {
        match block {
            Block::Empty => Serializer::ser_empty_block(),
            Block::Raw(text) => Serializer::ser_raw_block(text),
            Block::Inline(prose) => self.ser_prose(prose, None, true),
            Block::Concat(blocks) => self.ser_concat_block(blocks),
            Block::Seq(blocks) => self.ser_seq_block(blocks),
            Block::Item { level, kind, prose_head, block_body } => {
                self.ser_item_block(*level, kind, prose_head, block_body)
            }
            Block::Table { header, rows } => self.ser_table_block(header, rows),
        }
    }

    // - Empty blocks
    //
    //   Empty   -> (empty)

    fn ser_empty_block() -> String {
        String::new()
    }

    // - Raw blocks
    //
    //   Raw("x")   -> x

    fn ser_raw_block(text: &str) -> String {
        text.to_owned()
    }

    // - Concatenated blocks
    //
    //   Concat([Raw("x"), Raw("y")])   -> xy

    fn ser_concat_block(&mut self, blocks: &[Block]) -> String {
        blocks.iter().map(|block| self.ser_block(block)).collect()
    }

    // - Block sequences
    //
    //   Seq([Raw("x"), Raw("y")])   -> x
    //                                  y

    fn ser_seq_block(&mut self, blocks: &[Block]) -> String {
        let texts: Vec<String> = blocks.iter().map(|block| self.ser_block(block)).collect();
        texts.join("\n")
    }

    // - List items
    //
    //   Item { 0, Ordered(None), Text("A"), Empty }
    //   -> . A
    //
    //   Item { 1, Unordered, Text("B"), Empty }
    //   ->  ** B
    //
    //   Item { 1, Ordered(Some("arm")), Text("If x"),
    //          Item { 2, Ordered(None), Text("Return y."), Empty } }
    //   ->  .. +++<span class="bk-arm-anchor" id="arm"></span>+++If x
    //        ... Return y.

    fn ser_item_block(
        &mut self,
        level: usize,
        kind: &ItemKind,
        prose_head: &Prose,
        block_body: &Block,
    ) -> String {
        let text_bullet = match kind {
            ItemKind::Unordered => adoc_unordered_bullet(level),
            ItemKind::Ordered(_) => adoc_ordered_bullet(level),
        };
        // Only ordered arms emit anchors
        let text_anchor = match kind {
            ItemKind::Ordered(Some(anchor)) => {
                format!("+++<span class=\"bk-arm-anchor\" id=\"{anchor}\"></span>+++")
            }
            ItemKind::Ordered(None) | ItemKind::Unordered => String::new(),
        };
        let text_head = self.ser_prose(prose_head, None, true);
        let mut text = format!("{text_bullet}{text_anchor}{text_head}");
        // Empty bodies leave no trailing newline
        let text_body = self.ser_block(block_body);
        if !text_body.is_empty() {
            text.push('\n');
            text.push_str(&text_body);
        }
        text
    }

    // - Tables
    //
    //   Table { [Text("Input"), Text("Output")], [[Token("a"), Token("b")]] }
    //   -> [cols="2", options="header"]
    //      |===
    //      | Input | Output
    //
    //      | a | b
    //
    //      |===

    fn ser_table_block(&mut self, header: &[Prose], rows: &[Vec<Code>]) -> String {
        // Use the header as the single source of the column count
        let cols = header.len();
        let texts_header: Vec<String> = header
            .iter()
            .map(|prose| self.ser_prose(prose, None, true))
            .collect();
        let text_header = texts_header.join(" | ");
        // Resolve cell links in the enclosing document's context
        let mut texts_row = Vec::new();
        for row in rows {
            let texts_cell: Vec<String> = row
                .iter()
                .map(|code| self.ser_code(CodeStyle::Plain, code, None, false))
                .collect();
            texts_row.push(format!("| {}", texts_cell.join(" | ")));
        }
        let text_rows = texts_row.join("\n");
        format!(
            "[cols=\"{cols}\", options=\"header\"]\n|===\n| {text_header} \n\n{text_rows}\n\n|==="
        )
    }
}

// - Entry points
//
//   ser_prose(Link(Subject(Function("f")), Text("x")), &subject_name)
//   -> xref:f[x]
//
//   ser_prose_in_link(Link(Direct("t"), Code(Token("x"))))
//   -> ``x``
//
//   ser_code(Seq([Token("a "), Link(Direct("f"), Token("b"))]), &subject_name)
//   -> a xref:f[b]

/// Serializes prose using the enclosing document's anchor resolver.
pub fn ser_prose(prose: &Prose, anchor: &dyn Fn(&Subject) -> Option<String>) -> String {
    let mut serializer = Serializer::new(anchor, BTreeMap::new());
    serializer.ser_prose(prose, None, true)
}

/// Serializes a link label without creating nested cross-references.
pub fn ser_prose_in_link(prose: &Prose) -> String {
    // The empty outer target suppresses direct links as well as subjects
    Serializer::new(&|_| None, BTreeMap::new()).ser_prose(prose, Some(""), false)
}

/// Serializes code without monospace markup using the given anchor resolver.
pub fn ser_code(code: &Code, anchor: &dyn Fn(&Subject) -> Option<String>) -> String {
    let mut serializer = Serializer::new(anchor, BTreeMap::new());
    serializer.ser_code(CodeStyle::Plain, code, None, false)
}

/// Resolves arm labels before serializing a fragment with the given anchor resolver.
pub fn ser_block(block: &Block, anchor: &dyn Fn(&Subject) -> Option<String>) -> String {
    let markers = block.anchor_markers();
    let mut serializer = Serializer::new(anchor, markers);
    serializer.ser_block(block)
}

// == Visible widths
//
//   Seq([Text("a "), Code(Token("bc"))]).width()   -> 4
//   Fallthrough("t", Explicit("else")).width()     -> 8, counting the arrow label
//   Fallthrough("t", Derived).width()              -> 0

impl Prose {
    /// Measures prose before adding code and link markup.
    pub fn width(&self) -> usize {
        match self {
            Prose::Text(text) => text.len(),
            Prose::Code(code) | Prose::PlainCode(code) => code.width(),
            Prose::Link(_, prose) => prose.width(),
            Prose::Fallthrough(_, FallthroughLabel::Derived) | Prose::Empty => 0,
            Prose::Fallthrough(_, FallthroughLabel::Explicit(text)) => text.len() + 4,
            Prose::Seq(proses) => proses.iter().map(Prose::width).sum(),
        }
    }
}

impl Code {
    /// Measures code before adding monospace and link markup.
    pub fn width(&self) -> usize {
        match self {
            Code::Token(text) => text.len(),
            Code::Link(_, code) => code.width(),
            Code::Seq(codes) => codes.iter().map(Code::width).sum(),
            Code::Empty => 0,
        }
    }
}
