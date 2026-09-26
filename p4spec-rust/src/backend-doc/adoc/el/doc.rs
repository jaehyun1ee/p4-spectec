//! Width-sensitive documents for elaboration-language AsciiDoc
//!
//! ```text
//! Doc::group(Doc::concat([
//!     Doc::text("f("),
//!     Doc::nest(4, Doc::concat([
//!         Doc::break_(""), Doc::text("x,"), Doc::break_(" "), Doc::text("y"),
//!     ])),
//!     Doc::text(")"),
//! ]))
//!
//!   render(7)   -> f(x, y)
//!   render(6)   -> f(
//!                      x,
//!                      y)
//! ```

// == Documents

// - Document trees
//
//   Text("x")                 -> x
//   Break(" ")                -> " " when flat, newline and indentation when broken
//   Line                      -> newline and indentation
//   Nest(4, Line)             -> newline and four more columns of indentation

/// A tree describing possible flat and broken layouts.
#[derive(Clone, Debug)]
pub(super) enum Doc {
    /// Emits nothing.
    Empty,
    /// Emits text without splitting it.
    Text(String),
    /// Emits flat text or starts an indented line.
    Break(String),
    /// Always starts an indented line.
    Line,
    /// Renders documents consecutively without nesting by sequence length.
    Concat(Vec<Doc>),
    /// Adds the offset to indentation at subsequent breaks in the document.
    Nest(usize, Box<Doc>),
    /// Chooses a flat layout when the enclosed document fits.
    Group(Box<Doc>),
}

// - Layout modes
//
//   Flat, Break(" ")          -> " "
//   Broken, Break(" ")        -> newline and indentation

/// Controls whether optional breaks emit text or start a new line.
#[derive(Clone, Copy)]
enum Mode {
    /// Emits each break's stored text within a group that fits.
    Flat,
    /// Starts indented lines, allowing nested groups to fit independently.
    Broken,
}

// - Rendering commands
//
//   Command { indent: 4, mode: Broken, doc: Break("") }   -> newline and four spaces

/// One pending document on the layout work stack.
#[derive(Clone, Copy)]
struct Command<'a> {
    indent: usize,
    mode: Mode,
    doc: &'a Doc,
}

// == Constructors

impl Doc {
    // - Primitive documents
    //
    //   Doc::text("")             -> Empty
    //   Doc::text("x")            -> Text("x")
    //   Doc::break_(" ")          -> Break(" ")
    //   Doc::nest(0, doc)         -> doc
    //   Doc::nest(2, doc)         -> Nest(2, doc)
    //   Doc::group(doc)           -> Group(doc)

    /// Constructs an indivisible text document, collapsing empty text.
    pub(super) fn text(text: impl Into<String>) -> Doc {
        let text = text.into();
        if text.is_empty() { Doc::Empty } else { Doc::Text(text) }
    }

    /// Constructs a break with the text used by its flat layout.
    pub(super) fn break_(text: impl Into<String>) -> Doc {
        Doc::Break(text.into())
    }

    /// Nests a document by the requested indentation.
    pub(super) fn nest(indent: usize, doc: Doc) -> Doc {
        if indent == 0 { doc } else { Doc::Nest(indent, Box::new(doc)) }
    }

    /// Groups a document so its breaks flatten together when they fit.
    pub(super) fn group(doc: Doc) -> Doc {
        Doc::Group(Box::new(doc))
    }

    // - Sequences
    //
    //   Doc::concat([x, y])       -> Concat([x, y])
    //   Doc::join(sep, [])        -> Empty
    //   Doc::join(sep, [x, y, z]) -> Concat([x, sep, y, sep, z])
    //   Doc::flow([x, y, z])      -> Concat([x, Group(Concat([Break(" "), y])),
    //                                           Group(Concat([Break(" "), z]))])

    /// Concatenates documents in iteration order.
    pub(super) fn concat(docs: impl IntoIterator<Item = Doc>) -> Doc {
        Doc::Concat(docs.into_iter().collect())
    }

    /// Joins documents with a shared separator.
    pub(super) fn join(separator: Doc, docs: impl IntoIterator<Item = Doc>) -> Doc {
        let mut docs = docs.into_iter();
        let Some(doc_head) = docs.next() else {
            return Doc::Empty;
        };
        // Store siblings together so dropping a wide list does not recurse
        let mut docs_joined = vec![doc_head];
        for doc in docs {
            docs_joined.push(separator.clone());
            docs_joined.push(doc);
        }
        Doc::concat(docs_joined)
    }

