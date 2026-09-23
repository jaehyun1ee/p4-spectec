//! Notation shape, token, and argument diagnostics
//!
//! Matching sites supply the complete notation and an optional relation name.
//! Argument checks retain the declaration type span even inside nested notation.

use crate::{
    diagnostic::Label,
    lang::{
        common::{
            Id,
            notation::{atom::Atom, mixfix::AtomPhrase},
            source::Span,
        },
        il::ast as il,
        traits::{at::At, print::Print},
    },
};

use super::{ElabError, cause};

const NOTATION_SHAPE_MISMATCH: &str = "elab/notation-shape-mismatch";
const NOTATION_TOKEN_MISMATCH: &str = "elab/notation-token-mismatch";

/// Reports incompatible notation shapes without guessing a missing suffix.
pub(in crate::pass::elaborate) fn notation_shape_mismatch(
    span: &Span,
    not_typ_il: &il::NotTyp,
    id_rel: Option<&Id>,
) -> ElabError {
    let subject = id_rel
        .map_or_else(|| "notation".to_owned(), |id| format!("notation of relation '{}'", id.node));
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
    not_typ_il: &il::NotTyp,
    id_rel: Option<&Id>,
) -> ElabError {
    let subject = id_rel
        .map_or_else(|| "notation".to_owned(), |id| format!("notation of relation '{}'", id.node));
    cause(
        NOTATION_TOKEN_MISMATCH,
        format!("expected '{}', but found '{}'", atom_text(atom_expect), atom_text(atom)),
        vec![
            Label::primary(&atom.span, "unexpected token"),
            Label::secondary(&atom_expect.span, "expected token declared here"),
        ],
        vec![format!("expected {subject}: {}", not_typ_il.to_string())],
    )
}
