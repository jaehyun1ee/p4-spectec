//! Extracts name-resolution metadata from completed P4 parse-tree values.
//!
//! Parser semantic actions use these projections to register declaration names,
//! referenced type identifiers, and the presence of type parameters.

use crate::lang::data::value::{Value, get};

use super::{context::TypeId, error::ExtractError};

// == Identifier extraction

pub(super) fn id_name(value: &Value) -> Result<String, ExtractError> {
    let unexpected = || ExtractError::UnexpectedValue("id_name");
    get::matches! {
        value,
        "_ID text" => |values| {
            let text = get::text(values[0]).map_err(|_| unexpected())?;
            Ok(text.to_owned())
        },
        "APPLY" => |_values| Ok("apply".to_owned()),
        "KEY" => |_values| Ok("key".to_owned()),
        "ACTIONS" => |_values| Ok("actions".to_owned()),
        "STATE" => |_values| Ok("state".to_owned()),
        "ENTRIES" => |_values| Ok("entries".to_owned()),
        "TYPE" => |_values| Ok("type".to_owned()),
        "PRIORITY" => |_values| Ok("priority".to_owned()),
        "_TID text" => |values| {
            let text = get::text(values[0]).map_err(|_| unexpected())?;
            Ok(text.to_owned())
        },
        "LIST" => |_values| Ok("list".to_owned()),
        _ => Err(unexpected()),
    }
}

pub(super) fn id_function_prototype(value: &Value) -> Result<String, ExtractError> {
    get::matches! {
        value,
        "typeOrVoid name typeParameterListOpt `( parameterList `)" => |values| {
            id_name(values[1])
        },
        _ => Err(ExtractError::UnexpectedValue("id_function_prototype")),
    }
}

pub(super) fn id_declaration(value: &Value) -> Result<String, ExtractError> {
    get::matches! {
        value,
        "annotationList CONST type name initializer ';'" => |values| id_name(values[2]),
        "annotationList type `( argumentList `) name ';'" => |values| id_name(values[3]),
        "annotationList type `( argumentList `) name objectInitializer ';'" => |values| {
            id_name(values[3])
        },
        "annotationList functionPrototype blockStatement" => |values| {
            id_function_prototype(values[1])
        },
        "annotationList ACTION name `( parameterList `) blockStatement" => |values| {
            id_name(values[1])
        },
        "annotationList EXTERN functionPrototype ';'" => |values| {
            id_function_prototype(values[1])
        },
        "annotationList EXTERN nonTypeName typeParameterListOpt `{ externConstructorOrMethodPrototypeList `}" => |values| {
            id_name(values[1])
        },
        "annotationList PARSER name typeParameterListOpt `( parameterList `) constructorParameterListOpt `{ parserLocalDeclarationList parserStateList `}" => |values| {
            id_name(values[1])
        },
        "annotationList CONTROL name typeParameterListOpt `( parameterList `) constructorParameterListOpt `{ controlLocalDeclarationList APPLY controlBody `}" => |values| {
            id_name(values[1])
        },
        "annotationList ENUM name `{ nameList trailingCommaOpt `}" => |values| {
            id_name(values[1])
        },
        "annotationList ENUM type name `{ namedExpressionList trailingCommaOpt `}" => |values| {
            id_name(values[2])
        },
        "annotationList STRUCT name typeParameterListOpt `{ typeFieldList `}" => |values| {
            id_name(values[1])
        },
        "annotationList HEADER name typeParameterListOpt `{ typeFieldList `}" => |values| {
            id_name(values[1])
        },
        "annotationList HEADER_UNION name typeParameterListOpt `{ typeFieldList `}" => |values| {
            id_name(values[1])
        },
        "annotationList TYPEDEF typedef name ';'" => |values| id_name(values[2]),
        "annotationList TYPE typeRef name ';'" => |values| id_name(values[2]),
        "annotationList PARSER name typeParameterListOpt `( parameterList `) ';'"
        | "annotationList CONTROL name typeParameterListOpt `( parameterList `) ';'"
        | "annotationList PACKAGE name typeParameterListOpt `( parameterList `) ';'" => |values| {
            id_name(values[1])
        },
        "annotationList TABLE name `{ tablePropertyList `}" => |values| id_name(values[1]),
        _ => Err(ExtractError::UnexpectedValue("id_declaration")),
    }
}

// == Type identifier extraction

