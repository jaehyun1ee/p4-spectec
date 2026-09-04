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
        data::{
            typ,
            value::{Value, ValueKind, make},
        },
        il::ast::Typ,
    },
    phrase,
};

use super::{
    context::{Context, Location, Namespace, TypeId},
    extract,
};

// == Runtime value construction

thread_local! {
    static SHAPE_CACHE: RefCell<HashMap<Rc<str>, Rc<Mixop>>> = RefCell::new(HashMap::new());
}

fn named_type(name: &str) -> Typ {
    typ::make::var(
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
    values: Vec<Rc<Value>>,
    type_name: &str,
    left: Location,
    right: Location,
) -> Rc<Value> {
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
    left_value: Rc<Value>,
    operator_shape: &str,
    operator_left: Location,
    operator_right: Location,
    right_value: Rc<Value>,
    left: Location,
    right: Location,
) -> Rc<Value> {
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
    expression: Rc<Value>,
    left: Location,
    right: Location,
) -> Rc<Value> {
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

pub(crate) fn retag(value: Rc<Value>, type_name: &str) -> Rc<Value> {
    let typ = named_type(type_name);
    make::retag(value, &typ)
}

// == Shape inspection

pub(crate) fn matches<'value>(
    value: &'value Value,
    shape_text: &str,
) -> Option<Vec<&'value Rc<Value>>> {
    let ValueKind::Case(value_case) = &value.node else {
        return None;
    };
    let (actual, values) = value_case.split();
    let expected = shape(shape_text);
    (actual == *expected).then_some(values)
}

// == Parser context updates

pub(crate) fn declare_type_value(context: &Context, value: &Value, has_params: bool) {
    let id = extract::id_of_name(value).expect("P4 declaration name");
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
    let id = extract::id_of_name(value).expect("P4 declaration name");
    let type_id = match type_ref {
        Some(type_ref) => extract::tid_of_type_ref(type_ref).expect("P4 type reference"),
        None => TypeId::Empty,
    };
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

pub(crate) fn set_type_namespace(context: &Context, value: &Value, namespace: Namespace) {
    let id = extract::id_of_name(value).expect("P4 type name");
    context.set_type_namespace(&id, namespace);
}
