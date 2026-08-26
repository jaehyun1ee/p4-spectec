//! Partiality checks for prose-language data

use super::ast::*;

/// A construct is partial when its evaluation can fail because it invokes a
/// relation or function that may not match
pub fn is_partial_exp(exp: &Exp) -> bool {
    match &exp.node.node.kind {
        ExpKind::Bool(_) | ExpKind::Num(_) | ExpKind::Text(_) | ExpKind::Var(_) => false,
        ExpKind::Un(_, _, exp)
        | ExpKind::UpCast(_, exp)
        | ExpKind::DownCast(_, exp)
        | ExpKind::Sub(exp, _, _)
        | ExpKind::Match(exp, _)
        | ExpKind::Len(exp)
        | ExpKind::Dot(exp, _)
        | ExpKind::Iter(exp, _) => is_partial_exp(exp),
        ExpKind::Bin(_, _, exp_l, exp_r)
        | ExpKind::Cmp(_, _, exp_l, exp_r)
        | ExpKind::Cons(exp_l, exp_r)
        | ExpKind::Cat(exp_l, exp_r)
        | ExpKind::Mem(exp_l, exp_r)
        | ExpKind::Idx(exp_l, exp_r) => is_partial_exp(exp_l) || is_partial_exp(exp_r),
        ExpKind::Tuple(exps) | ExpKind::List(exps) => exps.iter().any(is_partial_exp),
        ExpKind::Case(not_exp) => not_exp.args().into_iter().any(is_partial_exp),
        ExpKind::Str(fields) => fields.iter().any(|(_, exp)| is_partial_exp(exp)),
        ExpKind::Opt(exp) => exp.as_deref().is_some_and(is_partial_exp),
        ExpKind::Slice(exp_b, exp_i, exp_n) => {
            is_partial_exp(exp_b) || is_partial_exp(exp_i) || is_partial_exp(exp_n)
        }
        ExpKind::Upd(exp_b, path, exp_f) => {
            is_partial_exp(exp_b) || is_partial_path(path) || is_partial_exp(exp_f)
        }
        ExpKind::Call(..) => true,
    }
}

/// Checks whether a path may fail during evaluation
pub fn is_partial_path(path: &Path) -> bool {
    match &path.node.kind {
        PathKind::Root => false,
        PathKind::Idx(path, exp_i) => is_partial_path(path) || is_partial_exp(exp_i),
        PathKind::Slice(path, exp_i, exp_n) => {
            is_partial_path(path) || is_partial_exp(exp_i) || is_partial_exp(exp_n)
        }
        PathKind::Dot(path, _) => is_partial_path(path),
    }
}

/// Checks whether a case may fail during evaluation
pub fn is_partial_case<Tier>(case: &Case<Tier>) -> bool {
    is_partial_guard(&case.guard)
}

/// Checks whether a guard may fail during evaluation
pub fn is_partial_guard(guard: &Guard) -> bool {
    match guard {
        Guard::Bool(_) | Guard::Sub(..) | Guard::Match(_) | Guard::Mem(_) => false,
        Guard::Cmp(_, _, exp) | Guard::CheckLetSub(_, _, exp) | Guard::CheckLetMatch(_, exp) => {
            is_partial_exp(exp)
        }
    }
}

/// Checks whether a group-tier instruction may fail during evaluation
pub fn is_partial_instr_group(instr: &InstrGroup) -> bool {
    match instr {
        InstrGroup::Rule(RuleGroupInstr { not_exp, .. }) => {
            not_exp.args().into_iter().any(is_partial_exp)
        }
        InstrGroup::Result(ResultGroupInstr { exps_output, .. }) => {
            exps_output.iter().any(is_partial_exp)
        }
        InstrGroup::Return(ReturnGroupInstr { exp }) => is_partial_exp(exp),
        InstrGroup::Backtrack(_) => false,
    }
}

/// Checks whether a dispatch-tier instruction may fail during evaluation
pub fn is_partial_instr_dispatch(instr: &InstrDispatch) -> bool {
    match instr {
        InstrDispatch::Group(_) | InstrDispatch::Route(_) => false,
    }
}

/// Checks whether an instruction may fail during evaluation
pub fn is_partial_instr<Tier>(
    is_partial_tier: impl Fn(&Tier) -> bool,
    instr: &Instr<Tier>,
) -> bool {
    match &instr.node.node.kind {
        InstrKind::If(IfInstr { exp, .. }) => is_partial_exp(exp),
        InstrKind::Hold(..) => true,
        InstrKind::Case(CaseInstr { exp, cases, .. }) => {
            is_partial_exp(exp) || cases.iter().any(is_partial_case)
        }
        InstrKind::Let(LetInstr { exp_r, .. }) => is_partial_exp(exp_r),
        InstrKind::Debug(DebugInstr { exp }) | InstrKind::Destruct(DestructInstr { exp, .. }) => {
            is_partial_exp(exp)
        }
        InstrKind::CheckLetSub(..) | InstrKind::CheckLetMatch(..) | InstrKind::OptionGet(..) => {
            true
        }
        InstrKind::Tier(TierInstr { tier }) => is_partial_tier(tier),
    }
}
