use crate::lang::{hints::input, il};

use super::ast::*;

// Identifiers

pub fn eq_id(id_a: &Id, id_b: &Id) -> bool {
    il::eq::eq_id(id_a, id_b)
}

// Atoms

pub fn eq_atom(atom_a: &Atom, atom_b: &Atom) -> bool {
    il::eq::eq_atom(atom_a, atom_b)
}

pub fn eq_atoms(atoms_a: &[Atom], atoms_b: &[Atom]) -> bool {
    il::eq::eq_atoms(atoms_a, atoms_b)
}

// Mixfix operators

pub fn eq_mixop(mixop_a: &Mixop, mixop_b: &Mixop) -> bool {
    il::eq::eq_mixop(mixop_a, mixop_b)
}

// Iterators

pub fn eq_iter(iter_a: Iter, iter_b: Iter) -> bool {
    il::eq::eq_iter(iter_a, iter_b)
}

pub fn eq_iters(iters_a: &[Iter], iters_b: &[Iter]) -> bool {
    il::eq::eq_iters(iters_a, iters_b)
}

// Variables

pub fn eq_var(var_a: &Var, var_b: &Var) -> bool {
    il::eq::eq_var(var_a, var_b)
}

pub fn eq_vars(vars_a: &[Var], vars_b: &[Var]) -> bool {
    il::eq::eq_vars(vars_a, vars_b)
}

// Types

pub fn eq_typ(typ_a: &Typ, typ_b: &Typ) -> bool {
    il::eq::eq_typ(typ_a, typ_b)
}

pub fn eq_typs(typs_a: &[Typ], typs_b: &[Typ]) -> bool {
    il::eq::eq_typs(typs_a, typs_b)
}

// Expressions

pub fn eq_exp(exp_a: &Exp, exp_b: &Exp) -> bool {
    il::eq::eq_exp(exp_a, exp_b)
}

pub fn eq_exps(exps_a: &[Exp], exps_b: &[Exp]) -> bool {
    il::eq::eq_exps(exps_a, exps_b)
}

pub fn eq_iterexp(iterexp_a: &IterExp, iterexp_b: &IterExp) -> bool {
    il::eq::eq_iterexp(iterexp_a, iterexp_b)
}

pub fn eq_iterexps(iterexps_a: &[IterExp], iterexps_b: &[IterExp]) -> bool {
    il::eq::eq_iterexps(iterexps_a, iterexps_b)
}

// Patterns

pub fn eq_pattern(pattern_a: &Pattern, pattern_b: &Pattern) -> bool {
    il::eq::eq_pattern(pattern_a, pattern_b)
}

// Paths

pub fn eq_path(path_a: &Path, path_b: &Path) -> bool {
    il::eq::eq_path(path_a, path_b)
}

// Arguments

pub fn eq_arg(arg_a: &Arg, arg_b: &Arg) -> bool {
    il::eq::eq_arg(arg_a, arg_b)
}

pub fn eq_args(args_a: &[Arg], args_b: &[Arg]) -> bool {
    il::eq::eq_args(args_a, args_b)
}

// Type arguments

pub fn eq_targ(targ_a: &Targ, targ_b: &Targ) -> bool {
    il::eq::eq_targ(targ_a, targ_b)
}

pub fn eq_targs(targs_a: &[Targ], targs_b: &[Targ]) -> bool {
    il::eq::eq_targs(targs_a, targs_b)
}

// Holding case analysis

pub fn eq_holdcase(holdcase_a: &HoldCase, holdcase_b: &HoldCase) -> bool {
    match (holdcase_a, holdcase_b) {
        (
            HoldCase::BothH(block_hold_a, block_not_hold_a),
            HoldCase::BothH(block_hold_b, block_not_hold_b),
        ) => eq_block(block_hold_a, block_hold_b) && eq_block(block_not_hold_a, block_not_hold_b),
        (HoldCase::HoldH(block_a, dangle_a), HoldCase::HoldH(block_b, dangle_b))
        | (HoldCase::NotHoldH(block_a, dangle_a), HoldCase::NotHoldH(block_b, dangle_b)) => {
            eq_block(block_a, block_b) && dangle_a == dangle_b
        }
        _ => false,
    }
}

// Case analysis

pub fn eq_case(case_a: &Case, case_b: &Case) -> bool {
    eq_guard(&case_a.guard, &case_b.guard) && eq_block(&case_a.block, &case_b.block)
}

pub fn eq_cases(cases_a: &[Case], cases_b: &[Case]) -> bool {
    cases_a.len() == cases_b.len()
        && cases_a
            .iter()
            .zip(cases_b)
            .all(|(case_a, case_b)| eq_case(case_a, case_b))
}

pub fn eq_guard(guard_a: &Guard, guard_b: &Guard) -> bool {
    match (guard_a, guard_b) {
        (Guard::BoolG(value_a), Guard::BoolG(value_b)) => value_a == value_b,
        (Guard::CmpG(operation_a, type_a, exp_a), Guard::CmpG(operation_b, type_b, exp_b)) => {
            operation_a == operation_b && type_a == type_b && eq_exp(exp_a, exp_b)
        }
        (Guard::SubG(type_a, _), Guard::SubG(type_b, _)) => eq_typ(type_a, type_b),
        (Guard::MatchG(pattern_a), Guard::MatchG(pattern_b)) => eq_pattern(pattern_a, pattern_b),
        (Guard::MemG(exp_a), Guard::MemG(exp_b)) => eq_exp(exp_a, exp_b),
        _ => false,
    }
}

