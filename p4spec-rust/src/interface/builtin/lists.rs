//! List builtins, in the same order as `interface/builtin/lists.ml`.
//!
//! Each builtin first extracts its type and value arguments, performs the list
//! operation, and finally registers the newly constructed runtime value.  For
//! example, `rev_` turns `[a, b]` into `[b, a]` while preserving the element
//! type supplied by the specification.

use std::{collections::BTreeSet, rc::Rc};

use num_bigint::BigInt;

use crate::{
    lang::common::source::Span,
    lang::{il::ast::Typ, xl::num},
    runtime::{
        types::typ as make_type,
        value::{Value, ValueKind, ValueRef, get, make},
    },
};

use super::{BuiltinError, BuiltinResult, extract, return_value};

// == Conversion between runtime values and Rust collections

fn list_of_value<'a>(span: &Span, value: &'a Value) -> Result<&'a [ValueRef], BuiltinError> {
    get::list(value).map_err(|error| BuiltinError::new(span.clone(), error.to_string()))
}

fn bigint_of_value<'a>(span: &Span, value: &'a Value) -> Result<&'a BigInt, BuiltinError> {
    let number =
        get::num(value).map_err(|error| BuiltinError::new(span.clone(), error.to_string()))?;
    Ok(num::to_int(number))
}

// dec $rev_<X>(X* ) : X*

pub fn rev_(
    add: &mut dyn FnMut(ValueRef),
    span: &Span,
    type_args: &[Typ],
    values: &[ValueRef],
) -> BuiltinResult {
    let typ = extract::one(span, type_args)?;
    let list_type = make_type::list(typ.clone());
    let value_list = extract::one(span, values)?;
    let mut values = list_of_value(span, value_list)?.to_vec();
    values.reverse();
    let value = make::list(&list_type, values, Span::default());
    return_value(add, value)
}

// dec $concat_<X>((X* )* ) : X*

pub fn concat_(
    add: &mut dyn FnMut(ValueRef),
    span: &Span,
    type_args: &[Typ],
    values: &[ValueRef],
) -> BuiltinResult {
    let typ = extract::one(span, type_args)?;
    let list_type = make_type::list(typ.clone());
    let mut concatenated = Vec::new();
    let value_lists = extract::one(span, values)?;
    let lists = list_of_value(span, value_lists)?;
    for value_list in lists {
        let values = list_of_value(span, value_list)?;
        concatenated.extend(values.iter().cloned());
    }
    let value = make::list(&list_type, concatenated, Span::default());
    return_value(add, value)
}

// dec $distinct_<K>(K* ) : bool

pub fn distinct_(
    add: &mut dyn FnMut(ValueRef),
    span: &Span,
    type_args: &[Typ],
    values: &[ValueRef],
) -> BuiltinResult {
    let _typ = extract::one(span, type_args)?;
    let value_list = extract::one(span, values)?;
    let values = list_of_value(span, value_list)?;
    let set: BTreeSet<_> = values.iter().collect();
    let all_distinct = set.len() == values.len();
    let value = make::bool(all_distinct, Span::default());
    return_value(add, value)
}

// dec $partition_<X>(X*, nat) : (X*, X* )

pub fn partition_(
    add: &mut dyn FnMut(ValueRef),
    span: &Span,
    type_args: &[Typ],
    values: &[ValueRef],
) -> BuiltinResult {
    let typ = extract::one(span, type_args)?;
    let list_type = make_type::list(typ.clone());
    let (value_list, value_len) = extract::two(span, values)?;
    let values = list_of_value(span, value_list)?;
    let len = bigint_of_value(span, value_len)?;
    let (values_left, values_right): (Vec<_>, Vec<_>) = values
        .iter()
        .enumerate()
        .partition(|(index, _)| BigInt::from(*index) < *len);
    let value_left = make::list(
        &list_type,
        values_left
            .into_iter()
            .map(|(_, value)| Rc::clone(value))
            .collect(),
        Span::default(),
    );
    add(Rc::clone(&value_left));
    let value_right = make::list(
        &list_type,
        values_right
            .into_iter()
            .map(|(_, value)| Rc::clone(value))
            .collect(),
        Span::default(),
    );
    add(Rc::clone(&value_right));
    let tuple_type = make_type::tuple(vec![typ.clone(), typ.clone()]);
    let value = make::tuple(&tuple_type, vec![value_left, value_right], Span::default());
    return_value(add, value)
}

