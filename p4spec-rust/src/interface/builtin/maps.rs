//! Map builtins represented by the specification's ordered pair list.
//!
//! A map value is decoded to its pair list, updated at the first matching key,
//! and encoded again. Thus updating `{a: 1}` with `a: 2` preserves its list
//! position and produces `{a: 2}`.

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
    traits::eq::SyntaxEq,
};

use super::{BuiltinError, extract};

// == Value map

type ValueMap = Vec<Value>;

fn pair_mixop() -> Rc<Mixop> {
    shape("k ':' v")
}

fn map_mixop() -> Rc<Mixop> {
    shape("`{ k `}")
}

fn map_find_opt(arena: &ValueArena, key: &Value, map: &[Value]) -> Option<Value> {
    let pair_mixop = pair_mixop();
    for pair in map {
        let Ok(value_case) = get::case(arena, pair) else {
            continue;
        };
        if !value_case.eq_shape(&pair_mixop) {
            continue;
        }
        let args = value_case.args();
        if let [value_key, value_value] = args.as_slice()
            && arena.view(**value_key).syntax_eq(&arena.view(*key))
        {
            return Some(**value_value);
        }
    }
    None
}

fn make_pair(
    arena: &mut ValueArena,
    typ_key: &Typ,
    typ_value: &Typ,
    value_key: Value,
    value_value: Value,
) -> Result<Value, BuiltinError> {
    let pair_id = crate::phrase!(node: "pair".to_owned(), span: Span::default());
    let typ = typ::make::var(pair_id, vec![typ_key.clone(), typ_value.clone()]);
    let pair_mixop = pair_mixop();
    let value_case = Mixop::fill(&pair_mixop, [value_key, value_value])
        .expect("the pair mixop has exactly two arguments");
    Ok(make::case(
        arena,
        typ.node.clone(),
        value_case,
        Span::default(),
    )?)
}

fn map_update(
    arena: &mut ValueArena,
    typ_key: &Typ,
    typ_value: &Typ,
    key: &Value,
    value: &Value,
    map: &[Value],
) -> Result<ValueMap, BuiltinError> {
    let mut found = false;
    let mut updated = Vec::with_capacity(map.len() + 1);
    let pair_mixop = pair_mixop();
    for pair in map {
        let matching = get::case(arena, pair).ok().is_some_and(|value_case| {
            if !value_case.eq_shape(&pair_mixop) {
                return false;
            }
            let args = value_case.args();
            matches!(args.as_slice(), [value_key, _] if arena.view(**value_key).syntax_eq(&arena.view(*key)))
        });
        if !found && matching {
            updated.push(make_pair(arena, typ_key, typ_value, *key, *value)?);
            found = true;
        } else {
            updated.push(*pair);
        }
    }
    if !found {
        updated.push(make_pair(arena, typ_key, typ_value, *key, *value)?);
    }
    Ok(updated)
}

// == Conversion between meta-maps and runtime lists

fn map_of_value(arena: &ValueArena, value: &Value) -> Result<ValueMap, BuiltinError> {
    let value_case = get::case(arena, value).map_err(|_| BuiltinError::new("expected a map"))?;
    let map_mixop = map_mixop();
    if !value_case.eq_shape(&map_mixop) {
        return Err(BuiltinError::new("expected a map"));
    }
    let args = value_case.args();
    let value_pairs = extract::one(&args)?;
    get::list(arena, value_pairs)
        .map(<[Value]>::to_vec)
        .map_err(|_| BuiltinError::new("expected a map"))
}

fn value_of_map(
    arena: &mut ValueArena,
    typ_key: &Typ,
    typ_value: &Typ,
    map: ValueMap,
) -> Result<Value, BuiltinError> {
    let pair_id = crate::phrase!(node: "pair".to_owned(), span: Span::default());
    let typ_pair = typ::make::var(pair_id, vec![typ_key.clone(), typ_value.clone()]);
    let typ_pairs = typ::make::list(typ_pair);
    let value_pairs = make::list(arena, typ_pairs.node.clone(), map, Span::default())?;
    let map_id = crate::phrase!(node: "map".to_owned(), span: Span::default());
    let typ = typ::make::var(map_id, vec![typ_key.clone(), typ_value.clone()]);
    let map_mixop = map_mixop();
    let value_case =
        Mixop::fill(&map_mixop, [value_pairs]).expect("the map mixop has exactly one argument");
    let value = make::case(arena, typ.node.clone(), value_case, Span::default())?;
    Ok(value)
}

