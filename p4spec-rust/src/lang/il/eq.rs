//! Semantic equality for intermediate-language data
//!
//! Ignores source regions;
//! compares syntax represented by spanned nodes

use std::cmp::Ordering;

use crate::domain::mixfix::Mixfix;

use super::ast::*;

// == Semantic equality

// - Identifiers

/// Checks equality of id
pub fn eq_id(id_a: &Id, id_b: &Id) -> bool {
    id_a.node == id_b.node
}

// - Atoms

/// Checks equality of atom
pub fn eq_atom(atom_a: &Atom, atom_b: &Atom) -> bool {
    atom_a.node == atom_b.node
}

/// Checks equality of atoms
pub fn eq_atoms(atoms_a: &[Atom], atoms_b: &[Atom]) -> bool {
    atoms_a.len() == atoms_b.len()
        && atoms_a
            .iter()
            .zip(atoms_b)
            .all(|(atom_a, atom_b)| eq_atom(atom_a, atom_b))
}

// - Mixfix operators

/// Checks equality of mixop
pub fn eq_mixop(mixop_a: &Mixop, mixop_b: &Mixop) -> bool {
    mixop_a == mixop_b
}

// - Iterators

/// Checks equality of iter
pub fn eq_iter(iter_a: Iter, iter_b: Iter) -> bool {
    iter_a == iter_b
}

/// Checks equality of iters
pub fn eq_iters(iters_a: &[Iter], iters_b: &[Iter]) -> bool {
    iters_a == iters_b
}

fn compare_iters(iters_a: &[Iter], iters_b: &[Iter]) -> Ordering {
    let rank = |iter: &Iter| match iter {
        Iter::Opt => 0,
        Iter::List => 1,
    };
    iters_a.iter().map(rank).cmp(iters_b.iter().map(rank))
}

// - Variables

/// Checks equality of var
pub fn eq_var(var_a: &Var, var_b: &Var) -> bool {
    eq_id(&var_a.id, &var_b.id) && var_a.iters == var_b.iters
}

/// Checks equality of vars
pub fn eq_vars(vars_a: &[Var], vars_b: &[Var]) -> bool {
    let mut vars_a = vars_a.iter().collect::<Vec<_>>();
    let mut vars_b = vars_b.iter().collect::<Vec<_>>();
    let compare = |var_a: &&Var, var_b: &&Var| {
        var_a
            .id
            .node
            .cmp(&var_b.id.node)
            .then_with(|| compare_iters(&var_a.iters, &var_b.iters))
    };
    vars_a.sort_by(compare);
    vars_b.sort_by(compare);
    vars_a.len() == vars_b.len()
        && vars_a
            .into_iter()
            .zip(vars_b)
            .all(|(var_a, var_b)| eq_var(var_a, var_b))
}

// - Types

/// Checks equality of typ
pub fn eq_typ(typ_a: &Typ, typ_b: &Typ) -> bool {
    match (&typ_a.node, &typ_b.node) {
        (TypKind::Bool, TypKind::Bool) | (TypKind::Text, TypKind::Text) => true,
        (TypKind::Num(num_typ_a), TypKind::Num(num_typ_b)) => num_typ_a == num_typ_b,
        (TypKind::Var(id_a, targs_a), TypKind::Var(id_b, targs_b)) => {
            eq_id(id_a, id_b) && eq_typs(targs_a, targs_b)
        }
        (TypKind::Tuple(typs_a), TypKind::Tuple(typs_b)) => eq_typs(typs_a, typs_b),
        (TypKind::Iter(typ_a, iter_a), TypKind::Iter(typ_b, iter_b)) => {
            eq_typ(typ_a, typ_b) && iter_a == iter_b
        }
        (TypKind::Func(tparams_a, typs_a, typ_a), TypKind::Func(tparams_b, typs_b, typ_b)) => {
            eq_tparams(tparams_a, tparams_b) && eq_typs(typs_a, typs_b) && eq_typ(typ_a, typ_b)
        }
        _ => false,
    }
}

