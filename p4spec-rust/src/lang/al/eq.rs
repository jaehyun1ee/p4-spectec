//! Semantic equality for algorithmic-language data
//!
//! Ignores source regions and delegates shared syntax to IL equality

use crate::lang::il;

use super::ast::*;

// == Semantic equality

// - Identifiers

/// Checks identifiers for semantic equality
pub fn eq_id(id_a: &Id, id_b: &Id) -> bool {
    il::eq::eq_id(id_a, id_b)
}

// - Atoms

/// Checks atoms for semantic equality
pub fn eq_atom(atom_a: &Atom, atom_b: &Atom) -> bool {
    il::eq::eq_atom(atom_a, atom_b)
}

/// Checks atom sequences for semantic equality
pub fn eq_atoms(atoms_a: &[Atom], atoms_b: &[Atom]) -> bool {
    il::eq::eq_atoms(atoms_a, atoms_b)
}

// - Mixfix operators

/// Checks mixfix operators for semantic equality
pub fn eq_mixop(mixop_a: &Mixop, mixop_b: &Mixop) -> bool {
    il::eq::eq_mixop(mixop_a, mixop_b)
}

// - Iterators

/// Checks iterators for semantic equality
pub fn eq_iter(iter_a: Iter, iter_b: Iter) -> bool {
    il::eq::eq_iter(iter_a, iter_b)
}

/// Checks iterator sequences for semantic equality
pub fn eq_iters(iters_a: &[Iter], iters_b: &[Iter]) -> bool {
    il::eq::eq_iters(iters_a, iters_b)
}

// - Variables

/// Checks variables for semantic equality
pub fn eq_var(var_a: &Var, var_b: &Var) -> bool {
    il::eq::eq_var(var_a, var_b)
}

/// Checks variable sequences for semantic equality
pub fn eq_vars(vars_a: &[Var], vars_b: &[Var]) -> bool {
    il::eq::eq_vars(vars_a, vars_b)
}

// - Types

/// Checks types for semantic equality
pub fn eq_typ(typ_a: &Typ, typ_b: &Typ) -> bool {
    il::eq::eq_typ(typ_a, typ_b)
}

/// Checks type sequences for semantic equality
pub fn eq_typs(typs_a: &[Typ], typs_b: &[Typ]) -> bool {
    il::eq::eq_typs(typs_a, typs_b)
}

/// Checks notation types for semantic equality
pub fn eq_not_typ(not_typ_a: &NotTyp, not_typ_b: &NotTyp) -> bool {
    il::eq::eq_not_typ(not_typ_a, not_typ_b)
}

// - Values

/// Checks values for semantic equality
pub fn eq_value(value_a: &Value, value_b: &Value) -> bool {
    il::eq::eq_value(value_a, value_b)
}

/// Checks value sequences for semantic equality
pub fn eq_values(values_a: &[Value], values_b: &[Value]) -> bool {
    il::eq::eq_values(values_a, values_b)
}

// - Expressions

/// Checks expressions for semantic equality
pub fn eq_exp(exp_a: &Exp, exp_b: &Exp) -> bool {
    il::eq::eq_exp(exp_a, exp_b)
}

/// Checks expression sequences for semantic equality
pub fn eq_exps(exps_a: &[Exp], exps_b: &[Exp]) -> bool {
    il::eq::eq_exps(exps_a, exps_b)
}

/// Checks iterated expressions for semantic equality
pub fn eq_iterexp(iter_exp_a: &IterExp, iter_exp_b: &IterExp) -> bool {
    il::eq::eq_iterexp(iter_exp_a, iter_exp_b)
}

/// Checks iterated-expression sequences for semantic equality
pub fn eq_iterexps(iter_exps_a: &[IterExp], iter_exps_b: &[IterExp]) -> bool {
    il::eq::eq_iterexps(iter_exps_a, iter_exps_b)
}

// - Patterns

/// Checks patterns for semantic equality
pub fn eq_pattern(pattern_a: &Pattern, pattern_b: &Pattern) -> bool {
    il::eq::eq_pattern(pattern_a, pattern_b)
}

// - Paths

/// Checks paths for semantic equality
pub fn eq_path(path_a: &Path, path_b: &Path) -> bool {
    il::eq::eq_path(path_a, path_b)
}

// - Type parameters

/// Checks type parameters for semantic equality
pub fn eq_tparam(tparam_a: &TParam, tparam_b: &TParam) -> bool {
    il::eq::eq_tparam(tparam_a, tparam_b)
}

/// Checks type-parameter sequences for semantic equality
pub fn eq_tparams(tparams_a: &[TParam], tparams_b: &[TParam]) -> bool {
    il::eq::eq_tparams(tparams_a, tparams_b)
}

// - Arguments

/// Checks arguments for semantic equality
pub fn eq_arg(arg_a: &Arg, arg_b: &Arg) -> bool {
    il::eq::eq_arg(arg_a, arg_b)
}

/// Checks argument sequences for semantic equality
pub fn eq_args(args_a: &[Arg], args_b: &[Arg]) -> bool {
    il::eq::eq_args(args_a, args_b)
}

// - Type arguments

/// Checks type arguments for semantic equality
pub fn eq_targ(targ_a: &Targ, targ_b: &Targ) -> bool {
    il::eq::eq_targ(targ_a, targ_b)
}

/// Checks type-argument sequences for semantic equality
pub fn eq_targs(targs_a: &[Targ], targs_b: &[Targ]) -> bool {
    il::eq::eq_targs(targs_a, targs_b)
}

// - Premises

/// Checks premises for semantic equality
pub fn eq_prem(prem_a: &Prem, prem_b: &Prem) -> bool {
    il::eq::eq_prem(prem_a, prem_b)
}

/// Checks iterated premises for semantic equality
pub fn eq_iterprem(iter_prem_a: &IterPrem, iter_prem_b: &IterPrem) -> bool {
    il::eq::eq_iterprem(iter_prem_a, iter_prem_b)
}

/// Checks iterated-premise sequences for semantic equality
pub fn eq_iterprems(iter_prems_a: &[IterPrem], iter_prems_b: &[IterPrem]) -> bool {
    il::eq::eq_iterprems(iter_prems_a, iter_prems_b)
}
