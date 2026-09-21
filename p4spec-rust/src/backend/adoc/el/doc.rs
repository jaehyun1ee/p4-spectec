//! Width-sensitive documents for elaboration-language AsciiDoc
//!
//! A document records alternative layouts before rendering. `Break` nodes
//! become their flat text when the enclosing group fits, or start an indented
//! line when it does not. Nested groups may still fit after an outer group
//! breaks. `render` selects layouts against a fixed positive width.

// == Documents

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
    /// Renders two documents consecutively.
    Cat(Box<Doc>, Box<Doc>),
    /// Increases indentation inside a document.
    Nest(usize, Box<Doc>),
    /// Chooses a flat layout when the enclosed document fits.
    Group(Box<Doc>),
}

#[derive(Clone, Copy)]
enum Mode {
    Flat,
    Broken,
}

#[derive(Clone, Copy)]
struct Command<'a> {
    indent: usize,
    mode: Mode,
    doc: &'a Doc,
}

// == Constructors

/// Constructs an empty document.
pub(super) fn empty() -> Doc {
    Doc::Empty
}

/// Constructs an indivisible text document, collapsing empty text.
pub(super) fn text(text: impl Into<String>) -> Doc {
    let text = text.into();
    if text.is_empty() { Doc::Empty } else { Doc::Text(text) }
}

/// Constructs a break with the text used by its flat layout.
pub(super) fn break_(text: impl Into<String>) -> Doc {
    Doc::Break(text.into())
}

/// Constructs a forced line break.
pub(super) fn line() -> Doc {
    Doc::Line
}

/// Nests a document by the requested indentation.
pub(super) fn nest(indent: usize, doc: Doc) -> Doc {
    if indent == 0 { doc } else { Doc::Nest(indent, Box::new(doc)) }
}

/// Groups a document so its breaks flatten together when they fit.
pub(super) fn group(doc: Doc) -> Doc {
    Doc::Group(Box::new(doc))
}

// == Combinators

/// Concatenates two documents, eliminating empty operands.
pub(super) fn cat(doc_l: Doc, doc_r: Doc) -> Doc {
    match (doc_l, doc_r) {
        (Doc::Empty, doc) | (doc, Doc::Empty) => doc,
        (doc_l, doc_r) => Doc::Cat(Box::new(doc_l), Box::new(doc_r)),
    }
}

/// Concatenates documents in iteration order.
pub(super) fn concat(docs: impl IntoIterator<Item = Doc>) -> Doc {
    docs.into_iter().fold(empty(), cat)
}

/// Joins documents with a shared separator.
pub(super) fn join(separator: Doc, docs: impl IntoIterator<Item = Doc>) -> Doc {
    let mut docs = docs.into_iter();
    let Some(doc_head) = docs.next() else {
        return empty();
    };
    docs.fold(doc_head, |doc_l, doc_r| cat(cat(doc_l, separator.clone()), doc_r))
}

/// Joins documents with spaces that may break independently.
pub(super) fn flow(docs: impl IntoIterator<Item = Doc>) -> Doc {
    let mut docs = docs.into_iter();
    let Some(doc_head) = docs.next() else {
        return empty();
    };
    docs.fold(doc_head, |doc_l, doc_r| cat(doc_l, group(cat(break_(" "), doc_r))))
}

// == Layout

/// Tests whether the queued flat layout reaches a forced newline within width.
fn fits(mut width_remaining: isize, mut commands: Vec<Command<'_>>) -> bool {
    // Stop lookahead when the current line exceeds its available width
    while width_remaining >= 0 {
        // Exhausting the pending document means the line fits
        let Some(Command { indent, mode, doc }) = commands.pop() else {
            return true;
        };
        match doc {
            // Empty documents consume no space
            Doc::Empty => {}
            // Literal text consumes its byte width
            Doc::Text(text) => width_remaining -= text.len() as isize,
            // Only a broken layout turns an optional break into a newline
            Doc::Break(text) => match mode {
                Mode::Flat => width_remaining -= text.len() as isize,
                Mode::Broken => return true,
            },
            // Forced lines end the current line in either mode
            Doc::Line => return true,
            Doc::Cat(doc_l, doc_r) => {
                // Push in reverse order to visit the left document first
                commands.push(Command { indent, mode, doc: doc_r });
                commands.push(Command { indent, mode, doc: doc_l });
            }
            Doc::Nest(offset, doc) => {
                // Indentation takes effect at the next line break
                commands.push(Command { indent: indent + offset, mode, doc });
            }
            Doc::Group(doc) => {
                // Lookahead retains the enclosing mode until a line ends
                let mode = match mode {
                    Mode::Flat => Mode::Flat,
                    Mode::Broken => Mode::Broken,
                };
                commands.push(Command { indent, mode, doc });
            }
        }
    }
    false
}

/// Renders a document using group fitting at the requested positive width.
pub(super) fn render(width: usize, doc: &Doc) -> String {
    assert!(width > 0, "Doc::render: width must be positive");

    let mut output = String::with_capacity(256);
    let mut column = 0;
    let mut commands = vec![Command { indent: 0, mode: Mode::Broken, doc }];

    // Keep nested layout traversal off the call stack
    while let Some(Command { indent, mode, doc }) = commands.pop() {
        match doc {
            // Empty documents consume no space
            Doc::Empty => {}
            // Literal text consumes its byte width
            Doc::Text(text) => {
                output.push_str(text);
                column += text.len();
            }
            // Only a broken layout turns an optional break into a newline
            Doc::Break(text) => match mode {
                Mode::Flat => {
                    output.push_str(text);
                    column += text.len();
                }
                Mode::Broken => {
                    output.push('\n');
                    output.push_str(&" ".repeat(indent));
                    column = indent;
                }
            },
            // Forced lines end the current line in either mode
            Doc::Line => {
                output.push('\n');
                output.push_str(&" ".repeat(indent));
                column = indent;
            }
            Doc::Cat(doc_l, doc_r) => {
                // Push in reverse order to visit the left document first
                commands.push(Command { indent, mode, doc: doc_r });
                commands.push(Command { indent, mode, doc: doc_l });
            }
            Doc::Nest(offset, doc) => {
                // Indentation takes effect at the next line break
                commands.push(Command { indent: indent + offset, mode, doc });
            }
            Doc::Group(doc) => {
                // Flatten the group only when it and the following text fit
                let mode = match mode {
                    Mode::Flat => Mode::Flat,
                    Mode::Broken => {
                        let mut commands_flat = commands.clone();
                        commands_flat.push(Command { indent, mode: Mode::Flat, doc });
                        if fits(width as isize - column as isize, commands_flat) {
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
