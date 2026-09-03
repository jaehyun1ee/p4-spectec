//! List builtins, in the same order as `interface/builtin/lists.ml`.
//!
//! Each builtin first extracts its type and value arguments, performs the list
//! operation, and returns the newly constructed runtime value. For
//! example, `rev_` turns `[a, b]` into `[b, a]` while preserving the element
//! type supplied by the specification.

use std::{collections::BTreeSet, rc::Rc};

use num_bigint::BigInt;

use crate::{
    lang::common::source::Span,
    lang::{il::ast::Typ, xl::num},
    runtime::{
        types::typ,
        value::{Value, ValueKind, get, make},
    },
};

use super::{BuiltinError, extract};

// == Conversion between runtime values and Rust collections

fn list_of_value(value: &Value) -> Result<&[Rc<Value>], BuiltinError> {
    get::list(value).map_err(|error| BuiltinError::new(error.to_string()))
}

fn bigint_of_value(value: &Value) -> Result<&BigInt, BuiltinError> {
    let number = get::num(value).map_err(|error| BuiltinError::new(error.to_string()))?;
    Ok(num::to_int(number))
}

// dec $rev_<X>(X*) : X*

pub fn rev_(targs: &[Typ], values: &[Rc<Value>]) -> Result<Rc<Value>, BuiltinError> {
    let typ = extract::one(targs)?;
    let typ_list = typ::list(typ.clone());
    let value_list = extract::one(values)?;
    let mut values = list_of_value(value_list)?.to_vec();
    values.reverse();
    let value = make::list(&typ_list, values, Span::default());
    Ok(value)
}

// dec $concat_<X>((X*)*) : X*

pub fn concat_(targs: &[Typ], values: &[Rc<Value>]) -> Result<Rc<Value>, BuiltinError> {
    let typ = extract::one(targs)?;
    let typ_list = typ::list(typ.clone());
    let mut concatenated = Vec::new();
    let value_lists = extract::one(values)?;
    let lists = list_of_value(value_lists)?;
    for value_list in lists {
        let values = list_of_value(value_list)?;
        concatenated.extend(values.iter().cloned());
    }
    let value = make::list(&typ_list, concatenated, Span::default());
    Ok(value)
}

// dec $distinct_<K>(K*) : bool

pub fn distinct_(targs: &[Typ], values: &[Rc<Value>]) -> Result<Rc<Value>, BuiltinError> {
    let _typ = extract::one(targs)?;
    let value_list = extract::one(values)?;
    let values = list_of_value(value_list)?;
    let set: BTreeSet<_> = values.iter().collect();
    let all_distinct = set.len() == values.len();
    let value = make::bool(all_distinct, Span::default());
    Ok(value)
}

// dec $partition_<X>(X*, nat) : (X*, X*)

pub fn partition_(targs: &[Typ], values: &[Rc<Value>]) -> Result<Rc<Value>, BuiltinError> {
    let typ = extract::one(targs)?;
    let typ_list = typ::list(typ.clone());
    let (value_list, value_len) = extract::two(values)?;
    let values = list_of_value(value_list)?;
    let len = bigint_of_value(value_len)?;
    let (values_left, values_right): (Vec<_>, Vec<_>) = values
        .iter()
        .enumerate()
        .partition(|(index, _)| BigInt::from(*index) < *len);
    let value_left = make::list(
        &typ_list,
        values_left
            .into_iter()
            .map(|(_, value)| Rc::clone(value))
            .collect(),
        Span::default(),
    );
    let value_right = make::list(
        &typ_list,
        values_right
            .into_iter()
            .map(|(_, value)| Rc::clone(value))
            .collect(),
        Span::default(),
    );
    let typ_tuple = typ::tuple(vec![typ.clone(), typ.clone()]);
    let value = make::tuple(&typ_tuple, vec![value_left, value_right], Span::default());
    Ok(value)
}

// dec $assoc_<X, Y>(X, (X, Y)*) : Y?

pub fn assoc_(targs: &[Typ], values: &[Rc<Value>]) -> Result<Rc<Value>, BuiltinError> {
    let (_typ_key, typ_value) = extract::two(targs)?;
    let (value, value_list) = extract::two(values)?;
    let mut found = None;
    for pair in list_of_value(value_list)? {
        let pair = match &pair.node {
            ValueKind::Tuple(pair) if pair.len() == 2 => pair,
            _ => {
                return Err(BuiltinError::new("expected an association pair"));
            }
        };
        if found.is_none() && value == &pair[0] {
            found = Some(Rc::clone(&pair[1]));
        }
    }
    let typ_opt = typ::opt(typ_value.clone());
    let value = make::opt(&typ_opt, found, Span::default());
    Ok(value)
}

// dec $sort_<X>((nat, X)*) : (nat, X)*

pub fn sort_(targs: &[Typ], values: &[Rc<Value>]) -> Result<Rc<Value>, BuiltinError> {
    let typ_value = extract::one(targs)?;
    let typ_pair = typ::tuple(vec![typ::nat(), typ_value.clone()]);
    let typ_list = typ::list(typ_pair);
    let mut keyed = Vec::new();
    let value_list = extract::one(values)?;
    let pairs = list_of_value(value_list)?;
    for pair in pairs {
        let pair_values = match &pair.node {
            ValueKind::Tuple(pair) if pair.len() == 2 => pair,
            _ => {
                return Err(BuiltinError::new("expected a numeric pair"));
            }
        };
        let key = bigint_of_value(&pair_values[0])?.clone();
        keyed.push((key, Rc::clone(pair)));
    }
    keyed.sort_by(|(key_a, _), (key_b, _)| key_a.cmp(key_b));
    let values = keyed.into_iter().map(|(_, value)| value).collect();
    let value = make::list(&typ_list, values, Span::default());
    Ok(value)
}

// builtin dec $transpose_<X>(X**) : X**

pub fn transpose_(targs: &[Typ], values: &[Rc<Value>]) -> Result<Rc<Value>, BuiltinError> {
    let typ = extract::one(targs)?;
    let typ_list = typ::list(typ.clone());
    let typ_matrix = typ::list(typ_list.clone());
    let value_matrix = extract::one(values)?;
    let rows = list_of_value(value_matrix)?;
    let width = match rows.first() {
        Some(row) => {
            let values = list_of_value(row)?;
            values.len()
        }
        None => 0,
    };
    let mut columns = vec![Vec::with_capacity(rows.len()); width];
    for row in rows {
        let row = list_of_value(row)?;
        if row.len() != width {
            return Err(BuiltinError::new("cannot transpose a matrix of values"));
        }
        for (index, value) in row.iter().enumerate() {
            columns[index].push(Rc::clone(value));
        }
    }
    let mut value_rows = Vec::with_capacity(columns.len());
    for column in columns {
        let value_row = make::list(&typ_list, column, Span::default());
        value_rows.push(value_row);
    }
    let value = make::list(&typ_matrix, value_rows, Span::default());
    Ok(value)
}
