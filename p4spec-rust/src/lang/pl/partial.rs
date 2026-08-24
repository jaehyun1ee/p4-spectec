use super::ast::*;

/// A construct is partial when its evaluation can fail because it invokes a
/// relation or function that may not match
pub fn is_partial_exp(exp: &Exp) -> bool {
    match &exp.node.kind {
        ExpKind::BoolE(_) | ExpKind::NumE(_) | ExpKind::TextE(_) | ExpKind::VarE(_) => false,
        ExpKind::UnE(_, _, exp)
        | ExpKind::UpCastE(_, exp)
        | ExpKind::DownCastE(_, exp)
        | ExpKind::SubE(exp, _, _)
        | ExpKind::MatchE(exp, _)
        | ExpKind::LenE(exp)
        | ExpKind::DotE(exp, _)
        | ExpKind::IterE(exp, _) => is_partial_exp(exp),
        ExpKind::BinE(_, _, exp_l, exp_r)
        | ExpKind::CmpE(_, _, exp_l, exp_r)
        | ExpKind::ConsE(exp_l, exp_r)
        | ExpKind::CatE(exp_l, exp_r)
        | ExpKind::MemE(exp_l, exp_r)
        | ExpKind::IdxE(exp_l, exp_r) => is_partial_exp(exp_l) || is_partial_exp(exp_r),
        ExpKind::TupleE(exps) | ExpKind::ListE(exps) => exps.iter().any(is_partial_exp),
        ExpKind::CaseE(notexp) => notexp.args().into_iter().any(is_partial_exp),
        ExpKind::StrE(fields) => fields.iter().any(|(_, exp)| is_partial_exp(exp)),
        ExpKind::OptE(exp) => exp.as_deref().is_some_and(is_partial_exp),
        ExpKind::SliceE(exp_b, exp_l, exp_h) => {
            is_partial_exp(exp_b) || is_partial_exp(exp_l) || is_partial_exp(exp_h)
        }
        ExpKind::UpdE(exp_b, path, exp_f) => {
            is_partial_exp(exp_b) || is_partial_path(path) || is_partial_exp(exp_f)
        }
        ExpKind::CallE(..) => true,
    }
}

pub fn is_partial_path(path: &Path) -> bool {
    match &path.kind {
        PathKind::RootP => false,
        PathKind::IdxP(path, exp) => is_partial_path(path) || is_partial_exp(exp),
        PathKind::SliceP(path, exp_l, exp_h) => {
            is_partial_path(path) || is_partial_exp(exp_l) || is_partial_exp(exp_h)
        }
        PathKind::DotP(path, _) => is_partial_path(path),
    }
}

pub fn is_partial_case<Tier>(case: &Case<Tier>) -> bool {
    is_partial_guard(&case.guard)
}

pub fn is_partial_guard(guard: &Guard) -> bool {
    match guard {
        Guard::BoolG(_) | Guard::SubG(..) | Guard::MatchG(_) | Guard::MemG(_) => false,
        Guard::CmpG(_, _, exp) | Guard::CheckLetSubG(_, _, exp) | Guard::CheckLetMatchG(_, exp) => {
            is_partial_exp(exp)
        }
    }
}

pub fn is_partial_instr_group(instr: &InstrGroup) -> bool {
    match instr {
        InstrGroup::RuleI { notation, .. } => notation.args().into_iter().any(is_partial_exp),
        InstrGroup::ResultI { outputs, .. } => outputs.iter().any(is_partial_exp),
        InstrGroup::ReturnI(exp) => is_partial_exp(exp),
        InstrGroup::BacktrackI(_) => false,
    }
}

pub fn is_partial_instr_dispatch(instr: &InstrDispatch) -> bool {
    match instr {
        InstrDispatch::GroupI { .. } | InstrDispatch::RouteI(_) => false,
    }
}

pub fn is_partial_instr<Tier>(
    is_partial_tier: impl Fn(&Tier) -> bool,
    instr: &Instr<Tier>,
) -> bool {
    match &instr.node.kind {
        InstrKind::IfI(exp, _, _, _) => is_partial_exp(exp),
        InstrKind::HoldI(..) => true,
        InstrKind::CaseI(exp, cases, _) => is_partial_exp(exp) || cases.iter().any(is_partial_case),
        InstrKind::LetI(_, exp_r, _) => is_partial_exp(exp_r),
        InstrKind::DebugI(exp) | InstrKind::DestructI(_, exp) => is_partial_exp(exp),
        InstrKind::CheckLetSubI(..) | InstrKind::CheckLetMatchI(..) | InstrKind::OptionGetI(..) => {
            true
        }
        InstrKind::TierI(tier) => is_partial_tier(tier),
    }
}
