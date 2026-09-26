//! Text formatting for AsciiDoc prose
//!
//! ```text
//! adoc_mono_chopped("a \"b\" \"c\"")   -> ``a`` ``{quot}b{quot}`` ``{quot}c{quot}``
//! adoc_link("t", "a[b]")               -> <<t,a[b]>>
//! reindent_lines(1, "x\ny")            -> x
//!                                          ** y
//! ```

// == Inline text

/// The largest visible width kept inline by the prose renderer.
pub const ADOC_WIDTH_SHORT: usize = 30;

// - Scripts
//
//   adoc_subscript("max")   -> ~max~
//   adoc_superscript("?")   -> ^?^

/// Formats a subscript.
pub fn adoc_subscript(text: &str) -> String {
    format!("~{text}~")
}

/// Formats a superscript.
pub fn adoc_superscript(text: &str) -> String {
    format!("^{text}^")
}

// - Monospace
//
//   "a b"             -> ``a`` ``b``
//   "a  b"            -> ``a``  ``b``
//   "\"a b\""         -> ``"a`` ``b"``
//   "a \"b\" \"c\""   -> ``a`` ``{quot}b{quot}`` ``{quot}c{quot}``

/// Formats each nonempty word separately so code can wrap at spaces.
pub fn adoc_mono_chopped(text: &str) -> String {
    // Quotes spanning several phrases could start AsciiDoc quotation markup
    let text_escaped =
        if text.matches('"').count() > 2 { text.replace('"', "{quot}") } else { text.to_owned() };
    let texts_word: Vec<String> = text_escaped
        .split(' ')
        .map(|text_word| match text_word {
            "" => String::new(),
            _ => format!("``{text_word}``"),
        })
        .collect();
    texts_word.join(" ")
}

// - Cross-references
//
//   adoc_link("t", "x")      -> xref:t[x]
//   adoc_link("t", "a[b]")   -> <<t,a[b]>>
//   adoc_link("t", "a<b>")   -> xref:t[a<b>]

/// Chooses cross-reference delimiters that do not collide with the label.
pub fn adoc_link(target: &str, text: &str) -> String {
    // Brackets require the alternate cross-reference syntax
    if !text.contains(['[', ']']) {
        format!("xref:{target}[{text}]")
    } else if !text.contains(['<', '>']) {
        format!("<<{target},{text}>>")
    } else {
        // Neither delimiter can represent this label
        eprintln!(
            "Warning: Asciidoc link text contains both brackets and angle brackets. \
             Link may not render correctly.\n\t{text}"
        );
        text.to_owned()
    }
}

// == Indentation

// - List markers
//
//   adoc_ordered_bullet(0)     -> ". "
//   adoc_ordered_bullet(2)     -> "  ... "
//   adoc_unordered_bullet(1)   -> " ** "

/// Returns an ordered-list marker at the requested nesting level.
pub fn adoc_ordered_bullet(level: usize) -> String {
    let indent = " ".repeat(level);
    let marker = ".".repeat(level + 1);
    format!("{indent}{marker} ")
}

/// Returns an unordered-list marker at the requested nesting level.
pub fn adoc_unordered_bullet(level: usize) -> String {
    let indent = " ".repeat(level);
    let marker = "*".repeat(level + 1);
    format!("{indent}{marker} ")
}

// - Line joins
//
//   reindent_lines(1, "x\ny")   -> x
//                                   ** y
//   unindent_lines("x\ny")      -> xy

/// Starts continuation lines as unordered list entries.
pub fn reindent_lines(level: usize, text: &str) -> String {
    let bullet = adoc_unordered_bullet(level);
    text.replace('\n', &format!("\n{bullet}"))
}

/// Joins lines without inserting a separator.
pub fn unindent_lines(text: &str) -> String {
    text.replace('\n', "")
}
