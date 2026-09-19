//! Packs an IL value representing a P4 value from a Rust type

use num_bigint::BigInt;

use crate::{
    lang::{
        common::source::Span,
        data::value::{Value, ValueArena, make},
    },
    runner::ExternError,
};

// == P4 values

/// `D int`
pub fn p4_arbitrary_int(arena: &mut ValueArena, int: BigInt) -> Result<Value, ExternError> {
    let value_int = make::int(arena, int, Span::default())?;
    Ok(make::case_shaped! {
        arena: arena,
        shape: "D int",
        args: vec![value_int],
        typ: "value",
        span: Span::default(),
    }?)
}

/// `nat W int`
pub fn p4_fixed_bit(
    arena: &mut ValueArena,
    width: BigInt,
    int: BigInt,
) -> Result<Value, ExternError> {
    let nat = width
        .try_into()
        .map_err(|error: crate::lang::common::prim::num::NumericError| {
            ExternError::Failure(error.to_string())
        })?;
    let value_width = make::nat(arena, nat, Span::default())?;
    let value_int = make::int(arena, int, Span::default())?;
    Ok(make::case_shaped! {
        arena: arena,
        shape: "nat W int",
        args: vec![value_width, value_int],
        typ: "value",
        span: Span::default(),
    }?)
}

/// `tid . id`
pub fn p4_enum(arena: &mut ValueArena, id_enum: &str, id: &str) -> Result<Value, ExternError> {
    let value_enum = make::text(arena, id_enum.to_owned(), Span::default())?;
    let value_id = make::text(arena, id.to_owned(), Span::default())?;
    Ok(make::case_shaped! {
        arena: arena,
        shape: "tid '.' id",
        args: vec![value_enum, value_id],
        typ: "value",
        span: Span::default(),
    }?)
}
