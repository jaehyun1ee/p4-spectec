//! Expression inference, checking, and hint-only syntax diagnostics
//!
//! Expression attempts create these terminal causes and keep them beneath
//! uncoded attempt frames until one alternative succeeds or elaboration finishes.

use crate::diagnostic::Label;
use crate::lang::common::source::Span;

use super::{ElabError, cause};

const EXPRESSION_INFERENCE_INVALID: &str = "elab/expression-inference-invalid";

/// Reports a construct whose type cannot be inferred without context.
pub(in crate::pass::elaborate) fn expression_inference_invalid(
    span: &Span,
    description: &str,
) -> ElabError {
    cause(
        EXPRESSION_INFERENCE_INVALID,
        format!("cannot infer type of {description}"),
        vec![Label::primary(span, "")],
        Vec::new(),
    )
}

const OPERATOR_OPERAND_TYPE_MISMATCH: &str = "elab/operator-operand-type-mismatch";

/// Reports an operator with unsupported operand types.
pub(in crate::pass::elaborate) fn operator_operand_type_mismatch(span: &Span) -> ElabError {
    cause(
        OPERATOR_OPERAND_TYPE_MISMATCH,
        "operator is not defined for the operand types",
        vec![Label::primary(span, "")],
        Vec::new(),
    )
}

const EXPRESSION_TYPE_MISMATCH: &str = "elab/expression-type-mismatch";

/// Reports an expression that does not satisfy its expected type relationship.
pub(in crate::pass::elaborate) fn expression_type_mismatch(
    span: &Span,
    message: impl Into<String>,
) -> ElabError {
    cause(EXPRESSION_TYPE_MISMATCH, message, vec![Label::primary(span, "")], Vec::new())
}

const EXPRESSION_CAST_INVALID: &str = "elab/expression-cast-invalid";

/// Reports a failed implicit cast from an inferred to an expected type.
pub(in crate::pass::elaborate) fn expression_cast_invalid(span: &Span) -> ElabError {
    cause(
        EXPRESSION_CAST_INVALID,
        "cannot cast inferred expression to expected type",
        vec![Label::primary(span, "")],
        Vec::new(),
    )
}

const EXPRESSION_ITERATION_MISMATCH: &str = "elab/expression-iteration-mismatch";

/// Reports an expression whose iteration differs from the expected iteration.
pub(in crate::pass::elaborate) fn expression_iteration_mismatch(
    span: &Span,
    message: impl Into<String>,
) -> ElabError {
    cause(EXPRESSION_ITERATION_MISMATCH, message, vec![Label::primary(span, "")], Vec::new())
}

const EXPRESSION_ARITY_MISMATCH: &str = "elab/expression-arity-mismatch";

/// Reports a tuple, struct, or notation expression with the wrong arity.
pub(in crate::pass::elaborate) fn expression_arity_mismatch(
    span: &Span,
    message: impl Into<String>,
) -> ElabError {
    cause(EXPRESSION_ARITY_MISMATCH, message, vec![Label::primary(span, "")], Vec::new())
}

const VARIANT_EXPRESSION_MATCH_REPEATED: &str = "elab/variant-expression-match-repeated";

/// Reports an expression that matches more than one variant case.
pub(in crate::pass::elaborate) fn variant_expression_match_repeated(span: &Span) -> ElabError {
    cause(
        VARIANT_EXPRESSION_MATCH_REPEATED,
        "expression matches multiple variant cases",
        vec![Label::primary(span, "")],
        Vec::new(),
    )
}

const HOLE_OUTSIDE_HINT_UNSUPPORTED: &str = "elab/hole-outside-hint-unsupported";
const FUSE_OUTSIDE_HINT_UNSUPPORTED: &str = "elab/fuse-outside-hint-unsupported";
const UNPAREN_OUTSIDE_HINT_UNSUPPORTED: &str = "elab/unparen-outside-hint-unsupported";
const LATEX_OUTSIDE_HINT_UNSUPPORTED: &str = "elab/latex-outside-hint-unsupported";

/// Reports a hole expression outside a hint.
pub(in crate::pass::elaborate) fn hole_outside_hint_unsupported(span: &Span) -> ElabError {
    cause(
        HOLE_OUTSIDE_HINT_UNSUPPORTED,
        "hole is not allowed outside a hint",
        vec![Label::primary(span, "")],
        vec![
            concat!(
                "A `%`, `%N`, `%%`, or `!%` marks an argument slot inside a ",
                "`hint(...)` expression.",
            )
            .to_owned(),
        ],
    )
}

/// Reports token concatenation outside a hint.
pub(in crate::pass::elaborate) fn fuse_outside_hint_unsupported(span: &Span) -> ElabError {
    cause(
        FUSE_OUTSIDE_HINT_UNSUPPORTED,
        "token concatenation `#` is not allowed outside a hint",
        vec![Label::primary(span, "")],
        vec!["The `#` operator joins rendered hint fragments without a space.".to_owned()],
    )
}

/// Reports unparenthesizing outside a hint.
pub(in crate::pass::elaborate) fn unparen_outside_hint_unsupported(span: &Span) -> ElabError {
    cause(
        UNPAREN_OUTSIDE_HINT_UNSUPPORTED,
        "unparenthesizing operator `##` is not allowed outside a hint",
        vec![Label::primary(span, "")],
        vec!["The `##` operator strips parentheses only while rendering a hint.".to_owned()],
    )
}

/// Reports a LaTeX literal outside a hint.
pub(in crate::pass::elaborate) fn latex_outside_hint_unsupported(span: &Span) -> ElabError {
    cause(
        LATEX_OUTSIDE_HINT_UNSUPPORTED,
        "LaTeX literals are not allowed outside a hint",
        vec![Label::primary(span, "")],
        vec!["A `%latex(\"...\")` literal embeds raw LaTeX only inside a hint.".to_owned()],
    )
}
