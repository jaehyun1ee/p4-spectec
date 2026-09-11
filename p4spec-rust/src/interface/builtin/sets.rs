//! Set builtins backed by a collection with actual set semantics.
//!
//! Runtime syntax is decoded into a sorted list, operated on, and encoded back
//! in semantic value order. For example, the union of `{a}` and `{a, b}` is
//! emitted once as `{a, b}`.

use std::rc::Rc;

use crate::lang::{
    common::{
        notation::mixop::{Mixop, shape},
        source::Span,
    },
    data::{
        typ,
        value::{Value, ValueArena, get, make},
    },
    il::ast::Typ,
    traits::{cmp::SyntaxCmp, eq::SyntaxEq},
};

use super::{BuiltinError, extract};

// == Value set

type ValueSet = Vec<Value>;

fn sort_set(arena: &ValueArena, set: &mut ValueSet) {
    set.sort_by(|value_a, value_b| arena.view(*value_a).syntax_cmp(&arena.view(*value_b)));
    set.dedup_by(|value_a, value_b| arena.view(*value_a).syntax_eq(&arena.view(*value_b)));
}

fn contains(arena: &ValueArena, set: &[Value], value: &Value) -> bool {
    set.binary_search_by(|value_element| arena.view(*value_element).syntax_cmp(&arena.view(*value)))
        .is_ok()
}

// == Conversion between meta-sets and runtime lists

fn set_mixop() -> Rc<Mixop> {
    shape("`{ k `}")
}

fn set_of_value(arena: &ValueArena, value: &Value) -> Result<ValueSet, BuiltinError> {
    let value_case = get::case(arena, value).map_err(|_| BuiltinError::new("expected a set"))?;
    let set_mixop = set_mixop();
    if !value_case.eq_shape(&set_mixop) {
        return Err(BuiltinError::new("expected a set"));
    }
    let args = value_case.args();
    let value_elements = extract::one(&args)?;
    let values =
        get::list(arena, value_elements).map_err(|_| BuiltinError::new("expected a set"))?;
    let mut set = values.to_vec();
    sort_set(arena, &mut set);
    Ok(set)
}

fn value_of_set(
    arena: &mut ValueArena,
    typ_key: &Typ,
    set: ValueSet,
) -> Result<Value, BuiltinError> {
    let values_element = set.into_iter().collect();
    let typ_list = typ::make::list(typ_key.clone());
    let value_elements = make::list(arena, typ_list.node.into(), values_element, Span::default())?;
    let set_id = crate::phrase!(node: "set".to_owned(), span: Span::default());
    let typ = typ::make::var(set_id, vec![typ_key.clone()]);
    let set_mixop = set_mixop();
    let value_case =
        Mixop::fill(&set_mixop, [value_elements]).expect("the set mixop has exactly one argument");
    let value = make::case(arena, typ.node.into(), value_case, Span::default())?;
    Ok(value)
}

// == Built-in implementations

// dec $intersect_set<K>(set<K>, set<K>) : set<K>

pub fn intersect_set(
    arena: &mut ValueArena,
    targs: &[Typ],
    values: &[Value],
) -> Result<Value, BuiltinError> {
    let typ_key = extract::one(targs)?;
    let (value_set_a, value_set_b) = extract::two(values)?;
    let set_a = set_of_value(arena, value_set_a)?;
    let set_b = set_of_value(arena, value_set_b)?;
    let intersection = set_a
        .into_iter()
        .filter(|value| contains(arena, &set_b, value))
        .collect();
    value_of_set(arena, typ_key, intersection)
}

// dec $union_set<K>(set<K>, set<K>) : set<K>

pub fn union_set(
    arena: &mut ValueArena,
    targs: &[Typ],
    values: &[Value],
) -> Result<Value, BuiltinError> {
    let typ_key = extract::one(targs)?;
    let (value_set_a, value_set_b) = extract::two(values)?;
    let set_a = set_of_value(arena, value_set_a)?;
    let set_b = set_of_value(arena, value_set_b)?;
    let mut union = set_a;
    union.extend(set_b);
    sort_set(arena, &mut union);
    value_of_set(arena, typ_key, union)
}

// dec $unions_set<K>(set<K>*) : set<K>

pub fn unions_set(
    arena: &mut ValueArena,
    targs: &[Typ],
    values: &[Value],
) -> Result<Value, BuiltinError> {
    let typ_key = extract::one(targs)?;
    let value_sets = extract::one(values)?;
    let values =
        get::list(arena, value_sets).map_err(|error| BuiltinError::new(error.to_string()))?;
    let mut union = ValueSet::new();
    for value in values {
        let set = set_of_value(arena, value)?;
        union.extend(set);
    }
    sort_set(arena, &mut union);
    value_of_set(arena, typ_key, union)
}

// dec $diff_set<K>(set<K>, set<K>) : set<K>

pub fn diff_set(
    arena: &mut ValueArena,
    targs: &[Typ],
    values: &[Value],
) -> Result<Value, BuiltinError> {
    let typ_key = extract::one(targs)?;
    let (value_set_a, value_set_b) = extract::two(values)?;
    let set_a = set_of_value(arena, value_set_a)?;
    let set_b = set_of_value(arena, value_set_b)?;
    let difference = set_a
        .into_iter()
        .filter(|value| !contains(arena, &set_b, value))
        .collect();
    value_of_set(arena, typ_key, difference)
}

// dec $sub_set<K>(set<K>, set<K>) : bool

pub fn sub_set(
    arena: &mut ValueArena,
    targs: &[Typ],
    values: &[Value],
) -> Result<Value, BuiltinError> {
    let _typ_key = extract::one(targs)?;
    let (value_set_a, value_set_b) = extract::two(values)?;
    let set_a = set_of_value(arena, value_set_a)?;
    let set_b = set_of_value(arena, value_set_b)?;
    let is_subset = set_a.iter().all(|value| contains(arena, &set_b, value));
    let value = make::bool(arena, is_subset, Span::default())?;
    Ok(value)
}

// dec $eq_set<K>(set<K>, set<K>) : bool

pub fn eq_set(
    arena: &mut ValueArena,
    targs: &[Typ],
    values: &[Value],
) -> Result<Value, BuiltinError> {
    let _typ_key = extract::one(targs)?;
    let (value_set_a, value_set_b) = extract::two(values)?;
    let set_a = set_of_value(arena, value_set_a)?;
    let set_b = set_of_value(arena, value_set_b)?;
    let equal = set_a.len() == set_b.len()
        && set_a
            .iter()
            .zip(&set_b)
            .all(|(value_a, value_b)| arena.view(*value_a).syntax_eq(&arena.view(*value_b)));
    let value = make::bool(arena, equal, Span::default())?;
    Ok(value)
}
