//! Text formatting for AsciiDoc prose
//!
//! Escaping preserves literal punctuation inside code and links.
//! List markers use the nesting level shared by the block serializer.

// == Inline text

/// The largest visible width kept inline by the prose renderer.
pub const ADOC_WIDTH_SHORT: usize = 30;

/// Wraps occurrences of a character in inline passthroughs.
pub fn adoc_escape(character: char, text: &str) -> String {
    text.replace(character, &format!("+{character}+"))
}

/// Formats a subscript.
pub fn adoc_subscript(text: &str) -> String {
    format!("~{text}~")
}

/// Formats a superscript.
pub fn adoc_superscript(text: &str) -> String {
    format!("^{text}^")
}

/// Formats an unconstrained monospace span.
pub fn adoc_mono(text: &str) -> String {
    format!("``{text}``")
}

/// Escapes quotes that could start AsciiDoc quotation markup.
pub fn adoc_escape_quotes(text: &str) -> String {
    text.replace('"', "{quot}")
}

/// Escapes quotes when more than one quoted phrase shares a span.
pub fn adoc_escape_ambiguous_quotes(text: &str) -> String {
    if text.matches('"').count() > 2 { adoc_escape_quotes(text) } else { text.to_owned() }
}

/// Formats each nonempty word separately so code can wrap at spaces.
pub fn adoc_mono_chopped(text: &str) -> String {
    adoc_escape_ambiguous_quotes(text)
        .split(' ')
        .map(|text| if text.is_empty() { String::new() } else { adoc_mono(text) })
        .collect::<Vec<_>>()
        .join(" ")
}

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
            "Warning: Asciidoc link text contains both brackets and angle brackets. Link may not render correctly.\n\t{text}"
        );
        text.to_owned()
    }
}

// == Indentation

/// Returns an ordered-list marker at the requested nesting level.
pub fn adoc_ordered_bullet(level: usize) -> String {
    format!("{}{} ", " ".repeat(level), ".".repeat(level + 1))
}

/// Returns an unordered-list marker at the requested nesting level.
pub fn adoc_unordered_bullet(level: usize) -> String {
    format!("{}{} ", " ".repeat(level), "*".repeat(level + 1))
}

/// Starts continuation lines as unordered list entries.
pub fn reindent_lines(level: usize, text: &str) -> String {
    text.replace('\n', &format!("\n{}", adoc_unordered_bullet(level)))
}

/// Joins lines without inserting a separator.
pub fn unindent_lines(text: &str) -> String {
    text.replace('\n', "")
}
