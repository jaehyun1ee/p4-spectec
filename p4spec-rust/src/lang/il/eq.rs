use std::cmp::Ordering;

use crate::domain::mixfix::Mixfix;

use super::ast::*;

// Identifiers

pub fn eq_id(id_a: &Id, id_b: &Id) -> bool {
    id_a.node == id_b.node
}

// Atoms

pub fn eq_atom(atom_a: &Atom, atom_b: &Atom) -> bool {
    atom_a.node == atom_b.node
}

pub fn eq_atoms(atoms_a: &[Atom], atoms_b: &[Atom]) -> bool {
    atoms_a.len() == atoms_b.len()
        && atoms_a
            .iter()
            .zip(atoms_b)
            .all(|(atom_a, atom_b)| eq_atom(atom_a, atom_b))
}

// Mixfix operators

pub fn eq_mixop(mixop_a: &Mixop, mixop_b: &Mixop) -> bool {
    mixop_a == mixop_b
}

// Iterators

pub fn eq_iter(iter_a: Iter, iter_b: Iter) -> bool {
    iter_a == iter_b
}

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

// Variables

pub fn eq_var(var_a: &Var, var_b: &Var) -> bool {
    eq_id(&var_a.id, &var_b.id) && var_a.iters == var_b.iters
}

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

// Types

pub fn eq_typ(typ_a: &Typ, typ_b: &Typ) -> bool {
    match (&typ_a.node, &typ_b.node) {
        (TypKind::BoolT, TypKind::BoolT) | (TypKind::TextT, TypKind::TextT) => true,
        (TypKind::NumT(numtyp_a), TypKind::NumT(numtyp_b)) => numtyp_a == numtyp_b,
        (TypKind::VarT(id_a, targs_a), TypKind::VarT(id_b, targs_b)) => {
            eq_id(id_a, id_b) && eq_typs(targs_a, targs_b)
        }
        (TypKind::TupleT(typs_a), TypKind::TupleT(typs_b)) => eq_typs(typs_a, typs_b),
        (TypKind::IterT(typ_a, iter_a), TypKind::IterT(typ_b, iter_b)) => {
            eq_typ(typ_a, typ_b) && iter_a == iter_b
        }
        (TypKind::FuncT(tparams_a, typs_a, typ_a), TypKind::FuncT(tparams_b, typs_b, typ_b)) => {
            eq_tparams(tparams_a, tparams_b) && eq_typs(typs_a, typs_b) && eq_typ(typ_a, typ_b)
        }
        _ => false,
    }
}

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

pub fn eq_nottyp(nottyp_a: &NotTyp, nottyp_b: &NotTyp) -> bool {
    eq_mixfix_by(&nottyp_a.node, &nottyp_b.node, eq_typ)
}

// Values

pub fn eq_value(value_a: &Value, value_b: &Value) -> bool {
    match (&value_a.kind, &value_b.kind) {
        (ValueKind::BoolV(value_a), ValueKind::BoolV(value_b)) => value_a == value_b,
        (ValueKind::NumV(value_a), ValueKind::NumV(value_b)) => value_a == value_b,
        (ValueKind::TextV(value_a), ValueKind::TextV(value_b)) => value_a == value_b,
        (ValueKind::StructV(fields_a), ValueKind::StructV(fields_b)) => {
            fields_a.len() == fields_b.len()
                && fields_a
                    .iter()
                    .zip(fields_b)
                    .all(|((atom_a, value_a), (atom_b, value_b))| {
                        eq_atom(atom_a, atom_b) && eq_value(value_a, value_b)
                    })
        }
        (ValueKind::CaseV(value_a), ValueKind::CaseV(value_b)) => {
            eq_mixfix_by(value_a, value_b, eq_value)
        }
        (ValueKind::TupleV(values_a), ValueKind::TupleV(values_b))
        | (ValueKind::ListV(values_a), ValueKind::ListV(values_b)) => eq_values(values_a, values_b),
        (ValueKind::OptV(Some(value_a)), ValueKind::OptV(Some(value_b))) => {
            eq_value(value_a, value_b)
        }
        (ValueKind::OptV(None), ValueKind::OptV(None)) => true,
        (ValueKind::FuncV(id_a), ValueKind::FuncV(id_b)) => id_a == id_b,
        (ValueKind::ExternV(value_a), ValueKind::ExternV(value_b)) => value_a == value_b,
        _ => false,
    }
}

