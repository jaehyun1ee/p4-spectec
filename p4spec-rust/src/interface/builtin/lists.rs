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
    let mut values = list_of_value(span, extract::one(span, values)?)?.to_vec();
    values.reverse();
    return_value(add, make::list(&list_type, values, Span::default()))
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
    for value in list_of_value(span, extract::one(span, values)?)? {
        concatenated.extend(list_of_value(span, value)?.iter().cloned());
    }
    return_value(add, make::list(&list_type, concatenated, Span::default()))
}

// dec $distinct_<K>(K* ) : bool

pub fn distinct_(
    add: &mut dyn FnMut(ValueRef),
    span: &Span,
    type_args: &[Typ],
    values: &[ValueRef],
) -> BuiltinResult {
    let _typ = extract::one(span, type_args)?;
    let values = list_of_value(span, extract::one(span, values)?)?;
    let set: BTreeSet<_> = values.iter().collect();
    return_value(add, make::bool(set.len() == values.len(), Span::default()))
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
    return_value(
        add,
        make::tuple(&tuple_type, vec![value_left, value_right], Span::default()),
    )
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
        let pair = match &pair.kind {
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
    return_value(add, make::opt(&option_type, found, Span::default()))
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
    for pair in list_of_value(span, extract::one(span, values)?)? {
        let pair_values = match &pair.kind {
            ValueKind::Tuple(pair) if pair.len() == 2 => pair,
            _ => {
                return Err(BuiltinError::new(span.clone(), "expected a numeric pair"));
            }
        };
        keyed.push((
            bigint_of_value(span, &pair_values[0])?.clone(),
            Rc::clone(pair),
        ));
    }
    keyed.sort_by(|(key_a, _), (key_b, _)| key_a.cmp(key_b));
    return_value(
        add,
        make::list(
            &list_type,
            keyed.into_iter().map(|(_, value)| value).collect(),
            Span::default(),
        ),
    )
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
    let rows = list_of_value(span, extract::one(span, values)?)?;
    let width = match rows.first() {
        Some(row) => list_of_value(span, row)?.len(),
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
    return_value(add, make::list(&matrix_type, value_rows, Span::default()))
}
