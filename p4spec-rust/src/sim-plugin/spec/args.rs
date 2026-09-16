use crate::{
    lang::data::value::{Value, ValueArena, ValueError, get},
    runner::ExternError,
};

// == Arguments

pub fn assoc(
    arena: &ValueArena,
    value_ids: Value,
    value_args: Value,
) -> Result<Vec<(String, Value)>, ExternError> {
    let names = get::list(arena, &value_ids)?
        .iter()
        .map(|value_id| get::text(arena, value_id).map(str::to_owned))
        .collect::<Result<Vec<_>, _>>()?;
    let values = get::list(arena, &value_args)?;
    if names.len() != values.len() {
        return Err(
            ValueError::ExpectedCount { expected: names.len(), actual: values.len() }.into()
        );
    }
    Ok(names.into_iter().zip(values.iter().copied()).collect())
}

/// Finds the first argument with the requested name
pub fn find(args: &[(String, Value)], name: &str) -> Result<Value, ExternError> {
    args.iter()
        .find(|(name_arg, _)| name_arg == name)
        .map(|(_, value)| *value)
        .ok_or_else(|| ExternError::Failure(format!("argument not found: {name}")))
}
