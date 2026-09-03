//! Map builtins represented by the specification's ordered pair list.
//!
//! A map value is decoded to its pair list, updated at the first matching key,
//! and encoded again. Thus updating `{a: 1}` with `a: 2` preserves its list
//! position and produces `{a: 2}`, as in the OCaml implementation.

use std::rc::Rc;

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

// == Value map

type ValueMap = Vec<ValueRef>;

fn pair_mixop() -> Mixop {
    let colon = crate::phrase!(node: Atom::Operator(":".to_owned()), span: Span::default());
    Mixfix::Seq(vec![Mixfix::Arg(()), Mixfix::Atom(colon), Mixfix::Arg(())])
}

fn map_mixop() -> Mixop {
    let left = crate::phrase!(node: Atom::LBrace, span: Span::default());
    let right = crate::phrase!(node: Atom::RBrace, span: Span::default());
    Mixfix::Brack(left, Box::new(Mixfix::Arg(())), right)
}

fn map_find_opt(key: &Value, map: &[ValueRef]) -> Option<ValueRef> {
    let pair_mixop = pair_mixop();
    for pair in map {
        let Ok(value_case) = get::case(pair) else {
            continue;
        };
        if value_case.split().0 != pair_mixop {
            continue;
        }
        let args = value_case.args();
        if let [value_key, value_value] = args.as_slice()
            && value_key.as_ref() == key
        {
            return Some(Rc::clone(value_value));
        }
    }
    None
}

fn make_pair(
    add: &mut dyn FnMut(ValueRef),
    typ_key: &Typ,
    typ_value: &Typ,
    value_key: ValueRef,
    value_value: ValueRef,
) -> ValueRef {
    let pair_id = crate::phrase!(node: "pair".to_owned(), span: Span::default());
    let typ = make_type::var(pair_id, vec![typ_key.clone(), typ_value.clone()]);
    let pair_mixop = pair_mixop();
    let value_case = Mixop::fill(&pair_mixop, [value_key, value_value])
        .expect("the pair mixop has exactly two arguments");
    let value_pair = make::case(&typ, value_case, Span::default());
    add(Rc::clone(&value_pair));
    value_pair
}

fn map_update(
    add: &mut dyn FnMut(ValueRef),
    typ_key: &Typ,
    typ_value: &Typ,
    key: &ValueRef,
    value: &ValueRef,
    map: &[ValueRef],
) -> ValueMap {
    let mut found = false;
    let mut updated = Vec::with_capacity(map.len() + 1);
    let pair_mixop = pair_mixop();
    for pair in map {
        let matching = get::case(pair).ok().is_some_and(|value_case| {
            if value_case.split().0 != pair_mixop {
                return false;
            }
            let args = value_case.args();
            matches!(args.as_slice(), [value_key, _] if *value_key == key)
        });
        if !found && matching {
            updated.push(make_pair(
                add,
                typ_key,
                typ_value,
                Rc::clone(key),
                Rc::clone(value),
            ));
            found = true;
        } else {
            updated.push(Rc::clone(pair));
        }
    }
    if !found {
        updated.push(make_pair(
            add,
            typ_key,
            typ_value,
            Rc::clone(key),
            Rc::clone(value),
        ));
    }
    updated
}

// == Conversion between meta-maps and OCaml lists

fn map_of_value(span: &Span, value: &Value) -> Result<ValueMap, BuiltinError> {
    let value_case =
        get::case(value).map_err(|_| BuiltinError::new(span.clone(), "expected a map"))?;
    let map_mixop = map_mixop();
    if value_case.split().0 != map_mixop {
        return Err(BuiltinError::new(span.clone(), "expected a map"));
    }
    let args = value_case.args();
    let value_pairs = extract::one(span, &args)?;
    get::list(value_pairs)
        .map(<[ValueRef]>::to_vec)
        .map_err(|_| BuiltinError::new(span.clone(), "expected a map"))
}