pub fn eq_values(values_a: &[Value], values_b: &[Value]) -> bool {
    values_a.len() == values_b.len()
        && values_a
            .iter()
            .zip(values_b)
            .all(|(value_a, value_b)| eq_value(value_a, value_b))
}

// Expressions

pub fn eq_exp(exp_a: &Exp, exp_b: &Exp) -> bool {
    match (&exp_a.kind, &exp_b.kind) {
        (ExpKind::BoolE(value_a), ExpKind::BoolE(value_b)) => value_a == value_b,
        (ExpKind::NumE(value_a), ExpKind::NumE(value_b)) => value_a == value_b,
        (ExpKind::TextE(value_a), ExpKind::TextE(value_b)) => value_a == value_b,
        (ExpKind::VarE(id_a), ExpKind::VarE(id_b)) => eq_id(id_a, id_b),
        (ExpKind::UnE(op_a, typ_a, exp_a), ExpKind::UnE(op_b, typ_b, exp_b)) => {
            op_a == op_b && typ_a == typ_b && eq_exp(exp_a, exp_b)
        }
        (
            ExpKind::BinE(op_a, typ_a, left_a, right_a),
            ExpKind::BinE(op_b, typ_b, left_b, right_b),
        ) => op_a == op_b && typ_a == typ_b && eq_exp(left_a, left_b) && eq_exp(right_a, right_b),
        (
            ExpKind::CmpE(op_a, typ_a, left_a, right_a),
            ExpKind::CmpE(op_b, typ_b, left_b, right_b),
        ) => op_a == op_b && typ_a == typ_b && eq_exp(left_a, left_b) && eq_exp(right_a, right_b),
        (ExpKind::UpCastE(typ_a, exp_a), ExpKind::UpCastE(typ_b, exp_b))
        | (ExpKind::DownCastE(typ_a, exp_a), ExpKind::DownCastE(typ_b, exp_b)) => {
            eq_typ(typ_a, typ_b) && eq_exp(exp_a, exp_b)
        }
        (ExpKind::SubE(exp_a, typ_a, _), ExpKind::SubE(exp_b, typ_b, _)) => {
            eq_exp(exp_a, exp_b) && eq_typ(typ_a, typ_b)
        }
        (ExpKind::MatchE(exp_a, pattern_a), ExpKind::MatchE(exp_b, pattern_b)) => {
            eq_exp(exp_a, exp_b) && eq_pattern(pattern_a, pattern_b)
        }
        (ExpKind::TupleE(exps_a), ExpKind::TupleE(exps_b))
        | (ExpKind::ListE(exps_a), ExpKind::ListE(exps_b)) => eq_exps(exps_a, exps_b),
        (ExpKind::CaseE(notexp_a), ExpKind::CaseE(notexp_b)) => {
            eq_mixfix_by(notexp_a, notexp_b, eq_exp)
        }
        (ExpKind::StrE(fields_a), ExpKind::StrE(fields_b)) => {
            fields_a.len() == fields_b.len()
                && fields_a
                    .iter()
                    .zip(fields_b)
                    .all(|((atom_a, exp_a), (atom_b, exp_b))| {
                        eq_atom(atom_a, atom_b) && eq_exp(exp_a, exp_b)
                    })
        }
        (ExpKind::OptE(Some(exp_a)), ExpKind::OptE(Some(exp_b))) => eq_exp(exp_a, exp_b),
        (ExpKind::OptE(None), ExpKind::OptE(None)) => true,
        (ExpKind::ConsE(head_a, tail_a), ExpKind::ConsE(head_b, tail_b))
        | (ExpKind::CatE(head_a, tail_a), ExpKind::CatE(head_b, tail_b))
        | (ExpKind::MemE(head_a, tail_a), ExpKind::MemE(head_b, tail_b)) => {
            eq_exp(head_a, head_b) && eq_exp(tail_a, tail_b)
        }
        (ExpKind::LenE(exp_a), ExpKind::LenE(exp_b)) => eq_exp(exp_a, exp_b),
        (ExpKind::DotE(exp_a, atom_a), ExpKind::DotE(exp_b, atom_b)) => {
            eq_exp(exp_a, exp_b) && eq_atom(atom_a, atom_b)
        }
        (ExpKind::IdxE(base_a, index_a), ExpKind::IdxE(base_b, index_b)) => {
            eq_exp(base_a, base_b) && eq_exp(index_a, index_b)
        }
        (ExpKind::SliceE(base_a, left_a, right_a), ExpKind::SliceE(base_b, left_b, right_b)) => {
            eq_exp(base_a, base_b) && eq_exp(left_a, left_b) && eq_exp(right_a, right_b)
        }
        (ExpKind::UpdE(base_a, path_a, value_a), ExpKind::UpdE(base_b, path_b, value_b)) => {
            eq_exp(base_a, base_b) && eq_path(path_a, path_b) && eq_exp(value_a, value_b)
        }
        (ExpKind::CallE(id_a, targs_a, args_a), ExpKind::CallE(id_b, targs_b, args_b)) => {
            eq_id(id_a, id_b) && eq_typs(targs_a, targs_b) && eq_args(args_a, args_b)
        }
        (ExpKind::IterE(exp_a, iterexp_a), ExpKind::IterE(exp_b, iterexp_b)) => {
            eq_exp(exp_a, exp_b) && eq_iterexp(iterexp_a, iterexp_b)
        }
        _ => false,
    }
}