/// Checks equality of typs
pub fn eq_typs(typs_a: &[Typ], typs_b: &[Typ]) -> bool {
    typs_a.len() == typs_b.len()
        && typs_a
            .iter()
            .zip(typs_b)
            .all(|(typ_a, typ_b)| eq_typ(typ_a, typ_b))
}

fn eq_mixfix_by<T, U>(
    mixfix_a: &Mixfix<T>,
    mixfix_b: &Mixfix<U>,
    mut eq_arg: impl FnMut(&T, &U) -> bool,
) -> bool {
    mixfix_a.eq_by(mixfix_b, |arg_a, arg_b| eq_arg(arg_a, arg_b))
}

/// Checks equality of not typ
pub fn eq_not_typ(not_typ_a: &NotTyp, not_typ_b: &NotTyp) -> bool {
    eq_mixfix_by(&not_typ_a.node, &not_typ_b.node, eq_typ)
}

// - Values

/// Checks equality of value
pub fn eq_value(value_a: &Value, value_b: &Value) -> bool {
    match (&value_a.node.kind, &value_b.node.kind) {
        (ValueKind::Bool(value_a), ValueKind::Bool(value_b)) => value_a == value_b,
        (ValueKind::Num(value_a), ValueKind::Num(value_b)) => value_a == value_b,
        (ValueKind::Text(value_a), ValueKind::Text(value_b)) => value_a == value_b,
        (ValueKind::Struct(fields_a), ValueKind::Struct(fields_b)) => {
            fields_a.len() == fields_b.len()
                && fields_a
                    .iter()
                    .zip(fields_b)
                    .all(|((atom_a, value_a), (atom_b, value_b))| {
                        eq_atom(atom_a, atom_b) && eq_value(value_a, value_b)
                    })
        }
        (ValueKind::Case(value_a), ValueKind::Case(value_b)) => {
            eq_mixfix_by(value_a, value_b, eq_value)
        }
        (ValueKind::Tuple(values_a), ValueKind::Tuple(values_b))
        | (ValueKind::List(values_a), ValueKind::List(values_b)) => eq_values(values_a, values_b),
        (ValueKind::Opt(Some(value_a)), ValueKind::Opt(Some(value_b))) => {
            eq_value(value_a, value_b)
        }
        (ValueKind::Opt(None), ValueKind::Opt(None)) => true,
        (ValueKind::Func(id_a), ValueKind::Func(id_b)) => id_a == id_b,
        (ValueKind::Extern(value_a), ValueKind::Extern(value_b)) => value_a == value_b,
        _ => false,
    }
}

/// Checks equality of values
pub fn eq_values(values_a: &[Value], values_b: &[Value]) -> bool {
    values_a.len() == values_b.len()
        && values_a
            .iter()
            .zip(values_b)
            .all(|(value_a, value_b)| eq_value(value_a, value_b))
}

// - Expressions

