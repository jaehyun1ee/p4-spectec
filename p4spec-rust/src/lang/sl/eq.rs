//! Semantic equality for structured-language data
//!
//! Ignores source regions;
//! compares relation hints and instruction identifiers

use crate::lang::{hints::input, il};

use super::ast::*;

// == Semantic equality

// - Identifiers

/// Checks equality of id
pub fn eq_id(id_a: &Id, id_b: &Id) -> bool {
    il::eq::eq_id(id_a, id_b)
}

// - Atoms

/// Checks equality of atom
pub fn eq_atom(atom_a: &Atom, atom_b: &Atom) -> bool {
    il::eq::eq_atom(atom_a, atom_b)
}

/// Checks equality of atoms
pub fn eq_atoms(atoms_a: &[Atom], atoms_b: &[Atom]) -> bool {
    il::eq::eq_atoms(atoms_a, atoms_b)
}

// - Mixfix operators

/// Checks equality of mixop
pub fn eq_mixop(mixop_a: &Mixop, mixop_b: &Mixop) -> bool {
    il::eq::eq_mixop(mixop_a, mixop_b)
}

// - Iterators

/// Checks equality of iter
pub fn eq_iter(iter_a: Iter, iter_b: Iter) -> bool {
    il::eq::eq_iter(iter_a, iter_b)
}

/// Checks equality of iters
pub fn eq_iters(iters_a: &[Iter], iters_b: &[Iter]) -> bool {
    il::eq::eq_iters(iters_a, iters_b)
}

// - Variables

/// Checks equality of var
pub fn eq_var(var_a: &Var, var_b: &Var) -> bool {
    il::eq::eq_var(var_a, var_b)
}

/// Checks equality of vars
pub fn eq_vars(vars_a: &[Var], vars_b: &[Var]) -> bool {
    il::eq::eq_vars(vars_a, vars_b)
}

// - Types

/// Checks equality of typ
pub fn eq_typ(typ_a: &Typ, typ_b: &Typ) -> bool {
    il::eq::eq_typ(typ_a, typ_b)
}

/// Checks equality of typs
pub fn eq_typs(typs_a: &[Typ], typs_b: &[Typ]) -> bool {
    il::eq::eq_typs(typs_a, typs_b)
}

// - Expressions

/// Checks equality of exp
pub fn eq_exp(exp_a: &Exp, exp_b: &Exp) -> bool {
    il::eq::eq_exp(exp_a, exp_b)
}

/// Checks equality of exps
pub fn eq_exps(exps_a: &[Exp], exps_b: &[Exp]) -> bool {
    il::eq::eq_exps(exps_a, exps_b)
}

/// Checks equality of iterexp
pub fn eq_iterexp(iter_exp_a: &IterExp, iter_exp_b: &IterExp) -> bool {
    il::eq::eq_iterexp(iter_exp_a, iter_exp_b)
}

/// Checks equality of iterexps
pub fn eq_iterexps(iter_exps_a: &[IterExp], iter_exps_b: &[IterExp]) -> bool {
    il::eq::eq_iterexps(iter_exps_a, iter_exps_b)
}

// - Patterns

/// Checks equality of pattern
pub fn eq_pattern(pattern_a: &Pattern, pattern_b: &Pattern) -> bool {
    il::eq::eq_pattern(pattern_a, pattern_b)
}

// - Paths

/// Checks equality of path
pub fn eq_path(path_a: &Path, path_b: &Path) -> bool {
    il::eq::eq_path(path_a, path_b)
}

// - Arguments

/// Checks equality of arg
pub fn eq_arg(arg_a: &Arg, arg_b: &Arg) -> bool {
    il::eq::eq_arg(arg_a, arg_b)
}

/// Checks equality of args
pub fn eq_args(args_a: &[Arg], args_b: &[Arg]) -> bool {
    il::eq::eq_args(args_a, args_b)
}

// - Type arguments

/// Checks equality of targ
pub fn eq_targ(targ_a: &Targ, targ_b: &Targ) -> bool {
    il::eq::eq_targ(targ_a, targ_b)
}

/// Checks equality of targs
pub fn eq_targs(targs_a: &[Targ], targs_b: &[Targ]) -> bool {
    il::eq::eq_targs(targs_a, targs_b)
}

// - Holding case analysis