pub fn eq_exps(exps_a: &[Exp], exps_b: &[Exp]) -> bool {
    exps_a.len() == exps_b.len()
        && exps_a
            .iter()
            .zip(exps_b)
            .all(|(exp_a, exp_b)| eq_exp(exp_a, exp_b))
}

pub fn eq_iterexp(iterexp_a: &IterExp, iterexp_b: &IterExp) -> bool {
    iterexp_a.0 == iterexp_b.0 && eq_vars(&iterexp_a.1, &iterexp_b.1)
}

pub fn eq_iterexps(iterexps_a: &[IterExp], iterexps_b: &[IterExp]) -> bool {
    iterexps_a.len() == iterexps_b.len()
        && iterexps_a
            .iter()
            .zip(iterexps_b)
            .all(|(iterexp_a, iterexp_b)| eq_iterexp(iterexp_a, iterexp_b))
}

// Patterns

pub fn eq_pattern(pattern_a: &Pattern, pattern_b: &Pattern) -> bool {
    match (pattern_a, pattern_b) {
        (Pattern::CaseP(mixop_a), Pattern::CaseP(mixop_b)) => mixop_a == mixop_b,
        (Pattern::ListP(pattern_a), Pattern::ListP(pattern_b)) => pattern_a == pattern_b,
        (Pattern::OptP(pattern_a), Pattern::OptP(pattern_b)) => pattern_a == pattern_b,
        _ => false,
    }
}

// Paths

pub fn eq_path(path_a: &Path, path_b: &Path) -> bool {
    match (&path_a.kind, &path_b.kind) {
        (PathKind::RootP, PathKind::RootP) => true,
        (PathKind::IdxP(path_a, exp_a), PathKind::IdxP(path_b, exp_b)) => {
            eq_path(path_a, path_b) && eq_exp(exp_a, exp_b)
        }
        (PathKind::SliceP(path_a, left_a, right_a), PathKind::SliceP(path_b, left_b, right_b)) => {
            eq_path(path_a, path_b) && eq_exp(left_a, left_b) && eq_exp(right_a, right_b)
        }
        (PathKind::DotP(path_a, atom_a), PathKind::DotP(path_b, atom_b)) => {
            eq_path(path_a, path_b) && eq_atom(atom_a, atom_b)
        }
        _ => false,
    }
}