/// Checks equality of exp
pub fn eq_exp(exp_a: &Exp, exp_b: &Exp) -> bool {
    match (&exp_a.node.kind, &exp_b.node.kind) {
        (ExpKind::Bool(value_a), ExpKind::Bool(value_b)) => value_a == value_b,
        (ExpKind::Num(value_a), ExpKind::Num(value_b)) => value_a == value_b,
        (ExpKind::Text(value_a), ExpKind::Text(value_b)) => value_a == value_b,
        (ExpKind::Var(id_a), ExpKind::Var(id_b)) => eq_id(id_a, id_b),
        (ExpKind::Un(op_a, typ_a, exp_a), ExpKind::Un(op_b, typ_b, exp_b)) => {
            op_a == op_b && typ_a == typ_b && eq_exp(exp_a, exp_b)
        }
        (
            ExpKind::Bin(op_a, typ_a, left_a, right_a),
            ExpKind::Bin(op_b, typ_b, left_b, right_b),
        ) => op_a == op_b && typ_a == typ_b && eq_exp(left_a, left_b) && eq_exp(right_a, right_b),
        (
            ExpKind::Cmp(op_a, typ_a, left_a, right_a),
            ExpKind::Cmp(op_b, typ_b, left_b, right_b),
        ) => op_a == op_b && typ_a == typ_b && eq_exp(left_a, left_b) && eq_exp(right_a, right_b),
        (ExpKind::UpCast(typ_a, exp_a), ExpKind::UpCast(typ_b, exp_b))
        | (ExpKind::DownCast(typ_a, exp_a), ExpKind::DownCast(typ_b, exp_b)) => {
            eq_typ(typ_a, typ_b) && eq_exp(exp_a, exp_b)
        }
        (ExpKind::Sub(exp_a, typ_a, _), ExpKind::Sub(exp_b, typ_b, _)) => {
            eq_exp(exp_a, exp_b) && eq_typ(typ_a, typ_b)
        }
        (ExpKind::Match(exp_a, pattern_a), ExpKind::Match(exp_b, pattern_b)) => {
            eq_exp(exp_a, exp_b) && eq_pattern(pattern_a, pattern_b)
        }
        (ExpKind::Tuple(exps_a), ExpKind::Tuple(exps_b))
        | (ExpKind::List(exps_a), ExpKind::List(exps_b)) => eq_exps(exps_a, exps_b),
        (ExpKind::Case(not_exp_a), ExpKind::Case(not_exp_b)) => {
            eq_mixfix_by(not_exp_a, not_exp_b, eq_exp)
        }
        (ExpKind::Str(fields_a), ExpKind::Str(fields_b)) => {
            fields_a.len() == fields_b.len()
                && fields_a
                    .iter()
                    .zip(fields_b)
                    .all(|((atom_a, exp_a), (atom_b, exp_b))| {
                        eq_atom(atom_a, atom_b) && eq_exp(exp_a, exp_b)
                    })
        }
        (ExpKind::Opt(Some(exp_a)), ExpKind::Opt(Some(exp_b))) => eq_exp(exp_a, exp_b),
        (ExpKind::Opt(None), ExpKind::Opt(None)) => true,
        (ExpKind::Cons(head_a, tail_a), ExpKind::Cons(head_b, tail_b))
        | (ExpKind::Cat(head_a, tail_a), ExpKind::Cat(head_b, tail_b))
        | (ExpKind::Mem(head_a, tail_a), ExpKind::Mem(head_b, tail_b)) => {
            eq_exp(head_a, head_b) && eq_exp(tail_a, tail_b)
        }
        (ExpKind::Len(exp_a), ExpKind::Len(exp_b)) => eq_exp(exp_a, exp_b),
        (ExpKind::Dot(exp_a, atom_a), ExpKind::Dot(exp_b, atom_b)) => {
            eq_exp(exp_a, exp_b) && eq_atom(atom_a, atom_b)
        }
        (ExpKind::Idx(base_a, index_a), ExpKind::Idx(base_b, index_b)) => {
            eq_exp(base_a, base_b) && eq_exp(index_a, index_b)
        }
        (ExpKind::Slice(base_a, left_a, right_a), ExpKind::Slice(base_b, left_b, right_b)) => {
            eq_exp(base_a, base_b) && eq_exp(left_a, left_b) && eq_exp(right_a, right_b)
        }
        (ExpKind::Upd(base_a, path_a, value_a), ExpKind::Upd(base_b, path_b, value_b)) => {
            eq_exp(base_a, base_b) && eq_path(path_a, path_b) && eq_exp(value_a, value_b)
        }
        (ExpKind::Call(id_a, targs_a, args_a), ExpKind::Call(id_b, targs_b, args_b)) => {
            eq_id(id_a, id_b) && eq_typs(targs_a, targs_b) && eq_args(args_a, args_b)
        }
        (ExpKind::Iter(exp_a, iter_exp_a), ExpKind::Iter(exp_b, iter_exp_b)) => {
            eq_exp(exp_a, exp_b) && eq_iterexp(iter_exp_a, iter_exp_b)
        }
        _ => false,
    }
}

