use crate::{
    domain::source::{Region, Spanned},
    lang::{
        il::ast::{self as il, Iter, TypKind},
        sl::ast::{self as sl},
        xl::num,
    },
};

// Type

pub type T = il::Typ;

// Constructor

pub mod make {
    use super::*;

    pub fn iterate(mut typ: T, iters: &[Iter]) -> T {
        for iter in iters {
            let span = typ.span.clone();
            typ = Spanned::new(TypKind::IterT(Box::new(typ), *iter), span);
        }
        typ
    }

    pub fn bool_kind() -> TypKind {
        TypKind::BoolT
    }

    pub fn bool_type() -> T {
        Spanned::new(bool_kind(), Region::none())
    }

    pub fn nat_kind() -> TypKind {
        TypKind::NumT(num::Typ::NatT)
    }

    pub fn nat_type() -> T {
        Spanned::new(nat_kind(), Region::none())
    }

    pub fn int_kind() -> TypKind {
        TypKind::NumT(num::Typ::IntT)
    }

    pub fn int_type() -> T {
        Spanned::new(int_kind(), Region::none())
    }

    pub fn num_kind(num_type: num::Typ) -> TypKind {
        TypKind::NumT(num_type)
    }

    pub fn num_type(num_type: num::Typ) -> T {
        Spanned::new(num_kind(num_type), Region::none())
    }

    pub fn text_kind() -> TypKind {
        TypKind::TextT
    }

    pub fn text_type() -> T {
        Spanned::new(text_kind(), Region::none())
    }

    pub fn var_kind(id: il::Id, type_args: Vec<il::Targ>) -> TypKind {
        TypKind::VarT(id, type_args)
    }

    pub fn var_type(id: il::Id, type_args: Vec<il::Targ>) -> T {
        Spanned::new(var_kind(id, type_args), Region::none())
    }

    pub fn tuple_kind(types: Vec<T>) -> TypKind {
        TypKind::TupleT(types)
    }

    pub fn tuple_type(types: Vec<T>) -> T {
        Spanned::new(tuple_kind(types), Region::none())
    }

    pub fn iter_kind(typ: T, iter: Iter) -> TypKind {
        TypKind::IterT(Box::new(typ), iter)
    }

    pub fn iter_type(typ: T, iter: Iter) -> T {
        Spanned::new(iter_kind(typ, iter), Region::none())
    }

    pub fn opt_kind(typ: T) -> TypKind {
        iter_kind(typ, Iter::Opt)
    }

    pub fn opt_type(typ: T) -> T {
        iter_type(typ, Iter::Opt)
    }

    pub fn list_kind(typ: T) -> TypKind {
        iter_kind(typ, Iter::List)
    }

    pub fn list_type(typ: T) -> T {
        iter_type(typ, Iter::List)
    }

    pub fn func_kind(type_params: Vec<il::TParam>, param_types: Vec<T>, return_type: T) -> TypKind {
        TypKind::FuncT(type_params, param_types, Box::new(return_type))
    }

    pub fn func_type(type_params: Vec<il::TParam>, param_types: Vec<T>, return_type: T) -> T {
        Spanned::new(
            func_kind(type_params, param_types, return_type),
            Region::none(),
        )
    }

    pub fn of_param_il(param: &il::Param) -> T {
        match &param.node {
            il::ParamKind::ExpP(typ) => typ.clone(),
            il::ParamKind::DefP(_, type_params, params, return_type) => func_type(
                type_params.clone(),
                of_params_il(params),
                return_type.clone(),
            ),
        }
    }

    pub fn of_params_il(params: &[il::Param]) -> Vec<T> {
        params.iter().map(of_param_il).collect()
    }

    pub fn of_param_sl(param: &sl::Param) -> T {
        match &param.node {
            sl::ParamKind::ExpP(typ, _) => typ.clone(),
            sl::ParamKind::DefP(_, type_params, params, return_type) => func_type(
                type_params.clone(),
                of_params_sl(params),
                return_type.clone(),
            ),
        }
    }

    pub fn of_params_sl(params: &[sl::Param]) -> Vec<T> {
        params.iter().map(of_param_sl).collect()
    }
}