    /// Joins documents with spaces that may break independently.
    pub(super) fn flow(docs: impl IntoIterator<Item = Doc>) -> Doc {
        let mut docs = docs.into_iter();
        let Some(doc_head) = docs.next() else {
            return Doc::Empty;
        };
        let mut docs_flowed = vec![doc_head];
        for doc in docs {
            let doc_spaced = Doc::concat([Doc::break_(" "), doc]);
            docs_flowed.push(Doc::group(doc_spaced));
        }
        Doc::concat(docs_flowed)
    }
}

// == Layout

impl Doc {
    // - Fitting
    //
    //   fits(3, [Text("ab"), Break(" "), Text("c")] in Flat)     -> false
    //   fits(3, [Text("ab"), Break(" "), Text("c")] in Broken)   -> true

    /// Tests whether the queued flat layout reaches a forced newline within width.
    fn fits(mut width_remaining: isize, mut commands: Vec<Command<'_>>) -> bool {
        // Stop lookahead when the current line exceeds its available width
        while width_remaining >= 0 {
            // Exhausting the pending document means the line fits
            let Some(Command { indent, mode, doc }) = commands.pop() else {
                return true;
            };
            match (doc, mode) {
                // Empty documents consume no space
                (Doc::Empty, _) => {}
                // Literal text and flat breaks consume their byte width
                (Doc::Text(text), _) | (Doc::Break(text), Mode::Flat) => {
                    width_remaining -= text.len() as isize
                }
                // Broken breaks and forced lines end the current line
                (Doc::Break(_), Mode::Broken) | (Doc::Line, _) => return true,
                (Doc::Concat(docs), _) => {
                    // Push in reverse order to visit documents in source order
                    commands.extend(docs.iter().rev().map(|doc| Command { indent, mode, doc }));
                }
                (Doc::Nest(offset, doc), _) => {
                    // Indentation takes effect at the next line break
                    commands.push(Command { indent: indent + offset, mode, doc });
                }
                (Doc::Group(doc), _) => {
                    // Lookahead retains the enclosing mode until a line ends
                    commands.push(Command { indent, mode, doc });
                }
            }
        }
        false
    }

    // - Rendering
    //
    //   group("f(" nest(4, break("") "x," break(" ") "y") ")").render(7)   -> f(x, y)
    //   group("f(" nest(4, break("") "x," break(" ") "y") ")").render(6)   -> f(
    //                                                                             x,
    //                                                                             y)

    /// Renders a document using group fitting at the requested positive width.
    pub(super) fn render(&self, width: usize) -> String {
        assert!(width > 0, "Doc::render: width must be positive");

        let mut output = String::with_capacity(256);
        let mut column = 0;
        let mut commands = vec![Command { indent: 0, mode: Mode::Broken, doc: self }];

        // Keep nested layout traversal off the call stack
        while let Some(Command { indent, mode, doc }) = commands.pop() {
            match (doc, mode) {
                // Empty documents consume no space
                (Doc::Empty, _) => {}
                // Literal text and flat breaks consume their byte width
                (Doc::Text(text), _) | (Doc::Break(text), Mode::Flat) => {
                    output.push_str(text);
                    column += text.len();
                }
                // Broken breaks and forced lines start an indented line
                (Doc::Break(_), Mode::Broken) | (Doc::Line, _) => {
                    output.push('\n');
                    output.push_str(&" ".repeat(indent));
                    column = indent;
                }
                (Doc::Concat(docs), _) => {
                    // Push in reverse order to visit documents in source order
                    commands.extend(docs.iter().rev().map(|doc| Command { indent, mode, doc }));
                }
                (Doc::Nest(offset, doc), _) => {
                    // Indentation takes effect at the next line break
                    commands.push(Command { indent: indent + offset, mode, doc });
                }
                (Doc::Group(doc), _) => {
                    // Flatten the group only when it and the following text fit
                    let mode = match mode {
                        Mode::Flat => Mode::Flat,
                        Mode::Broken => {
                            let mut commands_flat = commands.clone();
                            commands_flat.push(Command { indent, mode: Mode::Flat, doc });
                            let width_remaining = width as isize - column as isize;
                            if Doc::fits(width_remaining, commands_flat) {
                                Mode::Flat
                            } else {
                                Mode::Broken
                            }
                        }
                    };
                    commands.push(Command { indent, mode, doc });
                }
            }
        }

        output
    }
}
