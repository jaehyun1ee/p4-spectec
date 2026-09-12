use num_bigint::BigInt;

use crate::{
    lang::{
        common::source::Span,
        data::{
            typ,
            value::{Value, ValueArena, make},
        },
    },
    runner::ExternError,
};

pub fn p4_arbitrary_int(arena: &mut ValueArena, int: BigInt) -> Result<Value, ExternError> {
    let value_int = make::int(arena, int, Span::default())?;
    Ok(make::case_shaped_(
        arena,
        "D int",
        vec![value_int],
        "value",
        Span::default(),
    )?)
}

pub fn p4_fixed_bit(
    arena: &mut ValueArena,
    width: BigInt,
    int: BigInt,
) -> Result<Value, ExternError> {
    let nat = width
        .try_into()
        .map_err(|error: crate::lang::xl::num::NumericError| {
            ExternError::Failure(error.to_string())
        })?;
    let value_width = make::nat(arena, nat, Span::default())?;
    let value_int = make::int(arena, int, Span::default())?;
    Ok(make::case_shaped_(
        arena,
        "nat W int",
        vec![value_width, value_int],
        "value",
        Span::default(),
    )?)
}

pub fn return_result(arena: &mut ValueArena, value: Option<Value>) -> Result<Value, ExternError> {
    let typ = typ::make::opt(typ::make::var(
        crate::phrase!(node: "value".to_owned(), span: Span::default()),
        Vec::new(),
    ));
    let value_opt = make::opt(arena, typ.node.into(), value, Span::default())?;
    Ok(make::case_shaped_(
        arena,
        "RETURN value?",
        vec![value_opt],
        "returnResult",
        Span::default(),
    )?)
}

pub fn reject_transition(arena: &mut ValueArena, name: &str) -> Result<Value, ExternError> {
    let value_name = make::text(arena, name.to_owned(), Span::default())?;
    let value_err = make::case_shaped_(
        arena,
        "ERROR '.' nameIR",
        vec![value_name],
        "errorValue",
        Span::default(),
    )?;
    Ok(make::case_shaped_(
        arena,
        "REJECT errorValue",
        vec![value_err],
        "rejectTransitionResult",
        Span::default(),
    )?)
}