// Instructions

pub fn eq_instr(instr_a: &Instr, instr_b: &Instr) -> bool {
    match (&instr_a.kind, &instr_b.kind) {
        (
            InstrKind::IfI(exp_a, iterexps_a, block_a, dangle_a),
            InstrKind::IfI(exp_b, iterexps_b, block_b, dangle_b),
        ) => {
            eq_exp(exp_a, exp_b)
                && eq_iterexps(iterexps_a, iterexps_b)
                && eq_block(block_a, block_b)
                && dangle_a == dangle_b
        }
        (
            InstrKind::HoldI(id_a, notexp_a, iterexps_a, holdcase_a),
            InstrKind::HoldI(id_b, notexp_b, iterexps_b, holdcase_b),
        ) => {
            eq_id(id_a, id_b)
                && notexp_a.eq_by(notexp_b, eq_exp)
                && eq_iterexps(iterexps_a, iterexps_b)
                && eq_holdcase(holdcase_a, holdcase_b)
        }
        (
            InstrKind::CaseI(exp_a, cases_a, dangle_a),
            InstrKind::CaseI(exp_b, cases_b, dangle_b),
        ) => eq_exp(exp_a, exp_b) && eq_cases(cases_a, cases_b) && dangle_a == dangle_b,
        (
            InstrKind::GroupI(id_a, signature_a, exps_a, block_a),
            InstrKind::GroupI(id_b, signature_b, exps_b, block_b),
        ) => {
            eq_id(id_a, id_b)
                && eq_rel_signature(signature_a, signature_b)
                && eq_exps(exps_a, exps_b)
                && eq_block(block_a, block_b)
        }
        (
            InstrKind::LetI(exp_l_a, exp_r_a, iterinstrs_a, block_a),
            InstrKind::LetI(exp_l_b, exp_r_b, iterinstrs_b, block_b),
        ) => {
            eq_exp(exp_l_a, exp_l_b)
                && eq_exp(exp_r_a, exp_r_b)
                && eq_iterinstrs(iterinstrs_a, iterinstrs_b)
                && eq_block(block_a, block_b)
        }
        (
            InstrKind::RuleI(id_a, notexp_a, inputs_a, iterinstrs_a, block_a),
            InstrKind::RuleI(id_b, notexp_b, inputs_b, iterinstrs_b, block_b),
        ) => {
            eq_id(id_a, id_b)
                && notexp_a.eq_by(notexp_b, eq_exp)
                && input::eq(inputs_a, inputs_b)
                && eq_iterinstrs(iterinstrs_a, iterinstrs_b)
                && eq_block(block_a, block_b)
        }
        (InstrKind::ResultI(signature_a, exps_a), InstrKind::ResultI(signature_b, exps_b)) => {
            eq_rel_signature(signature_a, signature_b) && eq_exps(exps_a, exps_b)
        }
        (InstrKind::ReturnI(exp_a), InstrKind::ReturnI(exp_b)) => eq_exp(exp_a, exp_b),
        (InstrKind::DebugI(exp_a, instr_a), InstrKind::DebugI(exp_b, instr_b)) => {
            eq_exp(exp_a, exp_b) && eq_instr(instr_a, instr_b)
        }
        _ => false,
    }
}

pub fn eq_instrs(instrs_a: &[Instr], instrs_b: &[Instr]) -> bool {
    instrs_a.len() == instrs_b.len()
        && instrs_a
            .iter()
            .zip(instrs_b)
            .all(|(instr_a, instr_b)| eq_instr(instr_a, instr_b))
}

pub fn eq_block(block_a: &Block, block_b: &Block) -> bool {
    eq_instrs(block_a, block_b)
}

pub fn eq_elseblock(elseblock_a: &ElseBlock, elseblock_b: &ElseBlock) -> bool {
    eq_block(elseblock_a, elseblock_b)
}

pub fn eq_elseblock_opt(elseblock_a: &Option<ElseBlock>, elseblock_b: &Option<ElseBlock>) -> bool {
    match (elseblock_a, elseblock_b) {
        (Some(block_a), Some(block_b)) => eq_elseblock(block_a, block_b),
        (None, None) => true,
        _ => false,
    }
}

pub fn eq_iterinstr(iterinstr_a: &IterInstr, iterinstr_b: &IterInstr) -> bool {
    il::eq::eq_iterprem(iterinstr_a, iterinstr_b)
}

pub fn eq_iterinstrs(iterinstrs_a: &[IterInstr], iterinstrs_b: &[IterInstr]) -> bool {
    il::eq::eq_iterprems(iterinstrs_a, iterinstrs_b)
}

// Relations

pub fn eq_rel_signature(signature_a: &RelSignature, signature_b: &RelSignature) -> bool {
    il::eq::eq_nottyp(&signature_a.notation, &signature_b.notation)
        && input::eq(&signature_a.input_hint, &signature_b.input_hint)
}