/// Checks equality of holdcase
pub fn eq_holdcase(hold_case_a: &HoldCase, hold_case_b: &HoldCase) -> bool {
    match (hold_case_a, hold_case_b) {
        (
            HoldCase::Both(block_hold_a, block_not_hold_a),
            HoldCase::Both(block_hold_b, block_not_hold_b),
        ) => eq_block(block_hold_a, block_hold_b) && eq_block(block_not_hold_a, block_not_hold_b),
        (HoldCase::Hold(block_a, dangle_a), HoldCase::Hold(block_b, dangle_b))
        | (HoldCase::NotHold(block_a, dangle_a), HoldCase::NotHold(block_b, dangle_b)) => {
            eq_block(block_a, block_b) && dangle_a == dangle_b
        }
        _ => false,
    }
}

// - Case analysis

/// Checks equality of case
pub fn eq_case(case_a: &Case, case_b: &Case) -> bool {
    eq_guard(&case_a.guard, &case_b.guard) && eq_block(&case_a.block, &case_b.block)
}

/// Checks equality of cases
pub fn eq_cases(cases_a: &[Case], cases_b: &[Case]) -> bool {
    cases_a.len() == cases_b.len()
        && cases_a
            .iter()
            .zip(cases_b)
            .all(|(case_a, case_b)| eq_case(case_a, case_b))
}

/// Checks equality of guard
pub fn eq_guard(guard_a: &Guard, guard_b: &Guard) -> bool {
    match (guard_a, guard_b) {
        (Guard::Bool(value_a), Guard::Bool(value_b)) => value_a == value_b,
        (Guard::Cmp(op_a, typ_a, exp_a), Guard::Cmp(op_b, typ_b, exp_b)) => {
            op_a == op_b && typ_a == typ_b && eq_exp(exp_a, exp_b)
        }
        (Guard::Sub(typ_a, _), Guard::Sub(typ_b, _)) => eq_typ(typ_a, typ_b),
        (Guard::Match(pattern_a), Guard::Match(pattern_b)) => eq_pattern(pattern_a, pattern_b),
        (Guard::Mem(exp_a), Guard::Mem(exp_b)) => eq_exp(exp_a, exp_b),
        _ => false,
    }
}

// - Instructions

