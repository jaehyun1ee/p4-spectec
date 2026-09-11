//! List builtins in specification order.
//!
//! Each builtin first extracts its type and value args, performs the list
//! operation, and returns the newly constructed runtime value. For
//! example, `rev_` turns `[a, b]` into `[b, a]` while preserving the element
//! type supplied by the specification.

use std::rc::Rc;

use num_bigint::BigInt;

use crate::lang::{
    common::source::Span,
    data::{
        typ,
        value::{Value, ValueArena, ValueKind, get, make},
    },
    il::ast::Typ,
    traits::{cmp::SyntaxCmp, eq::SyntaxEq},
    xl::num,
};

use super::{BuiltinError, extract};

// == Conversion between runtime values and Rust collections

fn list_of_value<'a>(arena: &'a ValueArena, value: &Value) -> Result<&'a [Value], BuiltinError> {
    get::list(arena, value).map_err(|error| BuiltinError::new(error.to_string()))
}

fn bigint_of_value<'a>(arena: &'a ValueArena, value: &Value) -> Result<&'a BigInt, BuiltinError> {
    let num = get::num(arena, value).map_err(|error| BuiltinError::new(error.to_string()))?;
    Ok(num::to_int(num))
}

// dec $rev_<X>(X*) : X*

pub fn rev_(
    arena: &mut ValueArena,
    targs: &[Typ],
    values: &[Value],
) -> Result<Value, BuiltinError> {
    let typ = extract::one(targs)?;
    let typ_list = typ::make::list(typ.clone());
    let value_list = extract::one(values)?;
    let mut values = list_of_value(arena, value_list)?.to_vec();
    values.reverse();
    let value = make::list(arena, typ_list.node.into(), values, Span::default())?;
    Ok(value)
}

// dec $concat_<X>((X*)*) : X*

pub fn concat_(
    arena: &mut ValueArena,
    targs: &[Typ],
    values: &[Value],
) -> Result<Value, BuiltinError> {
    let typ = extract::one(targs)?;
    let typ_list = typ::make::list(typ.clone());
    let mut concatenated = Vec::new();
    let value_lists = extract::one(values)?;
    let lists = list_of_value(arena, value_lists)?;
    for value_list in lists {
        let values = list_of_value(arena, value_list)?;
        concatenated.extend(values.iter().cloned());
    }
    let value = make::list(arena, typ_list.node.into(), concatenated, Span::default())?;
    Ok(value)
}

// dec $distinct_<K>(K*) : bool

pub fn distinct_(
    arena: &mut ValueArena,
    targs: &[Typ],
    values: &[Value],
) -> Result<Value, BuiltinError> {
    let _typ = extract::one(targs)?;
    let value_list = extract::one(values)?;
    let values = list_of_value(arena, value_list)?;
    let mut values = values.to_vec();
    values.sort_by(|value_a, value_b| arena.view(*value_a).syntax_cmp(&arena.view(*value_b)));
    let all_distinct = values
        .windows(2)
        .all(|values| !arena.view(values[0]).syntax_eq(&arena.view(values[1])));
    let value = make::bool(arena, all_distinct, Span::default())?;
    Ok(value)
}

// dec $partition_<X>(X*, nat) : (X*, X*)

pub fn partition_(
    arena: &mut ValueArena,
    targs: &[Typ],
    values: &[Value],
) -> Result<Value, BuiltinError> {
    let typ = extract::one(targs)?;
    let typ_list = Rc::new(typ::make::list(typ.clone()).node);
    let (value_list, value_len) = extract::two(values)?;
    let values = list_of_value(arena, value_list)?;
    let len = bigint_of_value(arena, value_len)?;
    let (values_left, values_right): (Vec<_>, Vec<_>) = values
        .iter()
        .copied()
        .enumerate()
        .partition(|(index, _)| BigInt::from(*index) < *len);
    let value_left = make::list(
        arena,
        typ_list.clone(),
        values_left.into_iter().map(|(_, value)| value).collect(),
        Span::default(),
    )?;
    let value_right = make::list(
        arena,
        typ_list.clone(),
        values_right.into_iter().map(|(_, value)| value).collect(),
        Span::default(),
    )?;
    let typ_tuple = typ::make::tuple(vec![typ.clone(), typ.clone()]);
    let value = make::tuple(
        arena,
        typ_tuple.node.into(),
        vec![value_left, value_right],
        Span::default(),
    )?;
    Ok(value)
}