// dec $assoc_<X, Y>(X, (X, Y)* ) : Y?

pub fn assoc_(
    add: &mut dyn FnMut(ValueRef),
    span: &Span,
    type_args: &[Typ],
    values: &[ValueRef],
) -> BuiltinResult {
    let (_key_type, value_type) = extract::two(span, type_args)?;
    let (value, value_list) = extract::two(span, values)?;
    let mut found = None;
    for pair in list_of_value(span, value_list)? {
        let pair = match &pair.node {
            ValueKind::Tuple(pair) if pair.len() == 2 => pair,
            _ => {
                return Err(BuiltinError::new(
                    span.clone(),
                    "expected an association pair",
                ));
            }
        };
        if found.is_none() && value == &pair[0] {
            found = Some(Rc::clone(&pair[1]));
        }
    }
    let option_type = make_type::opt(value_type.clone());
    let value = make::opt(&option_type, found, Span::default());
    return_value(add, value)
}

// dec $sort_<X>((nat, X)* ) : (nat, X)*

pub fn sort_(
    add: &mut dyn FnMut(ValueRef),
    span: &Span,
    type_args: &[Typ],
    values: &[ValueRef],
) -> BuiltinResult {
    let value_type = extract::one(span, type_args)?;
    let pair_type = make_type::tuple(vec![make_type::nat(), value_type.clone()]);
    let list_type = make_type::list(pair_type);
    let mut keyed = Vec::new();
    let value_list = extract::one(span, values)?;
    let pairs = list_of_value(span, value_list)?;
    for pair in pairs {
        let pair_values = match &pair.node {
            ValueKind::Tuple(pair) if pair.len() == 2 => pair,
            _ => {
                return Err(BuiltinError::new(span.clone(), "expected a numeric pair"));
            }
        };
        let key = bigint_of_value(span, &pair_values[0])?.clone();
        keyed.push((key, Rc::clone(pair)));
    }
    keyed.sort_by(|(key_a, _), (key_b, _)| key_a.cmp(key_b));
    let values = keyed.into_iter().map(|(_, value)| value).collect();
    let value = make::list(&list_type, values, Span::default());
    return_value(add, value)
}

// builtin dec $transpose_<X>(X** ) : X**

pub fn transpose_(
    add: &mut dyn FnMut(ValueRef),
    span: &Span,
    type_args: &[Typ],
    values: &[ValueRef],
) -> BuiltinResult {
    let typ = extract::one(span, type_args)?;
    let list_type = make_type::list(typ.clone());
    let matrix_type = make_type::list(list_type.clone());
    let value_matrix = extract::one(span, values)?;
    let rows = list_of_value(span, value_matrix)?;
    let width = match rows.first() {
        Some(row) => {
            let values = list_of_value(span, row)?;
            values.len()
        }
        None => 0,
    };
    let mut columns = vec![Vec::with_capacity(rows.len()); width];
    for row in rows {
        let row = list_of_value(span, row)?;
        if row.len() != width {
            return Err(BuiltinError::new(
                Span::default(),
                "cannot transpose a matrix of values",
            ));
        }
        for (index, value) in row.iter().enumerate() {
            columns[index].push(Rc::clone(value));
        }
    }
    let mut value_rows = Vec::with_capacity(columns.len());
    for column in columns {
        let value_row = make::list(&list_type, column, Span::default());
        add(Rc::clone(&value_row));
        value_rows.push(value_row);
    }
    let value = make::list(&matrix_type, value_rows, Span::default());
    return_value(add, value)
}
