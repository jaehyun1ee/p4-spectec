//! Projections that read a value of a given kind

use std::rc::Rc;

use crate::util::json::json;

use crate::lang::{common::prim::num::Number, data::arena::Arena};

use super::{ValueCase, ValueError, ValueField, ValueFlat, ValueFlatKind, ValueTag};

// - Errors

/// The error for a value of the wrong kind.
fn unexpected(arena: &Arena, value: &ValueFlat, expected: ValueTag) -> ValueError {
    ValueError::KindMismatch { expected, actual: arena.kind(value).tag() }
}

// - Primitives

/// The boolean in a value.
pub fn bool(arena: &Arena, value: &ValueFlat) -> Result<bool, ValueError> {
    match arena.kind(value) {
        ValueFlatKind::Bool(value) => Ok(*value),
        _ => Err(unexpected(arena, value, ValueTag::Bool)),
    }
}

/// The number in a value.
pub fn num<'a>(arena: &'a Arena, value: &ValueFlat) -> Result<&'a Number, ValueError> {
    match arena.kind(value) {
        ValueFlatKind::Num(value) => Ok(value),
        _ => Err(unexpected(arena, value, ValueTag::Num)),
    }
}

/// The text in a value.
pub fn text<'a>(arena: &'a Arena, value: &ValueFlat) -> Result<&'a str, ValueError> {
    match arena.kind(value) {
        ValueFlatKind::Text(value) => Ok(value),
        _ => Err(unexpected(arena, value, ValueTag::Text)),
    }
}

// - Structures

/// The fields of a struct value.
pub fn structure<'a>(arena: &'a Arena, value: &ValueFlat) -> Result<&'a [ValueField], ValueError> {
    match arena.kind(value) {
        ValueFlatKind::Struct(value_fields) => Ok(value_fields),
        _ => Err(unexpected(arena, value, ValueTag::Struct)),
    }
}

// - Cases

/// The shape and arguments of a case value.
pub fn case<'a>(arena: &'a Arena, value: &ValueFlat) -> Result<&'a ValueCase, ValueError> {
    match arena.kind(value) {
        ValueFlatKind::Case(value_case) => Ok(value_case),
        _ => Err(unexpected(arena, value, ValueTag::Case)),
    }
}

/// Matches a case value against mixop texts, binding its arguments per arm.
macro_rules! matches {
    (
        @arms $arena:ident, $value_case:ident;
        $shape:literal $(| $shape_alt:literal)* => |$values:ident| $body:expr,
        $($rest:tt)+
    ) => {{
        match $value_case {
            Some(value_case)
                if [$shape, $($shape_alt),*].into_iter().any(|shape_text| {
                    let expected = $crate::lang::data::notation::mixop::shape(shape_text);
                    $arena.arena_shape().eq_mixop(*value_case.mixop(), expected.as_ref())
                }) =>
            {
                let $values = value_case.args().iter().collect::<Vec<_>>();
                $body
            }
            _ => $crate::lang::data::value::get::matches! {
                @arms $arena, $value_case;
                $($rest)+
            },
        }
    }};
    (@arms $arena:ident, $value_case:ident; _ => $fallback:expr $(,)?) => {
        $fallback
    };
    ($arena:expr, $value:expr, $($arms:tt)+) => {{
        let value = $value;
        let arena = $arena;
        let value_case = match arena.kind(value) {
            $crate::lang::data::value::ValueFlatKind::Case(value_case) => Some(value_case),
            _ => None,
        };
        $crate::lang::data::value::get::matches! {
            @arms arena, value_case;
            $($arms)+
        }
    }};
}

pub(crate) use matches;

// - Sequences

/// The components of a tuple value.
pub fn tuple<'a>(arena: &'a Arena, value: &ValueFlat) -> Result<&'a [ValueFlat], ValueError> {
    match arena.kind(value) {
        ValueFlatKind::Tuple(values) => Ok(values),
        _ => Err(unexpected(arena, value, ValueTag::Tuple)),
    }
}

/// The content of an option value.
pub fn opt(arena: &Arena, value: &ValueFlat) -> Result<Option<ValueFlat>, ValueError> {
    match arena.kind(value) {
        ValueFlatKind::Opt(value) => Ok(*value),
        _ => Err(unexpected(arena, value, ValueTag::Opt)),
    }
}

/// The elements of a list value.
pub fn list<'a>(arena: &'a Arena, value: &ValueFlat) -> Result<&'a [ValueFlat], ValueError> {
    match arena.kind(value) {
        ValueFlatKind::List(values) => Ok(values),
        _ => Err(unexpected(arena, value, ValueTag::List)),
    }
}

// - Externals

/// The JSON of a host-owned value.
pub fn external<'a>(arena: &'a Arena, value: &ValueFlat) -> Result<&'a Rc<json>, ValueError> {
    match arena.kind(value) {
        ValueFlatKind::Extern(json) => Ok(json),
        _ => Err(unexpected(arena, value, ValueTag::Extern)),
    }
}

// - Indexing

/// The element at `index`, or an out-of-bounds error.
pub fn nth(values: &[ValueFlat], index: usize) -> Result<&ValueFlat, ValueError> {
    values
        .get(index)
        .ok_or(ValueError::IndexOutOfBounds { index, len: values.len() })
}

// - Arity

/// Exactly one value.
pub fn one(values: &[ValueFlat]) -> Result<&ValueFlat, ValueError> {
    match values {
        [value] => Ok(value),
        _ => Err(ValueError::CountMismatch { expected: 1, actual: values.len() }),
    }
}

/// Exactly two values.
pub fn two(values: &[ValueFlat]) -> Result<(&ValueFlat, &ValueFlat), ValueError> {
    match values {
        [value_a, value_b] => Ok((value_a, value_b)),
        _ => Err(ValueError::CountMismatch { expected: 2, actual: values.len() }),
    }
}

/// Exactly three values.
#[allow(clippy::type_complexity)]
pub fn three(values: &[ValueFlat]) -> Result<(&ValueFlat, &ValueFlat, &ValueFlat), ValueError> {
    match values {
        [value_a, value_b, value_c] => Ok((value_a, value_b, value_c)),
        _ => Err(ValueError::CountMismatch { expected: 3, actual: values.len() }),
    }
}

/// Exactly four values.
#[allow(clippy::type_complexity)]
pub fn four(
    values: &[ValueFlat],
) -> Result<(&ValueFlat, &ValueFlat, &ValueFlat, &ValueFlat), ValueError> {
    match values {
        [value_a, value_b, value_c, value_d] => Ok((value_a, value_b, value_c, value_d)),
        _ => Err(ValueError::CountMismatch { expected: 4, actual: values.len() }),
    }
}
