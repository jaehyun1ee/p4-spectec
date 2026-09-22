//! Function parameters, arguments, calls, and clause signature diagnostics
//!
//! Argument elaboration validates counts and kinds before signatures,
//! then uses these constructors with the offending use and declaration spans.

use crate::lang::common::{Id, source::Span};

use super::{ElabError, cause, primary, related};

const FUNCTION_PARAMETER_TYPE_PARAMETER_REPEATED: &str =
    "elab/function-parameter-type-parameter-repeated";

/// Reports a repeated type parameter inside a function parameter.
pub(in crate::pass::elaborate) fn function_parameter_type_parameter_repeated(
    tparam: &Id,
    span_previous: &Span,
) -> ElabError {
    let labels = vec![primary(&tparam.span), related(span_previous, "first declared here")];
    cause(
        FUNCTION_PARAMETER_TYPE_PARAMETER_REPEATED,
        format!("type parameter `{}` is repeated", tparam.node),
        labels,
        Vec::new(),
    )
}

const FUNCTION_ARGUMENT_NAME_MISMATCH: &str = "elab/function-argument-name-mismatch";

/// Reports a defining function argument whose name differs from its parameter.
pub(in crate::pass::elaborate) fn function_argument_name_mismatch(
    id_arg: &Id,
    id_param: &Id,
) -> ElabError {
    let labels =
        vec![primary(&id_arg.span), related(&id_param.span, "function parameter declared here")];
    cause(
        FUNCTION_ARGUMENT_NAME_MISMATCH,
        format!(
            "function argument `{}` must have the same name as declared function parameter `{}`",
            id_arg.node, id_param.node
        ),
        labels,
        vec![
            "A function argument in a definition clause binds the name declared by its function parameter."
                .to_owned(),
        ],
    )
}

const FUNCTION_ARGUMENT_TYPE_PARAMETER_ARITY_MISMATCH: &str =
    "elab/function-argument-type-parameter-arity-mismatch";
const FUNCTION_ARGUMENT_PARAMETER_ARITY_MISMATCH: &str =
    "elab/function-argument-parameter-arity-mismatch";
const FUNCTION_ARGUMENT_SIGNATURE_MISMATCH: &str = "elab/function-argument-signature-mismatch";

fn function_signature_labels(
    id_param: &Id,
    span_arg_declaration: Option<&Span>,
    span: &Span,
) -> Vec<crate::diagnostic::Label> {
    let mut labels =
        vec![primary(span), related(&id_param.span, "function parameter declared here")];
    if let Some(span_arg_declaration) = span_arg_declaration {
        labels.push(related(span_arg_declaration, "passed function declared here"));
    }
    labels
}

fn function_signature_note() -> Vec<String> {
    vec![
        "A passed function must have the same number of type parameters, parameter types, and return type as its function parameter."
            .to_owned(),
    ]
}

/// Reports a function argument with the wrong number of type parameters.
pub(in crate::pass::elaborate) fn function_argument_type_parameter_arity_mismatch(
    id_param: &Id,
    id_arg: &Id,
    expected: usize,
    actual: usize,
    span: &Span,
    span_arg_declaration: Option<&Span>,
) -> ElabError {
    let suffix = if expected == 1 { "" } else { "s" };
    cause(
        FUNCTION_ARGUMENT_TYPE_PARAMETER_ARITY_MISMATCH,
        format!(
            "function parameter `{}` has {expected} type parameter{suffix}, but passed function `{}` has {actual}",
            id_param.node, id_arg.node
        ),
        function_signature_labels(id_param, span_arg_declaration, span),
        function_signature_note(),
    )
}

/// Reports a function argument with the wrong number of parameters.
pub(in crate::pass::elaborate) fn function_argument_parameter_arity_mismatch(
    id_param: &Id,
    id_arg: &Id,
    expected: usize,
    actual: usize,
    span: &Span,
    span_arg_declaration: Option<&Span>,
) -> ElabError {
    let suffix = if expected == 1 { "" } else { "s" };
    cause(
        FUNCTION_ARGUMENT_PARAMETER_ARITY_MISMATCH,
        format!(
            "function parameter `{}` has {expected} parameter{suffix}, but passed function `{}` has {actual}",
            id_param.node, id_arg.node
        ),
        function_signature_labels(id_param, span_arg_declaration, span),
        function_signature_note(),
    )
}

/// Reports a function argument whose full signature is incompatible.
pub(in crate::pass::elaborate) fn function_argument_signature_mismatch(
    id_param: &Id,
    id_arg: &Id,
    span: &Span,
    span_arg_declaration: Option<&Span>,
) -> ElabError {
    cause(
        FUNCTION_ARGUMENT_SIGNATURE_MISMATCH,
        format!(
            "passed function `{}` must have the same signature as function parameter `{}`",
            id_arg.node, id_param.node
        ),
        function_signature_labels(id_param, span_arg_declaration, span),
        function_signature_note(),
    )
}

