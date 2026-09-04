//! Helper functions for P4 parser context management.
//!
//! These projections recover declaration names, referenced type identifiers,
//! and type-parameter presence from the runtime-value parse tree.

use crate::runtime::value::{Value, get};

use super::{context::TypeId, error::ExtractError, value};

fn unexpected(function: &'static str) -> ExtractError {
    ExtractError::UnexpectedValue(function)
}

// == Identifier extraction

pub fn id_of_name(value: &Value) -> Result<String, ExtractError> {
    if let Some(values) = value::matches(value, "_ID text") {
        let text = get::text(values[0]).map_err(|_| unexpected("id_of_name"))?;
        return Ok(text.to_owned());
    }
    if value::matches(value, "APPLY").is_some() {
        return Ok("apply".to_owned());
    }
    if value::matches(value, "KEY").is_some() {
        return Ok("key".to_owned());
    }
    if value::matches(value, "ACTIONS").is_some() {
        return Ok("actions".to_owned());
    }
    if value::matches(value, "STATE").is_some() {
        return Ok("state".to_owned());
    }
    if value::matches(value, "ENTRIES").is_some() {
        return Ok("entries".to_owned());
    }
    if value::matches(value, "TYPE").is_some() {
        return Ok("type".to_owned());
    }
    if value::matches(value, "PRIORITY").is_some() {
        return Ok("priority".to_owned());
    }
    if let Some(values) = value::matches(value, "_TID text") {
        let text = get::text(values[0]).map_err(|_| unexpected("id_of_name"))?;
        return Ok(text.to_owned());
    }
    if value::matches(value, "LIST").is_some() {
        return Ok("list".to_owned());
    }
    Err(unexpected("id_of_name"))
}

pub fn id_of_function_prototype(value: &Value) -> Result<String, ExtractError> {
    if let Some(values) = value::matches(
        value,
        "typeOrVoid name typeParameterListOpt `( parameterList `)",
    ) {
        return id_of_name(values[1]);
    }
    Err(unexpected("id_of_function_prototype"))
}

pub fn id_of_declaration(value: &Value) -> Result<String, ExtractError> {
    if let Some(values) = value::matches(value, "annotationList CONST type name initializer ';'") {
        return id_of_name(values[2]);
    }
    if let Some(values) = value::matches(value, "annotationList type `( argumentList `) name ';'") {
        return id_of_name(values[3]);
    }
    if let Some(values) = value::matches(
        value,
        "annotationList type `( argumentList `) name objectInitializer ';'",
    ) {
        return id_of_name(values[3]);
    }
    if let Some(values) = value::matches(value, "annotationList functionPrototype blockStatement") {
        return id_of_function_prototype(values[1]);
    }
    if let Some(values) = value::matches(
        value,
        "annotationList ACTION name `( parameterList `) blockStatement",
    ) {
        return id_of_name(values[1]);
    }
    if let Some(values) = value::matches(value, "annotationList EXTERN functionPrototype ';'") {
        return id_of_function_prototype(values[1]);
    }
    if let Some(values) = value::matches(
        value,
        "annotationList EXTERN nonTypeName typeParameterListOpt `{ externConstructorOrMethodPrototypeList `}",
    ) {
        return id_of_name(values[1]);
    }
    if let Some(values) = value::matches(
        value,
        "annotationList PARSER name typeParameterListOpt `( parameterList `) constructorParameterListOpt `{ parserLocalDeclarationList parserStateList `}",
    ) {
        return id_of_name(values[1]);
    }
    if let Some(values) = value::matches(
        value,
        "annotationList CONTROL name typeParameterListOpt `( parameterList `) constructorParameterListOpt `{ controlLocalDeclarationList APPLY controlBody `}",
    ) {
        return id_of_name(values[1]);
    }
    if let Some(values) = value::matches(
        value,
        "annotationList ENUM name `{ nameList trailingCommaOpt `}",
    ) {
        return id_of_name(values[1]);
    }
    if let Some(values) = value::matches(
        value,
        "annotationList ENUM type name `{ namedExpressionList trailingCommaOpt `}",
    ) {
        return id_of_name(values[2]);
    }
    if let Some(values) = value::matches(
        value,
        "annotationList STRUCT name typeParameterListOpt `{ typeFieldList `}",
    ) {
        return id_of_name(values[1]);
    }
    if let Some(values) = value::matches(
        value,
        "annotationList HEADER name typeParameterListOpt `{ typeFieldList `}",
    ) {
        return id_of_name(values[1]);
    }
    if let Some(values) = value::matches(
        value,
        "annotationList HEADER_UNION name typeParameterListOpt `{ typeFieldList `}",
    ) {
        return id_of_name(values[1]);
    }
    if let Some(values) = value::matches(value, "annotationList TYPEDEF typedef name ';'") {
        return id_of_name(values[2]);
    }
    if let Some(values) = value::matches(value, "annotationList TYPE type name ';'") {
        return id_of_name(values[2]);
    }
    if let Some(values) = value::matches(
        value,
        "annotationList PARSER name typeParameterListOpt `( parameterList `) ';'",
    ) {
        return id_of_name(values[1]);
    }
    if let Some(values) = value::matches(
        value,
        "annotationList CONTROL name typeParameterListOpt `( parameterList `) ';'",
    ) {
        return id_of_name(values[1]);
    }
    if let Some(values) = value::matches(
        value,
        "annotationList PACKAGE name typeParameterListOpt `( parameterList `) ';'",
    ) {
        return id_of_name(values[1]);
    }
    if let Some(values) = value::matches(value, "annotationList TABLE name `{ tablePropertyList `}")
    {
        return id_of_name(values[1]);
    }
    Err(unexpected("id_of_declaration"))
}

