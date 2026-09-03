//! Set builtins backed by a collection with actual set semantics.
//!
//! Runtime syntax is decoded into a `BTreeSet`, operated on, and encoded back
//! in semantic value order. For example, the union of `{a}` and `{a, b}` is
//! emitted once as `{a, b}`.

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
        types::typ,
        value::{Value, get, make},
    },
};

use super::{BuiltinError, extract};

// == Value set

type ValueSet = BTreeSet<Rc<Value>>;

// == Conversion between meta-sets and OCaml lists

fn set_mixop() -> Mixop {
    let left = crate::phrase!(node: Atom::LBrace, span: Span::default());
    let right = crate::phrase!(node: Atom::RBrace, span: Span::default());
    Mixfix::Brack(left, Box::new(Mixfix::Arg(())), right)
}

fn set_of_value(value: &Value) -> Result<ValueSet, BuiltinError> {
    let value_case = get::case(value).map_err(|_| BuiltinError::new("expected a set"))?;
    let set_mixop = set_mixop();
    if value_case.split().0 != set_mixop {
        return Err(BuiltinError::new("expected a set"));
    }
    let args = value_case.args();
    let value_elements = extract::one(&args)?;
    let values = get::list(value_elements).map_err(|_| BuiltinError::new("expected a set"))?;
    Ok(values.iter().cloned().collect())
}

fn value_of_set(typ_key: &Typ, set: ValueSet) -> Result<Rc<Value>, BuiltinError> {
    let values_element = set.into_iter().collect();
    let typ_list = typ::list(typ_key.clone());
    let value_elements = make::list(&typ_list, values_element, Span::default());
    let set_id = crate::phrase!(node: "set".to_owned(), span: Span::default());
    let typ = typ::var(set_id, vec![typ_key.clone()]);
    let set_mixop = set_mixop();
    let value_case =
        Mixop::fill(&set_mixop, [value_elements]).expect("the set mixop has exactly one argument");
    let value = make::case(&typ, value_case, Span::default());
    Ok(value)
}

// == Built-in implementations

// dec $intersect_set<K>(set<K>, set<K>) : set<K>

pub fn intersect_set(targs: &[Typ], values: &[Rc<Value>]) -> Result<Rc<Value>, BuiltinError> {
    let typ_key = extract::one(targs)?;
    let (value_set_a, value_set_b) = extract::two(values)?;
    let set_a = set_of_value(value_set_a)?;
    let set_b = set_of_value(value_set_b)?;
    let intersection = set_a.intersection(&set_b).cloned().collect();
    value_of_set(typ_key, intersection)
}

// dec $union_set<K>(set<K>, set<K>) : set<K>

pub fn union_set(targs: &[Typ], values: &[Rc<Value>]) -> Result<Rc<Value>, BuiltinError> {
    let typ_key = extract::one(targs)?;
    let (value_set_a, value_set_b) = extract::two(values)?;
    let set_a = set_of_value(value_set_a)?;
    let set_b = set_of_value(value_set_b)?;
    let union = set_a.union(&set_b).cloned().collect();
    value_of_set(typ_key, union)
}

// dec $unions_set<K>(set<K>*) : set<K>

pub fn unions_set(targs: &[Typ], values: &[Rc<Value>]) -> Result<Rc<Value>, BuiltinError> {
    let typ_key = extract::one(targs)?;
    let value_sets = extract::one(values)?;
    let values = get::list(value_sets).map_err(|error| BuiltinError::new(error.to_string()))?;
    let mut union = ValueSet::new();
    for value in values {
        let set = set_of_value(value)?;
        union.extend(set);
    }
    value_of_set(typ_key, union)
}

// dec $diff_set<K>(set<K>, set<K>) : set<K>

pub fn diff_set(targs: &[Typ], values: &[Rc<Value>]) -> Result<Rc<Value>, BuiltinError> {
    let typ_key = extract::one(targs)?;
    let (value_set_a, value_set_b) = extract::two(values)?;
    let set_a = set_of_value(value_set_a)?;
    let set_b = set_of_value(value_set_b)?;
    let difference = set_a.difference(&set_b).cloned().collect();
    value_of_set(typ_key, difference)
}

// dec $sub_set<K>(set<K>, set<K>) : bool

pub fn sub_set(targs: &[Typ], values: &[Rc<Value>]) -> Result<Rc<Value>, BuiltinError> {
    let _typ_key = extract::one(targs)?;
    let (value_set_a, value_set_b) = extract::two(values)?;
    let set_a = set_of_value(value_set_a)?;
    let set_b = set_of_value(value_set_b)?;
    let is_subset = set_a.is_subset(&set_b);
    let value = make::bool(is_subset, Span::default());
    Ok(value)
}

// dec $eq_set<K>(set<K>, set<K>) : bool

pub fn eq_set(targs: &[Typ], values: &[Rc<Value>]) -> Result<Rc<Value>, BuiltinError> {
    let _typ_key = extract::one(targs)?;
    let (value_set_a, value_set_b) = extract::two(values)?;
    let set_a = set_of_value(value_set_a)?;
    let set_b = set_of_value(value_set_b)?;
    let equal = set_a == set_b;
    let value = make::bool(equal, Span::default());
    Ok(value)
}