const FUNCTION_ARGUMENT_KIND_MISMATCH: &str = "elab/function-argument-kind-mismatch";

/// Reports an expression/function argument-kind mismatch.
pub(in crate::pass::elaborate) fn function_argument_kind_mismatch(
    expected: &str,
    actual: &str,
    span: &Span,
    span_param: &Span,
) -> ElabError {
    let labels = vec![primary(span), related(span_param, "parameter declared here")];
    cause(
        FUNCTION_ARGUMENT_KIND_MISMATCH,
        format!("expected {expected} argument, but got {actual} argument"),
        labels,
        Vec::new(),
    )
}

const FUNCTION_CALL_ARGUMENT_ARITY_MISMATCH: &str = "elab/function-call-argument-arity-mismatch";

/// Reports an argument list whose count differs from its parameter list.
pub(in crate::pass::elaborate) fn function_call_argument_arity_mismatch(
    id: Option<&Id>,
    expected: usize,
    actual: usize,
    span: &Span,
    span_declaration: Option<&Span>,
) -> ElabError {
    let suffix = if expected == 1 { "" } else { "s" };
    let message = match id {
        Some(id) => {
            format!("function `{}` expects {expected} argument{suffix}, but got {actual}", id.node)
        }
        None => format!("expected {expected} argument{suffix}, but got {actual}"),
    };
    let mut labels = vec![primary(span)];
    if let Some(span_declaration) = span_declaration {
        labels.push(related(span_declaration, "function declared here"));
    }
    cause(FUNCTION_CALL_ARGUMENT_ARITY_MISMATCH, message, labels, Vec::new())
}

const FUNCTION_CALL_TYPE_ARGUMENT_ARITY_MISMATCH: &str =
    "elab/function-call-type-argument-arity-mismatch";

/// Reports a call with the wrong number of explicit type arguments.
pub(in crate::pass::elaborate) fn function_call_type_argument_arity_mismatch(
    id: &Id,
    expected: usize,
    actual: usize,
    span: &Span,
    span_declaration: Option<&Span>,
) -> ElabError {
    let suffix = if expected == 1 { "" } else { "s" };
    let mut labels = vec![primary(span)];
    if let Some(span_declaration) = span_declaration {
        labels.push(related(span_declaration, "function declared here"));
    }
    cause(
        FUNCTION_CALL_TYPE_ARGUMENT_ARITY_MISMATCH,
        format!(
            "function `{}` expects {expected} type argument{suffix}, but got {actual}",
            id.node
        ),
        labels,
        Vec::new(),
    )
}

const FUNCTION_CLAUSE_ARGUMENT_ARITY_MISMATCH: &str =
    "elab/function-clause-argument-arity-mismatch";

/// Reports a function clause with the wrong number of arguments.
pub(in crate::pass::elaborate) fn function_clause_argument_arity_mismatch(
    id: &Id,
    expected: usize,
    actual: usize,
    span: &Span,
    span_declaration: &Span,
) -> ElabError {
    let suffix = if expected == 1 { "" } else { "s" };
    let labels = vec![primary(span), related(span_declaration, "function declared here")];
    cause(
        FUNCTION_CLAUSE_ARGUMENT_ARITY_MISMATCH,
        format!(
            "function `{}` was declared with {expected} parameter{suffix}, but this clause has {actual}",
            id.node
        ),
        labels,
        Vec::new(),
    )
}

const FUNCTION_CLAUSE_TYPE_PARAMETER_MISMATCH: &str =
    "elab/function-clause-type-parameter-mismatch";

/// Reports a clause whose type parameters differ from its declaration.
pub(in crate::pass::elaborate) fn function_clause_type_parameter_mismatch(
    id: &Id,
    expected: &[Id],
    actual: &[Id],
    span: &Span,
    span_declaration: &Span,
) -> ElabError {
    let describe = |tparams: &[Id]| {
        if tparams.is_empty() {
            "none".to_owned()
        } else {
            format!(
                "<{}>",
                tparams
                    .iter()
                    .map(|id| id.node.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
    };
    let labels =
        vec![primary(span), related(span_declaration, "expected type parameters declared here")];
    cause(
        FUNCTION_CLAUSE_TYPE_PARAMETER_MISMATCH,
        format!(
            "function `{}` was declared with type parameters {}, but this clause has type parameters {}",
            id.node,
            describe(expected),
            describe(actual)
        ),
        labels,
        vec![
            "A function clause must repeat the declared type parameters with the same names and order."
                .to_owned(),
        ],
    )
}