pub(super) fn type_id_type_ref(value: &Value) -> Result<TypeId, ExtractError> {
    let unexpected = || ExtractError::UnexpectedValue("type_id_type_ref");
    get::matches! {
        value,
        "BOOL"
        | "ERROR"
        | "MATCH_KIND"
        | "STRING"
        | "INT"
        | "INT `< int `>"
        | "INT `< `( expression `) `>"
        | "BIT"
        | "BIT `< int `>"
        | "BIT `< `( expression `) `>"
        | "VARBIT `< int `>"
        | "VARBIT `< `( expression `) `>" => |_values| Ok(TypeId::Empty),
        "_TID text" => |values| {
            let text = get::text(values[0]).map_err(|_| unexpected())?;
            Ok(TypeId::Local(text.to_owned()))
        },
        "_TID '.' typeName" => |values| {
            match type_id_type_ref(values[0])? {
                TypeId::Local(id) => Ok(TypeId::Global(id)),
                _ => Err(unexpected()),
            }
        },
        "prefixedTypeName `< typeArgumentList `>" => |values| {
            type_id_type_ref(values[0])
        },
        "namedType `[ expression `]"
        | "LIST `< typeArgument `>"
        | "TUPLE `< typeArgumentList `>" => |_values| Ok(TypeId::Empty),
        _ => Err(unexpected()),
    }
}

pub(super) fn type_id_declaration(value: &Value) -> Result<TypeId, ExtractError> {
    get::matches! {
        value,
        "annotationList CONST type name initializer ';'"
        | "annotationList type `( argumentList `) name ';'"
        | "annotationList type `( argumentList `) name objectInitializer ';'" => |values| {
            type_id_type_ref(values[1])
        },
        _ => Err(ExtractError::UnexpectedValue("type_id_declaration")),
    }
}

// == Type parameter extraction

fn has_type_params(value: &Value) -> Result<bool, ExtractError> {
    get::matches! {
        value,
        "_EMPTY" => |_values| Ok(false),
        "`< typeParameterList `>" => |_values| Ok(true),
        _ => Err(ExtractError::UnexpectedValue("has_type_params")),
    }
}

pub(super) fn has_type_params_function_prototype(value: &Value) -> Result<bool, ExtractError> {
    get::matches! {
        value,
        "typeOrVoid name typeParameterListOpt `( parameterList `)" => |values| {
            has_type_params(values[2])
        },
        _ => Err(ExtractError::UnexpectedValue(
            "has_type_params_function_prototype",
        )),
    }
}

pub(super) fn has_type_params_declaration(value: &Value) -> Result<bool, ExtractError> {
    get::matches! {
        value,
        "annotationList CONST type name initializer ';'"
        | "annotationList type `( argumentList `) name ';'"
        | "annotationList type `( argumentList `) name objectInitializer ';'" => |_values| {
            Ok(false)
        },
        "annotationList functionPrototype blockStatement" => |values| {
            has_type_params_function_prototype(values[1])
        },
        "annotationList ACTION name `( parameterList `) blockStatement" => |_values| Ok(false),
        "annotationList EXTERN functionPrototype ';'" => |values| {
            has_type_params_function_prototype(values[1])
        },
        "annotationList EXTERN nonTypeName typeParameterListOpt `{ externConstructorOrMethodPrototypeList `}"
        | "annotationList PARSER name typeParameterListOpt `( parameterList `) constructorParameterListOpt `{ parserLocalDeclarationList parserStateList `}"
        | "annotationList CONTROL name typeParameterListOpt `( parameterList `) constructorParameterListOpt `{ controlLocalDeclarationList APPLY controlBody `}" => |values| {
            has_type_params(values[2])
        },
        "annotationList ENUM name `{ nameList trailingCommaOpt `}"
        | "annotationList ENUM type name `{ namedExpressionList trailingCommaOpt `}" => |_values| {
            Ok(false)
        },
        "annotationList STRUCT name typeParameterListOpt `{ typeFieldList `}"
        | "annotationList HEADER name typeParameterListOpt `{ typeFieldList `}"
        | "annotationList HEADER_UNION name typeParameterListOpt `{ typeFieldList `}" => |values| {
            has_type_params(values[2])
        },
        "annotationList TYPEDEF typedef name ';'"
        | "annotationList TYPE typeRef name ';'" => |_values| Ok(false),
        "annotationList PARSER name typeParameterListOpt `( parameterList `) ';'"
        | "annotationList CONTROL name typeParameterListOpt `( parameterList `) ';'"
        | "annotationList PACKAGE name typeParameterListOpt `( parameterList `) ';'" => |values| {
            has_type_params(values[2])
        },
        "annotationList TABLE name `{ tablePropertyList `}" => |_values| Ok(false),
        _ => Err(ExtractError::UnexpectedValue(
            "has_type_params_declaration",
        )),
    }
}