// dec $assoc_<X, Y>(X, (X, Y)*) : Y?

pub fn assoc_(
    arena: &mut ValueArena,
    targs: &[Typ],
    values: &[Value],
) -> Result<Value, BuiltinError> {
    let (_typ_key, typ_value) = extract::two(targs)?;
    let (value, value_list) = extract::two(values)?;
    let mut found = None;
    for pair in list_of_value(arena, value_list)? {
        let pair = match arena.kind(pair) {
            ValueKind::Tuple(pair) if pair.len() == 2 => pair,
            _ => {
                return Err(BuiltinError::new("expected an association pair"));
            }
        };
        if found.is_none() && arena.view(*value).syntax_eq(&arena.view(pair[0])) {
            found = Some(pair[1]);
        }
    }
    let typ_opt = typ::make::opt(typ_value.clone());
    let value = make::opt(arena, typ_opt.node.into(), found, Span::default())?;
    Ok(value)
}

// dec $sort_<X>((nat, X)*) : (nat, X)*

pub fn sort_(
    arena: &mut ValueArena,
    targs: &[Typ],
    values: &[Value],
) -> Result<Value, BuiltinError> {
    let typ_value = extract::one(targs)?;
    let typ_pair = typ::make::tuple(vec![typ::make::nat(), typ_value.clone()]);
    let typ_list = typ::make::list(typ_pair);
    let mut keyed = Vec::new();
    let value_list = extract::one(values)?;
    let pairs = list_of_value(arena, value_list)?;
    for pair in pairs {
        let pair_values = match arena.kind(pair) {
            ValueKind::Tuple(pair) if pair.len() == 2 => pair,
            _ => {
                return Err(BuiltinError::new("expected a numeric pair"));
            }
        };
        let key = bigint_of_value(arena, &pair_values[0])?.clone();
        keyed.push((key, *pair));
    }
    keyed.sort_by(|(key_a, _), (key_b, _)| key_a.cmp(key_b));
    let values = keyed.into_iter().map(|(_, value)| value).collect();
    let value = make::list(arena, typ_list.node.into(), values, Span::default())?;
    Ok(value)
}

// builtin dec $transpose_<X>(X**) : X**

pub fn transpose_(
    arena: &mut ValueArena,
    targs: &[Typ],
    values: &[Value],
) -> Result<Value, BuiltinError> {
    let typ = extract::one(targs)?;
    let typ_list = typ::make::list(typ.clone());
    let typ_matrix = typ::make::list(typ_list.clone());
    let typ_list = Rc::new(typ_list.node);
    let value_matrix = extract::one(values)?;
    let rows = list_of_value(arena, value_matrix)?;
    let width = match rows.first() {
        Some(row) => {
            let values = list_of_value(arena, row)?;
            values.len()
        }
        None => 0,
    };
    let mut columns = vec![Vec::with_capacity(rows.len()); width];
    for row in rows {
        let row = list_of_value(arena, row)?;
        if row.len() != width {
            return Err(BuiltinError::new("cannot transpose a matrix of values"));
        }
        for (index, value) in row.iter().enumerate() {
            columns[index].push(*value);
        }
    }
    let mut value_rows = Vec::with_capacity(columns.len());
    for column in columns {
        let value_row = make::list(arena, typ_list.clone(), column, Span::default())?;
        value_rows.push(value_row);
    }
    let value = make::list(arena, typ_matrix.node.into(), value_rows, Span::default())?;
    Ok(value)
}