fn value_of_map(
    add: &mut dyn FnMut(ValueRef),
    typ_key: &Typ,
    typ_value: &Typ,
    map: ValueMap,
) -> BuiltinResult {
    let pair_id = crate::phrase!(node: "pair".to_owned(), span: Span::default());
    let typ_pair = make_type::var(pair_id, vec![typ_key.clone(), typ_value.clone()]);
    let typ_pairs = make_type::list(typ_pair);
    let value_pairs = make::list(&typ_pairs, map, Span::default());
    add(Rc::clone(&value_pairs));
    let map_id = crate::phrase!(node: "map".to_owned(), span: Span::default());
    let typ = make_type::var(map_id, vec![typ_key.clone(), typ_value.clone()]);
    let map_mixop = map_mixop();
    let value_case =
        Mixop::fill(&map_mixop, [value_pairs]).expect("the map mixop has exactly one argument");
    let value = make::case(&typ, value_case, Span::default());
    return_value(add, value)
}

// == Built-in implementations

// dec $find_map<K, V>(map<K, V>, K) : V?

pub fn find_map(
    add: &mut dyn FnMut(ValueRef),
    span: &Span,
    type_args: &[Typ],
    values: &[ValueRef],
) -> BuiltinResult {
    let (_typ_key, typ_value) = extract::two(span, type_args)?;
    let (value_map, value_key) = extract::two(span, values)?;
    let map = map_of_value(span, value_map)?;
    let typ_opt = make_type::opt(typ_value.clone());
    let value_opt = map_find_opt(value_key, &map);
    let value = make::opt(&typ_opt, value_opt, Span::default());
    return_value(add, value)
}

// dec $find_maps<K, V>(map<K, V>*, K) : V?

pub fn find_maps(
    add: &mut dyn FnMut(ValueRef),
    span: &Span,
    type_args: &[Typ],
    values: &[ValueRef],
) -> BuiltinResult {
    let (_typ_key, typ_value) = extract::two(span, type_args)?;
    let (value_maps, value_key) = extract::two(span, values)?;
    let values = get::list(value_maps)
        .map_err(|error| BuiltinError::new(span.clone(), error.to_string()))?;
    let mut value_opt = None;
    for value_map in values {
        let map = map_of_value(span, value_map)?;
        if value_opt.is_none() {
            value_opt = map_find_opt(value_key, &map);
        }
    }
    let typ_opt = make_type::opt(typ_value.clone());
    let value = make::opt(&typ_opt, value_opt, Span::default());
    return_value(add, value)
}

// dec $add_map<K, V>(map<K, V>, K, V) : map<K, V>

pub fn add_map(
    add: &mut dyn FnMut(ValueRef),
    span: &Span,
    type_args: &[Typ],
    values: &[ValueRef],
) -> BuiltinResult {
    let (typ_key, typ_value) = extract::two(span, type_args)?;
    let (value_map, value_key, value_value) = extract::three(span, values)?;
    let map = map_of_value(span, value_map)?;
    let map = map_update(add, typ_key, typ_value, value_key, value_value, &map);
    value_of_map(add, typ_key, typ_value, map)
}

// dec $adds_map<K, V>(map<K, V>, K*, V* ) : map<K, V>

pub fn adds_map(
    add: &mut dyn FnMut(ValueRef),
    span: &Span,
    type_args: &[Typ],
    values: &[ValueRef],
) -> BuiltinResult {
    let (typ_key, typ_value) = extract::two(span, type_args)?;
    let (value_map, value_keys, value_values) = extract::three(span, values)?;
    let mut map = map_of_value(span, value_map)?;
    let values_key = get::list(value_keys)
        .map_err(|error| BuiltinError::new(span.clone(), error.to_string()))?;
    let values_value = get::list(value_values)
        .map_err(|error| BuiltinError::new(span.clone(), error.to_string()))?;
    if values_key.len() != values_value.len() {
        return Err(BuiltinError::new(
            span.clone(),
            "map key and value lists must have the same length",
        ));
    }
    for (value_key, value_value) in values_key.iter().zip(values_value) {
        map = map_update(add, typ_key, typ_value, value_key, value_value, &map);
    }
    value_of_map(add, typ_key, typ_value, map)
}

// dec $update_map<K, V>(map<K, V>, K, V) : map<K, V>

pub fn update_map(
    add: &mut dyn FnMut(ValueRef),
    span: &Span,
    type_args: &[Typ],
    values: &[ValueRef],
) -> BuiltinResult {
    let (typ_key, typ_value) = extract::two(span, type_args)?;
    let (value_map, value_key, value_value) = extract::three(span, values)?;
    let map = map_of_value(span, value_map)?;
    let map = map_update(add, typ_key, typ_value, value_key, value_value, &map);
    value_of_map(add, typ_key, typ_value, map)
}
