//! Notation shape, token, and argument diagnostics
//!
//! Matching sites supply the complete notation and an optional relation name.
//! Argument checks retain the declaration type span even inside nested notation.

use crate::{
    diagnostic::{Label, Report, ReportKind},
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
const NOTATION_ARGUMENT_TYPE_MISMATCH: &str = "elab/notation-argument-type-mismatch";

/// Names the notation owner independently of the matching subtree.
fn notation_subject(id_rel: Option<&Id>) -> String {
    id_rel.map_or_else(|| "notation".to_owned(), |id| format!("relation '{}'", id.node))
}

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

/// Identifies one non-literal slot in a notation declaration.
pub(in crate::pass::elaborate) struct NotationArgument<'a> {
    /// Counts only argument slots, starting at zero.
    pub idx: usize,
    /// Names the relation when the notation belongs to one.
    pub id_rel: Option<&'a Id>,
    /// Retains the declaration's argument type and span.
    pub typ_il: &'a il::Typ,
}

impl NotationArgument<'_> {
    /// Names the argument using its zero-based position among type slots.
    fn subject(&self) -> String {
        format!("argument {} of {}", self.idx, notation_subject(self.id_rel))
    }

    /// Reports the actual type at the failed implicit cast.
    pub(in crate::pass::elaborate) fn type_mismatch(
        &self,
        typ_expect_il: &il::Typ,
        typ_infer_il: &il::Typ,
    ) -> ElabError {
        cause(
            NOTATION_ARGUMENT_TYPE_MISMATCH,
            format!(
                "{} expects '{}', but found '{}'",
                self.subject(),
                typ_expect_il.to_string(),
                typ_infer_il.to_string(),
            ),
            vec![Label::primary(&typ_infer_il.span, "")],
            Vec::new(),
        )
    }

    /// Links the existing cause to the declared slot without wrapping it.
    pub(in crate::pass::elaborate) fn annotate(&self, reports: &mut [Report]) {
        // Add context to causes without classifying their codes or messages
        let mut reports_pending: Vec<_> = reports.iter_mut().collect();
        while let Some(report) = reports_pending.pop() {
            if let ReportKind::Cause(diagnostic) = &mut report.kind {
                diagnostic.labels.push(Label::secondary(
                    &self.typ_il.span,
                    format!("{} expects '{}'", self.subject(), self.typ_il.to_string()),
                ));
            }
            reports_pending.extend(&mut report.children);
        }
    }
}
