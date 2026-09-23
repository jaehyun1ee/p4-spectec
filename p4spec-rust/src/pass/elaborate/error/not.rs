//! Notation shape, token, and argument diagnostics
//!
//! Matching sites supply the complete notation and its declaration kind.
//! Argument checks retain the declaration type span even inside nested notation.

use crate::{
    diagnostic::Label,
    lang::{
        common::{
            notation::{
                atom::Atom,
                mixfix::{AtomPhrase, Mixfix},
            },
            source::Span,
        },
        il::ast as il,
        traits::{at::At, print::Print},
    },
};

use super::super::expect::{NotExpect, NotExpectKind};
use super::{ElabError, cause};

const NOTATION_SHAPE_MISMATCH: &str = "elab/notation-shape-mismatch";
const NOTATION_TOKEN_MISMATCH: &str = "elab/notation-token-mismatch";

/// Reports incompatible notation shapes without guessing a missing suffix.
pub(in crate::pass::elaborate) fn notation_shape_mismatch(
    span: &Span,
    expect: &NotExpect<'_>,
) -> ElabError {
    let subject = notation_subject(expect.kind);
    let not_typ_il = expect.not_typ_il;
    cause(
        NOTATION_SHAPE_MISMATCH,
        format!("expression does not match {subject}"),
        vec![
            Label::primary(span, ""),
            Label::secondary(&not_typ_il.node.at(), "notation declared here"),
        ],
        vec![format!("expected notation: {}", not_typ_il.to_string())],
    )
}

/// Prints a token spelling without adding a second pair of quotes.
pub(in crate::pass::elaborate) fn atom_text(atom: &AtomPhrase) -> String {
    match &atom.node {
        Atom::Operator(text) => text.clone(),
        _ => atom.to_string(),
    }
}

/// Reports differing tokens at corresponding notation positions.
pub(in crate::pass::elaborate) fn notation_token_mismatch(
    atom_expect: &AtomPhrase,
    atom: &AtomPhrase,
    expect: &NotExpect<'_>,
) -> ElabError {
    let subject = notation_subject(expect.kind);
    let not_typ_il = expect.not_typ_il;
    // A single literal is already fully described by the mismatch message
    let notes = match &not_typ_il.node {
        Mixfix::Atom(_) => Vec::new(),
        _ => vec![format!("expected {subject}: {}", not_typ_il.to_string())],
    };
    cause(
        NOTATION_TOKEN_MISMATCH,
        format!("expected '{}', but found '{}'", atom_text(atom_expect), atom_text(atom)),
        vec![
            Label::primary(&atom.span, "unexpected token"),
            Label::secondary(&atom_expect.span, "expected token declared here"),
        ],
        notes,
    )
}

/// Keeps each note line within 80 columns after the renderer's `  = ` prefix.
const NOTE_WIDTH: usize = 76;

/// Lists alternative notations in one type scope without merging candidates.
pub(in crate::pass::elaborate) fn other_expected_notations(
    typ_il: &il::Typ,
    not_typs_il: &[&il::NotTyp],
) -> String {
    let texts: Vec<_> = not_typs_il
        .iter()
        .map(|not_typ_il| not_typ_il.to_string())
        .collect();
    // Identical spelling does not erase distinct candidate declarations
    let texts_listed = texts.iter().zip(not_typs_il).map(|(text, not_typ_il)| {
        if texts
            .iter()
            .filter(|text_other| *text_other == text)
            .count()
            > 1
        {
            let pos = &not_typ_il.span.left;
            format!("{text} ({}:{}:{})", pos.file, pos.line, pos.column + 1)
        } else {
            text.clone()
        }
    });
    let mut note = format!("other expected notations for '{}':", typ_il.to_string());
    let mut width = note.chars().count();
    // Wrap only between complete notations, keeping semicolon separators
    for (idx, text_listed) in texts_listed.enumerate() {
        if idx != 0 {
            note.push(';');
            width += 1;
        }
        let len = text_listed.chars().count();
        if width + 1 + len > NOTE_WIDTH {
            note.push_str("\n  ");
            width = 2;
        } else {
            note.push(' ');
            width += 1;
        }
        note.push_str(&text_listed);
        width += len;
    }
    note
}

/// Names the owner of a complete notation declaration.
fn notation_subject(kind: NotExpectKind<'_>) -> String {
    match kind {
        NotExpectKind::Rel(id) => format!("notation of relation '{}'", id.node),
        NotExpectKind::Variant => "notation".to_owned(),
    }
}

/// Names a notation argument using its zero-based position.
pub(super) fn notation_argument_subject(idx: usize, kind: NotExpectKind<'_>) -> String {
    let subject = match kind {
        NotExpectKind::Rel(id) => format!("relation '{}'", id.node),
        NotExpectKind::Variant => "notation".to_owned(),
    };
    format!("argument {idx} of {subject}")
}

const NOTATION_ARGUMENT_TYPE_MISMATCH: &str = "elab/notation-argument-type-mismatch";

/// Reports a notation argument that cannot be cast to its expected type.
pub(super) fn notation_argument_type_mismatch(
    idx: usize,
    kind: NotExpectKind<'_>,
    typ_expect_il: &il::Typ,
    typ_infer_il: &il::Typ,
) -> ElabError {
    super::type_mismatch(
        NOTATION_ARGUMENT_TYPE_MISMATCH,
        notation_argument_subject(idx, kind),
        typ_expect_il,
        typ_infer_il,
    )
}
