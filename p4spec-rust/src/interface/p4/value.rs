//! Runtime-value constructors and semantic-action helpers for the P4 grammar.
//!
//! Grammar actions fill a cached SpecTec mixfix shape, attach its declared P4
//! type and span, then update the parser's name context where necessary. For
//! example, a binary expression first constructs its operator value and then
//! fills `expression binop expression` with the two operands.

use std::{cell::RefCell, collections::HashMap, rc::Rc};

use crate::{
    frontend,
    lang::{
        common::{notation::mixop::Mixop, source::Span},
        il::ast::Typ,
    },
    phrase,
    runtime::{
        types::typ,
        value::{Value, ValueKind, ValueRef, get, make},
    },
};

use super::context::{Context, Location, Namespace, TypeId};

// == Runtime value construction

thread_local! {
    static SHAPE_CACHE: RefCell<HashMap<Rc<str>, Rc<Mixop>>> = RefCell::new(HashMap::new());
}

fn named_type(name: &str) -> Typ {
    typ::var(
        phrase! { node: name.to_owned(), span: Span::default() },
        Vec::new(),
    )
}

fn shape(shape_text: &str) -> Rc<Mixop> {
    SHAPE_CACHE.with(|cache| {
        if let Some(mixop) = cache.borrow().get(shape_text).cloned() {
            return mixop;
        }
        let mixop = frontend::parse::parse_mixop(shape_text)
            .expect("P4 grammar contains a valid SpecTec mixop");
        let mixop = Rc::new(mixop);
        cache
            .borrow_mut()
            .insert(Rc::from(shape_text), Rc::clone(&mixop));
        mixop
    })
}

pub(crate) fn case_value(
    context: &Context,
    shape_text: &str,
    values: Vec<ValueRef>,
    type_name: &str,
    left: Location,
    right: Location,
) -> ValueRef {
    let mixop = shape(shape_text);
    let value_case = Mixop::fill(mixop.as_ref(), values)
        .expect("P4 grammar mixop arity matches its semantic action");
    let typ = named_type(type_name);
    let span = context.span(left, right);
    make::case(&typ, value_case, span)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn binary_value(
    context: &Context,
    left_value: ValueRef,
    operator_shape: &str,
    operator_left: Location,
    operator_right: Location,
    right_value: ValueRef,
    left: Location,
    right: Location,
) -> ValueRef {
    let operator = case_value(
        context,
        operator_shape,
        Vec::new(),
        "binop",
        operator_left,
        operator_right,
    );
    case_value(
        context,
        "expression binop expression",
        vec![left_value, operator, right_value],
        "binaryExpression",
        left,
        right,
    )
}

pub(crate) fn unary_value(
    context: &Context,
    operator_shape: &str,
    operator_left: Location,
    operator_right: Location,
    expression: ValueRef,
    left: Location,
    right: Location,
) -> ValueRef {
    let operator = case_value(
        context,
        operator_shape,
        Vec::new(),
        "unop",
        operator_left,
        operator_right,
    );
    case_value(
        context,
        "unop expression",
        vec![operator, expression],
        "unaryExpression",
        left,
        right,
    )
}

pub(crate) fn retag(value: ValueRef, type_name: &str) -> ValueRef {
    let typ = named_type(type_name);
    make::retag(value, &typ)
}

// == Shape inspection

pub(crate) fn matches<'value>(
    value: &'value Value,
    shape_text: &str,
) -> Option<Vec<&'value ValueRef>> {
    let ValueKind::Case(value_case) = &value.node else {
        return None;
    };
    let (actual, values) = value_case.split();
    let expected = shape(shape_text);
    (actual == *expected).then_some(values)
}

pub fn id_of_name(value: &Value) -> Option<String> {
    for (shape_text, spelling) in [
        ("APPLY", "apply"),
        ("KEY", "key"),
        ("ACTIONS", "actions"),
        ("STATE", "state"),
        ("ENTRIES", "entries"),
        ("TYPE", "type"),
        ("PRIORITY", "priority"),
        ("LIST", "list"),
    ] {
        if matches(value, shape_text).is_some() {
            return Some(spelling.to_owned());
        }
    }
    for shape_text in ["_ID text", "_TID text"] {
        if let Some(values) = matches(value, shape_text) {
            return get::text(values[0]).ok().map(str::to_owned);
        }
    }
    None
}