/// Checks equality of exps
pub fn eq_exps(exps_a: &[Exp], exps_b: &[Exp]) -> bool {
    exps_a.len() == exps_b.len()
        && exps_a
            .iter()
            .zip(exps_b)
            .all(|(exp_a, exp_b)| eq_exp(exp_a, exp_b))
}

/// Checks equality of iterexp
pub fn eq_iterexp(iter_exp_a: &IterExp, iter_exp_b: &IterExp) -> bool {
    iter_exp_a.0 == iter_exp_b.0 && eq_vars(&iter_exp_a.1, &iter_exp_b.1)
}

/// Checks equality of iterexps
pub fn eq_iterexps(iter_exps_a: &[IterExp], iter_exps_b: &[IterExp]) -> bool {
    iter_exps_a.len() == iter_exps_b.len()
        && iter_exps_a
            .iter()
            .zip(iter_exps_b)
            .all(|(iter_exp_a, iter_exp_b)| eq_iterexp(iter_exp_a, iter_exp_b))
}

// - Patterns

/// Checks equality of pattern
pub fn eq_pattern(pattern_a: &Pattern, pattern_b: &Pattern) -> bool {
    match (pattern_a, pattern_b) {
        (Pattern::Case(mixop_a), Pattern::Case(mixop_b)) => mixop_a == mixop_b,
        (Pattern::List(pattern_a), Pattern::List(pattern_b)) => pattern_a == pattern_b,
        (Pattern::Opt(pattern_a), Pattern::Opt(pattern_b)) => pattern_a == pattern_b,
        _ => false,
    }
}

// - Paths

/// Checks equality of path
pub fn eq_path(path_a: &Path, path_b: &Path) -> bool {
    match (&path_a.node.kind, &path_b.node.kind) {
        (PathKind::Root, PathKind::Root) => true,
        (PathKind::Idx(path_a, exp_a), PathKind::Idx(path_b, exp_b)) => {
            eq_path(path_a, path_b) && eq_exp(exp_a, exp_b)
        }
        (PathKind::Slice(path_a, left_a, right_a), PathKind::Slice(path_b, left_b, right_b)) => {
            eq_path(path_a, path_b) && eq_exp(left_a, left_b) && eq_exp(right_a, right_b)
        }
        (PathKind::Dot(path_a, atom_a), PathKind::Dot(path_b, atom_b)) => {
            eq_path(path_a, path_b) && eq_atom(atom_a, atom_b)
        }
        _ => false,
    }
}

// - Type parameters

/// Checks equality of tparam
pub fn eq_tparam(tparam_a: &TParam, tparam_b: &TParam) -> bool {
    eq_id(tparam_a, tparam_b)
}

/// Checks equality of tparams
pub fn eq_tparams(tparams_a: &[TParam], tparams_b: &[TParam]) -> bool {
    tparams_a.len() == tparams_b.len()
        && tparams_a
            .iter()
            .zip(tparams_b)
            .all(|(tparam_a, tparam_b)| tparam_a.node == tparam_b.node)
}

// - Arguments

/// Checks equality of arg
pub fn eq_arg(arg_a: &Arg, arg_b: &Arg) -> bool {
    match (&arg_a.node, &arg_b.node) {
        (ArgKind::Exp(exp_a), ArgKind::Exp(exp_b)) => eq_exp(exp_a, exp_b),
        (ArgKind::Def(id_a), ArgKind::Def(id_b)) => eq_id(id_a, id_b),
        _ => false,
    }
}

