use std::{collections::BTreeSet, rc::Rc};

use crate::{
    lang::{
        common::{
            notation::{atom::Atom, mixfix::Mixfix, mixop::Mixop},
            source::Span,
        },
        il::ast::Typ,
    },
    runtime::{
        types::typ as make_type,
        value::{Value, ValueRef, get, make},
    },
};

use super::{BuiltinError, BuiltinResult, extract, return_value};

// Value set

type ValueSet = BTreeSet<ValueRef>;

// Conversion between meta-sets and OCaml lists

fn set_mixop() -> Mixop {
    Mixfix::Brack(
        crate::phrase!(node: Atom::LBrace, span: Span::default()),
        Box::new(Mixfix::Arg(())),
        crate::phrase!(node: Atom::RBrace, span: Span::default()),
    )
}

fn set_of_value(span: &Span, value: &Value) -> Result<ValueSet, BuiltinError> {
    let value_case =
        get::case(value).map_err(|_| BuiltinError::new(span.clone(), "expected a set"))?;
    if value_case.split().0 != set_mixop() {
        return Err(BuiltinError::new(span.clone(), "expected a set"));
    }
    let args = value_case.args();
    let value_elements = extract::one(span, &args)?;
    let values =
        get::list(value_elements).map_err(|_| BuiltinError::new(span.clone(), "expected a set"))?;
    Ok(values.iter().cloned().collect())
}

fn value_of_set(add: &mut dyn FnMut(ValueRef), typ_key: &Typ, set: ValueSet) -> BuiltinResult {
    let values_element = set.into_iter().collect();
    let typ_list = make_type::list(typ_key.clone());
    let value_elements = make::list(&typ_list, values_element, Span::default());
    add(Rc::clone(&value_elements));
    let typ = make_type::var(
        crate::phrase!(node: "set".to_owned(), span: Span::default()),
        vec![typ_key.clone()],
    );
    let value_case = Mixop::fill(&set_mixop(), [value_elements])
        .expect("the set mixop has exactly one argument");
    return_value(add, make::case(&typ, value_case, Span::default()))
}

// dec $intersect_set<K>(set<K>, set<K>) : set<K>

pub fn intersect_set(
    add: &mut dyn FnMut(ValueRef),
    span: &Span,
    type_args: &[Typ],
    values: &[ValueRef],
) -> BuiltinResult {
    let typ_key = extract::one(span, type_args)?;
    let (value_set_a, value_set_b) = extract::two(span, values)?;
    let set_a = set_of_value(span, value_set_a)?;
    let set_b = set_of_value(span, value_set_b)?;
    value_of_set(add, typ_key, set_a.intersection(&set_b).cloned().collect())
}

// dec $union_set<K>(set<K>, set<K>) : set<K>

pub fn union_set(
    add: &mut dyn FnMut(ValueRef),
    span: &Span,
    type_args: &[Typ],
    values: &[ValueRef],
) -> BuiltinResult {
    let typ_key = extract::one(span, type_args)?;
    let (value_set_a, value_set_b) = extract::two(span, values)?;
    let set_a = set_of_value(span, value_set_a)?;
    let set_b = set_of_value(span, value_set_b)?;
    value_of_set(add, typ_key, set_a.union(&set_b).cloned().collect())
}

// dec $unions_set<K>(set<K>* ) : set<K>

pub fn unions_set(
    add: &mut dyn FnMut(ValueRef),
    span: &Span,
    type_args: &[Typ],
    values: &[ValueRef],
) -> BuiltinResult {
    let typ_key = extract::one(span, type_args)?;
    let value_sets = extract::one(span, values)?;
    let values = get::list(value_sets)
        .map_err(|error| BuiltinError::new(span.clone(), error.to_string()))?;
    let mut union = ValueSet::new();
    for value in values {
        union.extend(set_of_value(span, value)?);
    }
    value_of_set(add, typ_key, union)
}

// dec $diff_set<K>(set<K>, set<K>) : set<K>

pub fn diff_set(
    add: &mut dyn FnMut(ValueRef),
    span: &Span,
    type_args: &[Typ],
    values: &[ValueRef],
) -> BuiltinResult {
    let typ_key = extract::one(span, type_args)?;
    let (value_set_a, value_set_b) = extract::two(span, values)?;
    let set_a = set_of_value(span, value_set_a)?;
    let set_b = set_of_value(span, value_set_b)?;
    value_of_set(add, typ_key, set_a.difference(&set_b).cloned().collect())
}

// dec $sub_set<K>(set<K>, set<K>) : bool

pub fn sub_set(
    add: &mut dyn FnMut(ValueRef),
    span: &Span,
    type_args: &[Typ],
    values: &[ValueRef],
) -> BuiltinResult {
    let _typ_key = extract::one(span, type_args)?;
    let (value_set_a, value_set_b) = extract::two(span, values)?;
    let set_a = set_of_value(span, value_set_a)?;
    let set_b = set_of_value(span, value_set_b)?;
    return_value(add, make::bool(set_a.is_subset(&set_b), Span::default()))
}

// dec $eq_set<K>(set<K>, set<K>) : bool

pub fn eq_set(
    add: &mut dyn FnMut(ValueRef),
    span: &Span,
    type_args: &[Typ],
    values: &[ValueRef],
) -> BuiltinResult {
    let _typ_key = extract::one(span, type_args)?;
    let (value_set_a, value_set_b) = extract::two(span, values)?;
    let set_a = set_of_value(span, value_set_a)?;
    let set_b = set_of_value(span, value_set_b)?;
    return_value(add, make::bool(set_a == set_b, Span::default()))
}