pub(crate) fn value_at(value: &Value, shape_text: &str, index: usize) -> Option<ValueRef> {
    matches(value, shape_text).and_then(|values| values.get(index).map(|value| Rc::clone(value)))
}

// == Parser context updates

pub(crate) fn declare_type_value(context: &Context, value: &Value, has_params: bool) {
    let id = id_of_name(value).expect("P4 declaration name");
    context
        .declare_type(id, has_params)
        .expect("P4 parser scope");
}

pub(crate) fn declare_var_value(
    context: &Context,
    value: &Value,
    has_params: bool,
    type_ref: Option<&Value>,
) {
    let id = id_of_name(value).expect("P4 declaration name");
    let type_id = type_ref.map_or(TypeId::Empty, type_id_of_ref);
    context
        .declare_var(id, has_params, type_id)
        .expect("P4 parser scope");
}

pub(crate) fn declare_names_as_vars(context: &Context, value: &Value) {
    if let Some(values) = matches(value, "nameList ',' name") {
        declare_names_as_vars(context, values[0]);
        declare_var_value(context, values[1], false, None);
    } else {
        declare_var_value(context, value, false, None);
    }
}

pub(crate) fn declare_names_as_types(context: &Context, value: &Value) {
    if let Some(values) = matches(value, "typeParameterList ',' typeParameter") {
        declare_names_as_types(context, values[0]);
        declare_type_value(context, values[1], false);
    } else {
        declare_type_value(context, value, false);
    }
}

pub(crate) fn type_id_of_ref(value: &Value) -> TypeId {
    if let Some(values) = matches(value, "_TID text") {
        return get::text(values[0])
            .map(|id| TypeId::Local(id.to_owned()))
            .unwrap_or(TypeId::Empty);
    }
    if let Some(values) = matches(value, "_TID '.' typeName") {
        return match type_id_of_ref(values[0]) {
            TypeId::Local(id) => TypeId::Global(id),
            _ => TypeId::Empty,
        };
    }
    if let Some(values) = matches(value, "prefixedTypeName `< typeArgumentList `>") {
        return type_id_of_ref(values[0]);
    }
    TypeId::Empty
}

fn has_type_parameters(value: &Value) -> bool {
    matches(value, "`< typeParameterList `>").is_some()
}

pub(crate) fn declaration_name(value: &Value) -> Option<ValueRef> {
    let cases = [
        ("annotationList CONST type name initializer ';'", 2),
        ("annotationList type `( argumentList `) name ';'", 3),
        (
            "annotationList type `( argumentList `) name objectInitializer ';'",
            3,
        ),
        (
            "annotationList ACTION name `( parameterList `) blockStatement",
            1,
        ),
        (
            "annotationList EXTERN nonTypeName typeParameterListOpt `{ externConstructorOrMethodPrototypeList `}",
            1,
        ),
        (
            "annotationList PARSER name typeParameterListOpt `( parameterList `) constructorParameterListOpt `{ parserLocalDeclarationList parserStateList `}",
            1,
        ),
        (
            "annotationList CONTROL name typeParameterListOpt `( parameterList `) constructorParameterListOpt `{ controlLocalDeclarationList APPLY controlBody `}",
            1,
        ),
        (
            "annotationList ENUM name `{ nameList trailingCommaOpt `}",
            1,
        ),
        (
            "annotationList ENUM type name `{ namedExpressionList trailingCommaOpt `}",
            2,
        ),
        (
            "annotationList STRUCT name typeParameterListOpt `{ typeFieldList `}",
            1,
        ),
        (
            "annotationList HEADER name typeParameterListOpt `{ typeFieldList `}",
            1,
        ),
        (
            "annotationList HEADER_UNION name typeParameterListOpt `{ typeFieldList `}",
            1,
        ),
        ("annotationList TYPEDEF typedef name ';'", 2),
        ("annotationList TYPE type name ';'", 2),
        (
            "annotationList PARSER name typeParameterListOpt `( parameterList `) ';'",
            1,
        ),
        (
            "annotationList CONTROL name typeParameterListOpt `( parameterList `) ';'",
            1,
        ),
        (
            "annotationList PACKAGE name typeParameterListOpt `( parameterList `) ';'",
            1,
        ),
        ("annotationList TABLE name `{ tablePropertyList `}", 1),
    ];
    for (shape_text, index) in cases {
        if let Some(value) = value_at(value, shape_text, index) {
            return Some(value);
        }
    }
    for shape_text in [
        "annotationList functionPrototype blockStatement",
        "annotationList EXTERN functionPrototype ';'",
    ] {
        if let Some(prototype) = value_at(value, shape_text, 1) {
            return value_at(
                &prototype,
                "typeOrVoid name typeParameterListOpt `( parameterList `)",
                1,
            );
        }
    }
    None
}