/// Checks equality of args
pub fn eq_args(args_a: &[Arg], args_b: &[Arg]) -> bool {
    args_a.len() == args_b.len()
        && args_a
            .iter()
            .zip(args_b)
            .all(|(arg_a, arg_b)| eq_arg(arg_a, arg_b))
}

// - Type arguments

/// Checks equality of targ
pub fn eq_targ(targ_a: &Targ, targ_b: &Targ) -> bool {
    eq_typ(targ_a, targ_b)
}

/// Checks equality of targs
pub fn eq_targs(targs_a: &[Targ], targs_b: &[Targ]) -> bool {
    targs_a.len() == targs_b.len()
        && targs_a
            .iter()
            .zip(targs_b)
            .all(|(targ_a, targ_b)| eq_targ(targ_a, targ_b))
}

// - Premises

/// Checks equality of prem
pub fn eq_prem(prem_a: &Prem, prem_b: &Prem) -> bool {
    match (&prem_a.node, &prem_b.node) {
        (
            PremKind::Rule(RulePrem {
                id: id_a,
                not_exp: not_exp_a,
                input_hint: input_hint_a,
            }),
            PremKind::Rule(RulePrem {
                id: id_b,
                not_exp: not_exp_b,
                input_hint: input_hint_b,
            }),
        ) => {
            eq_id(id_a, id_b)
                && eq_mixfix_by(not_exp_a, not_exp_b, eq_exp)
                && input_hint_a == input_hint_b
        }
        (PremKind::If(IfPrem { exp: exp_a }), PremKind::If(IfPrem { exp: exp_b }))
        | (PremKind::Debug(DebugPrem { exp: exp_a }), PremKind::Debug(DebugPrem { exp: exp_b })) => {
            eq_exp(exp_a, exp_b)
        }
        (
            PremKind::IfHold(IfHoldPrem {
                id: id_a,
                not_exp: not_exp_a,
            }),
            PremKind::IfHold(IfHoldPrem {
                id: id_b,
                not_exp: not_exp_b,
            }),
        )
        | (
            PremKind::IfNotHold(IfNotHoldPrem {
                id: id_a,
                not_exp: not_exp_a,
            }),
            PremKind::IfNotHold(IfNotHoldPrem {
                id: id_b,
                not_exp: not_exp_b,
            }),
        ) => eq_id(id_a, id_b) && eq_mixfix_by(not_exp_a, not_exp_b, eq_exp),
        (
            PremKind::Let(LetPrem {
                exp_l: exp_l_a,
                exp_r: exp_r_a,
            }),
            PremKind::Let(LetPrem {
                exp_l: exp_l_b,
                exp_r: exp_r_b,
            }),
        ) => eq_exp(exp_l_a, exp_l_b) && eq_exp(exp_r_a, exp_r_b),
        (
            PremKind::Iter(IteratedPrem {
                prem: prem_a,
                iter_prem: iter_prem_a,
            }),
            PremKind::Iter(IteratedPrem {
                prem: prem_b,
                iter_prem: iter_prem_b,
            }),
        ) => eq_prem(prem_a, prem_b) && eq_iterprem(iter_prem_a, iter_prem_b),
        _ => false,
    }
}

/// Checks equality of iterprem
pub fn eq_iterprem(iter_prem_a: &IterPrem, iter_prem_b: &IterPrem) -> bool {
    eq_iter(iter_prem_a.iter, iter_prem_b.iter)
        && eq_vars(&iter_prem_a.vars_bound, &iter_prem_b.vars_bound)
        && eq_vars(&iter_prem_a.vars_bind, &iter_prem_b.vars_bind)
}

/// Checks equality of iterprems
pub fn eq_iterprems(iter_prems_a: &[IterPrem], iter_prems_b: &[IterPrem]) -> bool {
    iter_prems_a.len() == iter_prems_b.len()
        && iter_prems_a
            .iter()
            .zip(iter_prems_b)
            .all(|(iter_prem_a, iter_prem_b)| eq_iterprem(iter_prem_a, iter_prem_b))
}