/// Checks equality of instr
pub fn eq_instr(instr_a: &Instr, instr_b: &Instr) -> bool {
    match (&instr_a.node.kind, &instr_b.node.kind) {
        (
            InstrKind::If(IfInstr {
                exp: exp_a,
                iter_exps: iter_exps_a,
                block: block_a,
                dangle: dangle_a,
            }),
            InstrKind::If(IfInstr {
                exp: exp_b,
                iter_exps: iter_exps_b,
                block: block_b,
                dangle: dangle_b,
            }),
        ) => {
            eq_exp(exp_a, exp_b)
                && eq_iterexps(iter_exps_a, iter_exps_b)
                && eq_block(block_a, block_b)
                && dangle_a == dangle_b
        }
        (
            InstrKind::Hold(HoldInstr {
                id: id_a,
                not_exp: not_exp_a,
                iter_exps: iter_exps_a,
                hold_case: hold_case_a,
            }),
            InstrKind::Hold(HoldInstr {
                id: id_b,
                not_exp: not_exp_b,
                iter_exps: iter_exps_b,
                hold_case: hold_case_b,
            }),
        ) => {
            eq_id(id_a, id_b)
                && not_exp_a.eq_by(not_exp_b, eq_exp)
                && eq_iterexps(iter_exps_a, iter_exps_b)
                && eq_holdcase(hold_case_a, hold_case_b)
        }
        (
            InstrKind::Case(CaseInstr {
                exp: exp_a,
                cases: cases_a,
                dangle: dangle_a,
            }),
            InstrKind::Case(CaseInstr {
                exp: exp_b,
                cases: cases_b,
                dangle: dangle_b,
            }),
        ) => eq_exp(exp_a, exp_b) && eq_cases(cases_a, cases_b) && dangle_a == dangle_b,
        (
            InstrKind::Group(GroupInstr {
                id: id_a,
                rel_signature: rel_signature_a,
                exps: exps_a,
                block: block_a,
            }),
            InstrKind::Group(GroupInstr {
                id: id_b,
                rel_signature: rel_signature_b,
                exps: exps_b,
                block: block_b,
            }),
        ) => {
            eq_id(id_a, id_b)
                && eq_rel_signature(rel_signature_a, rel_signature_b)
                && eq_exps(exps_a, exps_b)
                && eq_block(block_a, block_b)
        }
        (
            InstrKind::Let(LetInstr {
                exp_l: exp_l_a,
                exp_r: exp_r_a,
                iter_instrs: iter_instrs_a,
                block: block_a,
            }),
            InstrKind::Let(LetInstr {
                exp_l: exp_l_b,
                exp_r: exp_r_b,
                iter_instrs: iter_instrs_b,
                block: block_b,
            }),
        ) => {
            eq_exp(exp_l_a, exp_l_b)
                && eq_exp(exp_r_a, exp_r_b)
                && eq_iterinstrs(iter_instrs_a, iter_instrs_b)
                && eq_block(block_a, block_b)
        }
        (
            InstrKind::Rule(RuleInstr {
                id: id_a,
                not_exp: not_exp_a,
                input_hint: inputs_a,
                iter_instrs: iter_instrs_a,
                block: block_a,
            }),
            InstrKind::Rule(RuleInstr {
                id: id_b,
                not_exp: not_exp_b,
                input_hint: inputs_b,
                iter_instrs: iter_instrs_b,
                block: block_b,
            }),
        ) => {
            eq_id(id_a, id_b)
                && not_exp_a.eq_by(not_exp_b, eq_exp)
                && input::eq(inputs_a, inputs_b)
                && eq_iterinstrs(iter_instrs_a, iter_instrs_b)
                && eq_block(block_a, block_b)
        }
        (
            InstrKind::Result(ResultInstr {
                rel_signature: rel_signature_a,
                exps: exps_a,
            }),
            InstrKind::Result(ResultInstr {
                rel_signature: rel_signature_b,
                exps: exps_b,
            }),
        ) => eq_rel_signature(rel_signature_a, rel_signature_b) && eq_exps(exps_a, exps_b),
        (
            InstrKind::Return(ReturnInstr { exp: exp_a }),
            InstrKind::Return(ReturnInstr { exp: exp_b }),
        ) => eq_exp(exp_a, exp_b),
        (
            InstrKind::Debug(DebugInstr {
                exp: exp_a,
                instr: instr_a,
            }),
            InstrKind::Debug(DebugInstr {
                exp: exp_b,
                instr: instr_b,
            }),
        ) => eq_exp(exp_a, exp_b) && eq_instr(instr_a, instr_b),
        _ => false,
    }
}

/// Checks equality of instrs
pub fn eq_instrs(instrs_a: &[Instr], instrs_b: &[Instr]) -> bool {
    instrs_a.len() == instrs_b.len()
        && instrs_a
            .iter()
            .zip(instrs_b)
            .all(|(instr_a, instr_b)| eq_instr(instr_a, instr_b))
}

/// Checks equality of block
pub fn eq_block(block_a: &Block, block_b: &Block) -> bool {
    eq_instrs(block_a, block_b)
}

/// Checks equality of elseblock
pub fn eq_elseblock(else_block_a: &ElseBlock, else_block_b: &ElseBlock) -> bool {
    eq_block(else_block_a, else_block_b)
}

/// Checks equality of elseblock opt
pub fn eq_elseblock_opt(
    else_block_a: &Option<ElseBlock>,
    else_block_b: &Option<ElseBlock>,
) -> bool {
    match (else_block_a, else_block_b) {
        (Some(block_a), Some(block_b)) => eq_elseblock(block_a, block_b),
        (None, None) => true,
        _ => false,
    }
}

/// Checks equality of iterinstr
pub fn eq_iterinstr(iter_instr_a: &IterInstr, iter_instr_b: &IterInstr) -> bool {
    il::eq::eq_iterprem(iter_instr_a, iter_instr_b)
}

/// Checks equality of iterinstrs
pub fn eq_iterinstrs(iter_instrs_a: &[IterInstr], iter_instrs_b: &[IterInstr]) -> bool {
    il::eq::eq_iterprems(iter_instrs_a, iter_instrs_b)
}

// - Relations

/// Checks equality of rel signature
pub fn eq_rel_signature(rel_signature_a: &RelSignature, rel_signature_b: &RelSignature) -> bool {
    il::eq::eq_not_typ(&rel_signature_a.not_typ, &rel_signature_b.not_typ)
        && input::eq(&rel_signature_a.input_hint, &rel_signature_b.input_hint)
}
