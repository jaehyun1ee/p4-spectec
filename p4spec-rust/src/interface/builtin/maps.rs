use std::rc::Rc;

use crate::{
    domain::{
        atom::Atom,
        mixfix::{Mixfix, Mixop},
        source::{Region, Spanned},
    },
    lang::il::ast::Typ,
    runtime::{
        r#type::typ::make as make_type,
        value::{Value, ValueRef, get, make},
    },
};

use super::{BuiltinError, BuiltinResult, extract, return_value};

// Value map

type ValueMap = Vec<ValueRef>;

fn pair_mixop() -> Mixop {
    Mixfix::Infix(
        Box::new(Mixfix::Arg(())),
        Spanned::new(Atom::Colon, Region::none()),
        Box::new(Mixfix::Arg(())),
    )
}

fn map_mixop() -> Mixop {
    Mixfix::Brack(
        Spanned::new(Atom::LBrace, Region::none()),
        Box::new(Mixfix::Arg(())),
        Spanned::new(Atom::RBrace, Region::none()),
    )
}

fn map_find_opt(key: &Value, map: &[ValueRef]) -> Option<ValueRef> {
    for pair in map {
        let Ok(value_case) = get::case(pair) else {
            continue;
        };
        if value_case.split().0 != pair_mixop() {
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
    let typ = make_type::var_type(
        Spanned::new("pair".to_owned(), Region::none()),
        vec![typ_key.clone(), typ_value.clone()],
    );
    let value_case = Mixop::fill(&pair_mixop(), [value_key, value_value])
        .expect("the pair mixop has exactly two arguments");
    let value_pair = make::case(&typ, value_case, Region::none());
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
    for pair in map {
        let matching = get::case(pair).ok().is_some_and(|value_case| {
            if value_case.split().0 != pair_mixop() {
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

// Conversion between meta-maps and OCaml lists

fn map_of_value(span: &Region, value: &Value) -> Result<ValueMap, BuiltinError> {
    let value_case =
        get::case(value).map_err(|_| BuiltinError::new(span.clone(), "expected a map"))?;
    if value_case.split().0 != map_mixop() {
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
    let typ_pair = make_type::var_type(
        Spanned::new("pair".to_owned(), Region::none()),
        vec![typ_key.clone(), typ_value.clone()],
    );
    let value_pairs = make::list(&make_type::list_type(typ_pair), map, Region::none());
    add(Rc::clone(&value_pairs));
    let typ = make_type::var_type(
        Spanned::new("map".to_owned(), Region::none()),
        vec![typ_key.clone(), typ_value.clone()],
    );
    let value_case =
        Mixop::fill(&map_mixop(), [value_pairs]).expect("the map mixop has exactly one argument");
    return_value(add, make::case(&typ, value_case, Region::none()))
}

// Built-in implementations

// dec $find_map<K, V>(map<K, V>, K) : V?

pub fn find_map(
    add: &mut dyn FnMut(ValueRef),
    span: &Region,
    type_args: &[Typ],
    values: &[ValueRef],
) -> BuiltinResult {
    let (_typ_key, typ_value) = extract::two(span, type_args)?;
    let (value_map, value_key) = extract::two(span, values)?;
    let map = map_of_value(span, value_map)?;
    let typ_opt = make_type::opt_type(typ_value.clone());
    let value_opt = map_find_opt(value_key, &map);
    return_value(add, make::opt(&typ_opt, value_opt, Region::none()))
}

// dec $find_maps<K, V>(map<K, V>*, K) : V?

pub fn find_maps(
    add: &mut dyn FnMut(ValueRef),
    span: &Region,
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
    let typ_opt = make_type::opt_type(typ_value.clone());
    return_value(add, make::opt(&typ_opt, value_opt, Region::none()))
}

// dec $add_map<K, V>(map<K, V>, K, V) : map<K, V>

pub fn add_map(
    add: &mut dyn FnMut(ValueRef),
    span: &Region,
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
    span: &Region,
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
    span: &Region,
    type_args: &[Typ],
    values: &[ValueRef],
) -> BuiltinResult {
    let (typ_key, typ_value) = extract::two(span, type_args)?;
    let (value_map, value_key, value_value) = extract::three(span, values)?;
    let map = map_of_value(span, value_map)?;
    let map = map_update(add, typ_key, typ_value, value_key, value_value, &map);
    value_of_map(add, typ_key, typ_value, map)
}
