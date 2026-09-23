//! Expected-type diagnostics for notation slots and function signatures
//!
//! Checking sites identify the subject before expression elaboration.
//! Declaration spans stay separate from instantiated types so polymorphic calls
//! link argument failures to the parameter declaration, not the type argument.

use crate::{
    diagnostic::{Label, Report, ReportKind},
    lang::{
        common::{Id, source::Span},
        il::ast as il,
        traits::print::Print,
    },
};

use super::{ElabError, cause};

/// Identifies the declaration whose type constrains an expression.
pub(in crate::pass::elaborate) enum TypeSubject<'a> {
    /// Counts only notation argument slots, starting at zero.
    NotationArgument { idx: usize, id_rel: Option<&'a Id> },
    /// Counts function parameters, starting at zero.
    FunctionArgument { idx: usize, id_func: &'a Id },
    /// Names the function whose body is checked.
    FunctionReturn { id_func: &'a Id },
}

impl TypeSubject<'_> {
    /// Names the slot or result using the declaration owner.
    fn description(&self) -> String {
        match self {
            Self::NotationArgument { idx, id_rel } => {
                let subject = id_rel
                    .map_or_else(|| "notation".to_owned(), |id| format!("relation '{}'", id.node));
                format!("argument {idx} of {subject}")
            }
            Self::FunctionArgument { idx, id_func } => {
                format!("argument {idx} of function '${}'", id_func.node)
            }
            Self::FunctionReturn { id_func } => {
                format!("return value of function '${}'", id_func.node)
            }
        }
    }

    fn code(&self) -> &'static str {
        match self {
            Self::NotationArgument { .. } => "elab/notation-argument-type-mismatch",
            Self::FunctionArgument { .. } => "elab/function-argument-type-mismatch",
            Self::FunctionReturn { .. } => "elab/function-return-type-mismatch",
        }
    }
}

/// Retains an expected type and the declaration position that introduced it.
pub(in crate::pass::elaborate) struct ExpectedType<'a> {
    /// Identifies the argument or return value being checked.
    pub subject: TypeSubject<'a>,
    /// Holds the expected type after any call-site instantiation.
    pub typ_il: &'a il::Typ,
    /// Retains the source location before type substitution.
    pub span_declaration: &'a Span,
}

impl ExpectedType<'_> {
    /// Reports the actual type at the failed implicit cast.
    pub(in crate::pass::elaborate) fn type_mismatch(
        &self,
        typ_expect_il: &il::Typ,
        typ_infer_il: &il::Typ,
    ) -> ElabError {
        cause(
            self.subject.code(),
            format!(
                "{} expects '{}', but found '{}'",
                self.subject.description(),
                typ_expect_il.to_string(),
                typ_infer_il.to_string(),
            ),
            vec![Label::primary(&typ_infer_il.span, "")],
            Vec::new(),
        )
    }

    /// Links the existing cause to its declared type without wrapping it.
    pub(in crate::pass::elaborate) fn annotate(&self, reports: &mut [Report]) {
        // Add context to causes without classifying their codes or messages
        let mut reports_pending: Vec<_> = reports.iter_mut().collect();
        while let Some(report) = reports_pending.pop() {
            if let ReportKind::Cause(diagnostic) = &mut report.kind {
                let label = Label::secondary(
                    self.span_declaration,
                    format!("{} expects '{}'", self.subject.description(), self.typ_il.to_string()),
                );
                // Recursive calls can revisit the same declared parameter
                if !diagnostic.labels.contains(&label) {
                    diagnostic.labels.push(label);
                }
            }
            reports_pending.extend(&mut report.children);
        }
    }
}