// == Built-in implementations

// dec $find_map<K, V>(map<K, V>, K) : V?

pub fn find_map(
    arena: &mut ValueArena,
    targs: &[Typ],
    values: &[Value],
) -> Result<Value, BuiltinError> {
    let (_typ_key, typ_value) = extract::two(targs)?;
    let (value_map, value_key) = extract::two(values)?;
    let map = map_of_value(arena, value_map)?;
    let typ_opt = typ::make::opt(typ_value.clone());
    let value_opt = map_find_opt(arena, value_key, &map);
    let value = make::opt(arena, typ_opt.node.clone(), value_opt, Span::default())?;
    Ok(value)
}

// dec $find_maps<K, V>(map<K, V>*, K) : V?

pub fn find_maps(
    arena: &mut ValueArena,
    targs: &[Typ],
    values: &[Value],
) -> Result<Value, BuiltinError> {
    let (_typ_key, typ_value) = extract::two(targs)?;
    let (value_maps, value_key) = extract::two(values)?;
    let values =
        get::list(arena, value_maps).map_err(|error| BuiltinError::new(error.to_string()))?;
    let mut value_opt = None;
    for value_map in values {
        let map = map_of_value(arena, value_map)?;
        if value_opt.is_none() {
            value_opt = map_find_opt(arena, value_key, &map);
        }
    }
    let typ_opt = typ::make::opt(typ_value.clone());
    let value = make::opt(arena, typ_opt.node.clone(), value_opt, Span::default())?;
    Ok(value)
}

// dec $add_map<K, V>(map<K, V>, K, V) : map<K, V>

pub fn add_map(
    arena: &mut ValueArena,
    targs: &[Typ],
    values: &[Value],
) -> Result<Value, BuiltinError> {
    let (typ_key, typ_value) = extract::two(targs)?;
    let (value_map, value_key, value_value) = extract::three(values)?;
    let map = map_of_value(arena, value_map)?;
    let map = map_update(arena, typ_key, typ_value, value_key, value_value, &map)?;
    value_of_map(arena, typ_key, typ_value, map)
}

// dec $adds_map<K, V>(map<K, V>, K*, V*) : map<K, V>

pub fn adds_map(
    arena: &mut ValueArena,
    targs: &[Typ],
    values: &[Value],
) -> Result<Value, BuiltinError> {
    let (typ_key, typ_value) = extract::two(targs)?;
    let (value_map, value_keys, value_values) = extract::three(values)?;
    let mut map = map_of_value(arena, value_map)?;
    let values_key = get::list(arena, value_keys)
        .map_err(|error| BuiltinError::new(error.to_string()))?
        .to_vec();
    let values_value = get::list(arena, value_values)
        .map_err(|error| BuiltinError::new(error.to_string()))?
        .to_vec();
    if values_key.len() != values_value.len() {
        return Err(BuiltinError::new(
            "map key and value lists must have the same length",
        ));
    }
    for (value_key, value_value) in values_key.iter().zip(&values_value) {
        map = map_update(arena, typ_key, typ_value, value_key, value_value, &map)?;
    }
    value_of_map(arena, typ_key, typ_value, map)
}

// dec $update_map<K, V>(map<K, V>, K, V) : map<K, V>

pub fn update_map(
    arena: &mut ValueArena,
    targs: &[Typ],
    values: &[Value],
) -> Result<Value, BuiltinError> {
    let (typ_key, typ_value) = extract::two(targs)?;
    let (value_map, value_key, value_value) = extract::three(values)?;
    let map = map_of_value(arena, value_map)?;
    let map = map_update(arena, typ_key, typ_value, value_key, value_value, &map)?;
    value_of_map(arena, typ_key, typ_value, map)
}
