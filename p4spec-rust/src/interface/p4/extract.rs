use thiserror::Error;

use crate::runtime::value::Value;

use super::{context::TypeId, value};

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ExtractError {
    #[error("expected a P4 name")]
    Name,
    #[error("expected a P4 declaration")]
    Declaration,
    #[error("expected a P4 type reference")]
    TypeReference,
    #[error("expected a P4 type-parameter list")]
    TypeParameters,
}

pub fn id_of_name(input: &Value) -> Result<String, ExtractError> {
    value::id_of_name(input).ok_or(ExtractError::Name)
}

pub fn declaration_id(input: &Value) -> Result<String, ExtractError> {
    value::declaration_name(input)
        .as_deref()
        .and_then(value::id_of_name)
        .ok_or(ExtractError::Declaration)
}

pub fn declaration_has_type_parameters(input: &Value) -> Result<bool, ExtractError> {
    value::declaration_name(input)
        .ok_or(ExtractError::Declaration)
        .map(|_| value::declaration_has_type_parameters(input))
}

pub fn type_id_of_ref(input: &Value) -> Result<TypeId, ExtractError> {
    let type_id = value::type_id_of_ref(input);
    (type_id != TypeId::Empty)
        .then_some(type_id)
        .ok_or(ExtractError::TypeReference)
}

pub fn type_parameter_ids(input: &Value) -> Result<Vec<String>, ExtractError> {
    fn collect(input: &Value, ids: &mut Vec<String>) -> Result<(), ExtractError> {
        if let Some(values) = value::matches(input, "typeParameterList ',' typeParameter") {
            collect(values[0], ids)?;
            ids.push(id_of_name(values[1])?);
            return Ok(());
        }
        if let Some(values) = value::matches(input, "`< typeParameterList `>") {
            return collect(values[0], ids);
        }
        if value::matches(input, "_EMPTY").is_some() {
            return Ok(());
        }
        ids.push(id_of_name(input).map_err(|_| ExtractError::TypeParameters)?);
        Ok(())
    }

    let mut ids = Vec::new();
    collect(input, &mut ids)?;
    Ok(ids)
}