// Type parameters

pub fn eq_tparam(tparam_a: &TParam, tparam_b: &TParam) -> bool {
    eq_id(tparam_a, tparam_b)
}

pub fn eq_tparams(tparams_a: &[TParam], tparams_b: &[TParam]) -> bool {
    tparams_a.len() == tparams_b.len()
        && tparams_a
            .iter()
            .zip(tparams_b)
            .all(|(tparam_a, tparam_b)| tparam_a.node == tparam_b.node)
}

// Arguments

pub fn eq_arg(arg_a: &Arg, arg_b: &Arg) -> bool {
    match (&arg_a.node, &arg_b.node) {
        (ArgKind::ExpA(exp_a), ArgKind::ExpA(exp_b)) => eq_exp(exp_a, exp_b),
        (ArgKind::DefA(id_a), ArgKind::DefA(id_b)) => eq_id(id_a, id_b),
        _ => false,
    }
}

pub fn eq_args(args_a: &[Arg], args_b: &[Arg]) -> bool {
    args_a.len() == args_b.len()
        && args_a
            .iter()
            .zip(args_b)
            .all(|(arg_a, arg_b)| eq_arg(arg_a, arg_b))
}

// Type arguments

pub fn eq_targ(targ_a: &Targ, targ_b: &Targ) -> bool {
    eq_typ(targ_a, targ_b)
}

pub fn eq_targs(targs_a: &[Targ], targs_b: &[Targ]) -> bool {
    targs_a.len() == targs_b.len()
        && targs_a
            .iter()
            .zip(targs_b)
            .all(|(targ_a, targ_b)| eq_targ(targ_a, targ_b))
}

// Premises

pub fn eq_prem(prem_a: &Prem, prem_b: &Prem) -> bool {
    match (&prem_a.node, &prem_b.node) {
        (PremKind::RulePr(id_a, exp_a, input_a), PremKind::RulePr(id_b, exp_b, input_b)) => {
            eq_id(id_a, id_b) && eq_mixfix_by(exp_a, exp_b, eq_exp) && input_a == input_b
        }
        (PremKind::IfPr(exp_a), PremKind::IfPr(exp_b))
        | (PremKind::DebugPr(exp_a), PremKind::DebugPr(exp_b)) => eq_exp(exp_a, exp_b),
        (PremKind::IfHoldPr(id_a, exp_a), PremKind::IfHoldPr(id_b, exp_b))
        | (PremKind::IfNotHoldPr(id_a, exp_a), PremKind::IfNotHoldPr(id_b, exp_b)) => {
            eq_id(id_a, id_b) && eq_mixfix_by(exp_a, exp_b, eq_exp)
        }
        (PremKind::LetPr(left_a, right_a), PremKind::LetPr(left_b, right_b)) => {
            eq_exp(left_a, left_b) && eq_exp(right_a, right_b)
        }
        (PremKind::IterPr(prem_a, iter_a), PremKind::IterPr(prem_b, iter_b)) => {
            eq_prem(prem_a, prem_b) && eq_iterprem(iter_a, iter_b)
        }
        _ => false,
    }
}

pub fn eq_iterprem(iterprem_a: &IterPrem, iterprem_b: &IterPrem) -> bool {
    eq_iter(iterprem_a.iter, iterprem_b.iter)
        && eq_vars(&iterprem_a.vars_bound, &iterprem_b.vars_bound)
        && eq_vars(&iterprem_a.vars_bind, &iterprem_b.vars_bind)
}

pub fn eq_iterprems(iterprems_a: &[IterPrem], iterprems_b: &[IterPrem]) -> bool {
    iterprems_a.len() == iterprems_b.len()
        && iterprems_a
            .iter()
            .zip(iterprems_b)
            .all(|(iterprem_a, iterprem_b)| eq_iterprem(iterprem_a, iterprem_b))
}
