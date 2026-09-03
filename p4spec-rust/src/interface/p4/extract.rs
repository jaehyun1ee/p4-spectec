//! Typed projections from the P4 runtime-value parse tree.
//!
//! Each projection recognizes the mixfix shape produced by the grammar and
//! returns a small semantic result. For example, `type_parameter_ids` walks a
//! comma-separated tree left-to-right and yields its declared names.

use thiserror::Error;

use crate::runtime::value::Value;

use super::{context::TypeId, value};

// == Extraction errors

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

// == Public projections

pub fn id_of_name(input: &Value) -> Result<String, ExtractError> {
    value::id_of_name(input).ok_or(ExtractError::Name)
}

pub fn declaration_id(input: &Value) -> Result<String, ExtractError> {
    let declaration_name = value::declaration_name(input).ok_or(ExtractError::Declaration)?;
    value::id_of_name(&declaration_name).ok_or(ExtractError::Declaration)
}

pub fn declaration_has_type_parameters(input: &Value) -> Result<bool, ExtractError> {
    let _declaration_name = value::declaration_name(input).ok_or(ExtractError::Declaration)?;
    Ok(value::declaration_has_type_parameters(input))
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
            let id = id_of_name(values[1])?;
            ids.push(id);
            return Ok(());
        }
        if let Some(values) = value::matches(input, "`< typeParameterList `>") {
            return collect(values[0], ids);
        }
        if value::matches(input, "_EMPTY").is_some() {
            return Ok(());
        }
        let id = id_of_name(input).map_err(|_| ExtractError::TypeParameters)?;
        ids.push(id);
        Ok(())
    }

    let mut ids = Vec::new();
    collect(input, &mut ids)?;
    Ok(ids)
}