pub fn id_of_parameter(value: &Value) -> Result<String, ExtractError> {
    if let Some(values) = value::matches(value, "annotationList direction type name initializerOpt")
    {
        return id_of_name(values[3]);
    }
    Err(unexpected("id_of_parameter"))
}

// == Type identifier extraction

pub fn tid_of_type_ref(value: &Value) -> Result<TypeId, ExtractError> {
    for shape in [
        "BOOL",
        "ERROR",
        "MATCH_KIND",
        "STRING",
        "INT",
        "INT `< int `>",
        "INT `< `( expression `) `>",
        "BIT",
        "BIT `< int `>",
        "BIT `< `( expression `) `>",
        "VARBIT `< int `>",
        "VARBIT `< `( expression `) `>",
    ] {
        if value::matches(value, shape).is_some() {
            return Ok(TypeId::Empty);
        }
    }
    if let Some(values) = value::matches(value, "_TID text") {
        let text = get::text(values[0]).map_err(|_| unexpected("tid_of_type_ref"))?;
        return Ok(TypeId::Local(text.to_owned()));
    }
    if let Some(values) = value::matches(value, "_TID '.' typeName") {
        return match tid_of_type_ref(values[0])? {
            TypeId::Local(id) => Ok(TypeId::Global(id)),
            _ => Err(unexpected("tid_of_type_ref")),
        };
    }
    if let Some(values) = value::matches(value, "prefixedTypeName `< typeArgumentList `>") {
        return tid_of_type_ref(values[0]);
    }
    for shape in [
        "namedType `[ expression `]",
        "LIST `< typeArgument `>",
        "TUPLE `< typeArgumentList `>",
    ] {
        if value::matches(value, shape).is_some() {
            return Ok(TypeId::Empty);
        }
    }
    Err(unexpected("tid_of_type_ref"))
}

pub fn tid_of_declaration(value: &Value) -> Result<TypeId, ExtractError> {
    for shape in [
        "annotationList CONST type name initializer ';'",
        "annotationList type `( argumentList `) name ';'",
        "annotationList type `( argumentList `) name objectInitializer ';'",
    ] {
        if let Some(values) = value::matches(value, shape) {
            return tid_of_type_ref(values[1]);
        }
    }
    Err(unexpected("tid_of_declaration"))
}

// == Type parameter extraction

pub fn has_type_params(value: &Value) -> Result<bool, ExtractError> {
    if value::matches(value, "_EMPTY").is_some() {
        return Ok(false);
    }
    if value::matches(value, "`< typeParameterList `>").is_some() {
        return Ok(true);
    }
    Err(unexpected("has_type_params"))
}

pub fn has_type_params_function_prototype(value: &Value) -> Result<bool, ExtractError> {
    if let Some(values) = value::matches(
        value,
        "typeOrVoid name typeParameterListOpt `( parameterList `)",
    ) {
        return has_type_params(values[2]);
    }
    Err(unexpected("has_type_params_function_prototype"))
}

pub fn has_type_params_declaration(value: &Value) -> Result<bool, ExtractError> {
    for shape in [
        "annotationList CONST type name initializer ';'",
        "annotationList type `( argumentList `) name ';'",
        "annotationList type `( argumentList `) name objectInitializer ';'",
    ] {
        if value::matches(value, shape).is_some() {
            return Ok(false);
        }
    }
    if let Some(values) = value::matches(value, "annotationList functionPrototype blockStatement") {
        return has_type_params_function_prototype(values[1]);
    }
    if value::matches(
        value,
        "annotationList ACTION name `( parameterList `) blockStatement",
    )
    .is_some()
    {
        return Ok(false);
    }
    if let Some(values) = value::matches(value, "annotationList EXTERN functionPrototype ';'") {
        return has_type_params_function_prototype(values[1]);
    }
    for shape in [
        "annotationList EXTERN nonTypeName typeParameterListOpt `{ externConstructorOrMethodPrototypeList `}",
        "annotationList PARSER name typeParameterListOpt `( parameterList `) constructorParameterListOpt `{ parserLocalDeclarationList parserStateList `}",
        "annotationList CONTROL name typeParameterListOpt `( parameterList `) constructorParameterListOpt `{ controlLocalDeclarationList APPLY controlBody `}",
    ] {
        if let Some(values) = value::matches(value, shape) {
            return has_type_params(values[2]);
        }
    }
    for shape in [
        "annotationList ENUM name `{ nameList trailingCommaOpt `}",
        "annotationList ENUM type name `{ namedExpressionList trailingCommaOpt `}",
    ] {
        if value::matches(value, shape).is_some() {
            return Ok(false);
        }
    }
    for shape in [
        "annotationList STRUCT name typeParameterListOpt `{ typeFieldList `}",
        "annotationList HEADER name typeParameterListOpt `{ typeFieldList `}",
        "annotationList HEADER_UNION name typeParameterListOpt `{ typeFieldList `}",
    ] {
        if let Some(values) = value::matches(value, shape) {
            return has_type_params(values[2]);
        }
    }
    for shape in [
        "annotationList TYPEDEF typedef name ';'",
        "annotationList TYPE type name ';'",
    ] {
        if value::matches(value, shape).is_some() {
            return Ok(false);
        }
    }
    for shape in [
        "annotationList PARSER name typeParameterListOpt `( parameterList `) ';'",
        "annotationList CONTROL name typeParameterListOpt `( parameterList `) ';'",
        "annotationList PACKAGE name typeParameterListOpt `( parameterList `) ';'",
    ] {
        if let Some(values) = value::matches(value, shape) {
            return has_type_params(values[2]);
        }
    }
    if value::matches(value, "annotationList TABLE name `{ tablePropertyList `}").is_some() {
        return Ok(false);
    }
    Err(unexpected("has_type_params_declaration"))
}