pub(crate) fn declaration_id(value: &Value) -> String {
    declaration_name(value)
        .as_deref()
        .and_then(id_of_name)
        .expect("known P4 declaration shape")
}

pub(crate) fn declaration_has_type_parameters(value: &Value) -> bool {
    for (shape_text, index) in [
        (
            "annotationList EXTERN nonTypeName typeParameterListOpt `{ externConstructorOrMethodPrototypeList `}",
            2,
        ),
        (
            "annotationList PARSER name typeParameterListOpt `( parameterList `) constructorParameterListOpt `{ parserLocalDeclarationList parserStateList `}",
            2,
        ),
        (
            "annotationList CONTROL name typeParameterListOpt `( parameterList `) constructorParameterListOpt `{ controlLocalDeclarationList APPLY controlBody `}",
            2,
        ),
        (
            "annotationList STRUCT name typeParameterListOpt `{ typeFieldList `}",
            2,
        ),
        (
            "annotationList HEADER name typeParameterListOpt `{ typeFieldList `}",
            2,
        ),
        (
            "annotationList HEADER_UNION name typeParameterListOpt `{ typeFieldList `}",
            2,
        ),
        (
            "annotationList PARSER name typeParameterListOpt `( parameterList `) ';'",
            2,
        ),
        (
            "annotationList CONTROL name typeParameterListOpt `( parameterList `) ';'",
            2,
        ),
        (
            "annotationList PACKAGE name typeParameterListOpt `( parameterList `) ';'",
            2,
        ),
    ] {
        if let Some(parameters) = value_at(value, shape_text, index) {
            return has_type_parameters(&parameters);
        }
    }
    for shape_text in [
        "annotationList functionPrototype blockStatement",
        "annotationList EXTERN functionPrototype ';'",
    ] {
        if let Some(prototype) = value_at(value, shape_text, 1)
            && let Some(parameters) = value_at(
                &prototype,
                "typeOrVoid name typeParameterListOpt `( parameterList `)",
                2,
            )
        {
            return has_type_parameters(&parameters);
        }
    }
    false
}

pub(crate) fn declaration_type_id(value: &Value) -> TypeId {
    for (shape_text, index) in [
        ("annotationList CONST type name initializer ';'", 1),
        ("annotationList type `( argumentList `) name ';'", 1),
        (
            "annotationList type `( argumentList `) name objectInitializer ';'",
            1,
        ),
    ] {
        if let Some(type_ref) = value_at(value, shape_text, index) {
            return type_id_of_ref(&type_ref);
        }
    }
    TypeId::Empty
}

pub(crate) fn function_prototype_id(value: &Value) -> String {
    value_at(
        value,
        "typeOrVoid name typeParameterListOpt `( parameterList `)",
        1,
    )
    .as_deref()
    .and_then(id_of_name)
    .expect("P4 function prototype")
}

pub(crate) fn function_prototype_has_type_parameters(value: &Value) -> bool {
    value_at(
        value,
        "typeOrVoid name typeParameterListOpt `( parameterList `)",
        2,
    )
    .is_some_and(|value| has_type_parameters(&value))
}

pub(crate) fn set_type_namespace(context: &Context, value: &Value, namespace: Namespace) {
    let id = id_of_name(value).expect("P4 type name");
    context.set_type_namespace(&id, namespace);
}

pub(crate) fn declare_declaration_type(context: &Context, value: &Value) {
    context
        .declare_type(
            declaration_id(value),
            declaration_has_type_parameters(value),
        )
        .expect("P4 parser scope");
}

pub(crate) fn declare_declaration_var(context: &Context, value: &Value) {
    context
        .declare_var(
            declaration_id(value),
            declaration_has_type_parameters(value),
            declaration_type_id(value),
        )
        .expect("P4 parser scope");
}

pub(crate) fn declare_prototype_var(context: &Context, value: &Value) {
    context
        .declare_var(
            function_prototype_id(value),
            function_prototype_has_type_parameters(value),
            TypeId::Empty,
        )
        .expect("P4 parser scope");
}
