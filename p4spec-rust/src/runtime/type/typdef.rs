use crate::lang::il::ast::{DefTyp, TParam};

// Type definitions

#[derive(Clone, Debug, PartialEq)]
pub enum TypeDef {
    // Type parameter
    Param,
    // Extern type
    Extern,
    // Type being defined
    Defining(Vec<TParam>),
    // Type that is completely defined
    Defined(Vec<TParam>, Box<DefTyp>),
}

impl TypeDef {
    pub fn type_params(&self) -> &[TParam] {
        match self {
            Self::Param | Self::Extern => &[],
            Self::Defining(type_params) | Self::Defined(type_params, _) => type_params,
        }
    }
}
