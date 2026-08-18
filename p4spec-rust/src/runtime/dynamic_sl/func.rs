use std::fmt;

use crate::{
    lang::sl::ast::{Block, ElseBlock, Param, TParam, TableRow, Typ},
    runtime::r#type::typ::make,
};

// Function

#[derive(Clone, Debug, PartialEq)]
pub enum Function {
    Extern(Vec<TParam>, Vec<Param>, Typ),
    Builtin(Vec<TParam>, Vec<Param>, Typ),
    Table(Vec<Param>, Typ, Vec<TableRow>),
    Defined(Vec<TParam>, Vec<Param>, Typ, Block, Option<ElseBlock>),
}

impl fmt::Display for Function {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Extern(..) => "extern function",
            Self::Builtin(..) => "builtin function",
            Self::Table(..) => "table function",
            Self::Defined(..) => "defined function",
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Signature {
    pub type_params: Vec<TParam>,
    pub param_types: Vec<Typ>,
    pub return_type: Typ,
}

impl Function {
    pub fn get_signature(&self) -> Signature {
        match self {
            Self::Extern(type_params, params, return_type)
            | Self::Builtin(type_params, params, return_type)
            | Self::Defined(type_params, params, return_type, ..) => Signature {
                type_params: type_params.clone(),
                param_types: make::of_params_sl(params),
                return_type: return_type.clone(),
            },
            Self::Table(params, return_type, _) => Signature {
                type_params: Vec::new(),
                param_types: make::of_params_sl(params),
                return_type: return_type.clone(),
            },
        }
    }
}
