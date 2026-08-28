use crate::lang::{
    common::source::{Span, Spanned},
    il, pl, sl,
    xl::num,
};

/// Wraps a type in each iterator from innermost to outermost
pub fn iterate_type(mut typ: il::ast::Typ, iters: &[il::ast::Iter]) -> il::ast::Typ {
    for iter in iters {
        let span = typ.span.clone();
        typ = Spanned::new(il::ast::TypKind::Iter(Box::new(typ), *iter), span);
    }
    typ
}

pub fn bool_type() -> il::ast::Typ {
    Spanned::new(il::ast::TypKind::Bool, Span::default())
}

pub fn natural_type() -> il::ast::Typ {
    number_type(num::Typ::Nat)
}

pub fn integer_type() -> il::ast::Typ {
    number_type(num::Typ::Int)
}

pub fn number_type(number_type: num::Typ) -> il::ast::Typ {
    Spanned::new(il::ast::TypKind::Num(number_type), Span::default())
}

pub fn text_type() -> il::ast::Typ {
    Spanned::new(il::ast::TypKind::Text, Span::default())
}

pub fn variable_type(id: il::ast::Id, args: Vec<il::ast::Targ>) -> il::ast::Typ {
    Spanned::new(il::ast::TypKind::Var(id, args), Span::default())
}

pub fn tuple_type(types: Vec<il::ast::Typ>) -> il::ast::Typ {
    Spanned::new(il::ast::TypKind::Tuple(types), Span::default())
}

pub fn iteration_type(typ: il::ast::Typ, iter: il::ast::Iter) -> il::ast::Typ {
    Spanned::new(il::ast::TypKind::Iter(Box::new(typ), iter), Span::default())
}

pub fn optional_type(typ: il::ast::Typ) -> il::ast::Typ {
    iteration_type(typ, il::ast::Iter::Opt)
}

pub fn list_type(typ: il::ast::Typ) -> il::ast::Typ {
    iteration_type(typ, il::ast::Iter::List)
}

pub fn function_type(
    type_parameters: Vec<il::ast::TParam>,
    parameter_types: Vec<il::ast::Typ>,
    result_type: il::ast::Typ,
) -> il::ast::Typ {
    Spanned::new(
        il::ast::TypKind::Func(type_parameters, parameter_types, Box::new(result_type)),
        Span::default(),
    )
}

/// Extracts the callable type represented by a stage parameter
pub trait ParameterType {
    fn parameter_type(&self) -> il::ast::Typ;
}

impl ParameterType for il::ast::Param {
    fn parameter_type(&self) -> il::ast::Typ {
        match &self.node {
            il::ast::ParamKind::Exp(typ) => typ.clone(),
            il::ast::ParamKind::Def(_, tparams, params, typ) => {
                function_type(tparams.clone(), parameter_types(params), typ.clone())
            }
        }
    }
}

impl ParameterType for sl::ast::Param {
    fn parameter_type(&self) -> il::ast::Typ {
        match &self.node {
            sl::ast::ParamKind::Exp(typ, _) => typ.clone(),
            sl::ast::ParamKind::Def(_, tparams, params, typ) => {
                function_type(tparams.clone(), parameter_types(params), typ.clone())
            }
        }
    }
}

impl ParameterType for pl::ast::Param {
    fn parameter_type(&self) -> il::ast::Typ {
        match &self.node {
            pl::ast::ParamKind::Exp(typ, _) => typ.clone(),
            pl::ast::ParamKind::Def(_, tparams, params, typ) => {
                function_type(tparams.clone(), parameter_types(params), typ.clone())
            }
        }
    }
}

/// Extracts callable types from a list of stage parameters
pub fn parameter_types<P: ParameterType>(parameters: &[P]) -> Vec<il::ast::Typ> {
    parameters
        .iter()
        .map(ParameterType::parameter_type)
        .collect()
}
