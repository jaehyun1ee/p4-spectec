//! Notation shape, token, and argument diagnostics
//!
//! Matching sites supply the complete notation and its declaration kind.
//! Argument checks retain the declaration type span even inside nested notation.

use crate::{
    diagnostic::Label,
    lang::{
        common::{
            notation::{atom::Atom, mixfix::AtomPhrase},
            source::Span,
        },
        il::ast as il,
        traits::{at::At, print::Print},
    },
};

use super::super::{
    expect::{NotExpect, NotExpectKind},
    notation::NotationReport,
};
use super::{ElabError, cause};

const NOTATION_SHAPE_MISMATCH: &str = "elab/notation-shape-mismatch";
const NOTATION_TOKEN_MISMATCH: &str = "elab/notation-token-mismatch";

/// Reports incompatible notation shapes without guessing a missing suffix.
pub(in crate::pass::elaborate) fn notation_shape_mismatch(
    span: &Span,
    expect: &NotExpect<'_>,
) -> NotationReport {
    let subject = notation_subject(expect.kind);
    let not_typ_il = expect.not_typ_il;
    NotationReport::shape(
        *cause(
            NOTATION_SHAPE_MISMATCH,
            format!("expression does not match {subject}"),
            vec![
                Label::primary(span, ""),
                Label::secondary(&not_typ_il.node.at(), "notation declared here"),
            ],
            vec![format!("expected notation: {}", not_typ_il.to_string())],
        ),
        span,
    )
}

/// Prints a token spelling without adding a second pair of quotes.
fn atom_text(atom: &AtomPhrase) -> String {
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
) -> NotationReport {
    let subject = notation_subject(expect.kind);
    let not_typ_il = expect.not_typ_il;
    // A single literal is already fully described by the mismatch message
    let notes = match &not_typ_il.node {
        crate::lang::common::notation::mixfix::Mixfix::Atom(_) => Vec::new(),
        _ => vec![format!("expected {subject}: {}", not_typ_il.to_string())],
    };
    NotationReport::token(
        *cause(
            NOTATION_TOKEN_MISMATCH,
            format!("expected '{}', but found '{}'", atom_text(atom_expect), atom_text(atom)),
            vec![
                Label::primary(&atom.span, "unexpected token"),
                Label::secondary(&atom_expect.span, "expected token declared here"),
            ],
            notes,
        ),
        &atom.span,
        &atom_text(atom_expect),
        &atom_text(atom),
    )
}

/// Lists alternative notations in one type scope without merging candidates.
pub(in crate::pass::elaborate) fn other_expected_notations(
    typ_il: &il::Typ,
    not_typs_il: &[&il::NotTyp],
) -> String {
    let texts: Vec<_> = not_typs_il
        .iter()
        .map(|not_typ_il| not_typ_il.to_string())
        .collect();
    let mut text = format!("other expected notations for '{}':", typ_il.to_string());
    let mut columns = text.chars().count();
    for (idx, (text_not, not_typ_il)) in texts.iter().zip(not_typs_il).enumerate() {
        // Identical spelling does not erase distinct candidate declarations
        let text_not = if texts
            .iter()
            .filter(|text_other| *text_other == text_not)
            .count()
            > 1
        {
            let pos = &not_typ_il.span.left;
            format!("{text_not} ({}:{}:{})", pos.file, pos.line, pos.column + 1)
        } else {
            text_not.clone()
        };
        // Wrap only between complete notations, keeping semicolon separators
        if idx != 0 {
            text.push(';');
            columns += 1;
        }
        let len = text_not.chars().count();
        if columns + 1 + len > 76 {
            text.push_str("\n  ");
            columns = 2;
        } else {
            text.push(' ');
            columns += 1;
        }
        text.push_str(&text_not);
        columns += len;
    }
    text
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
