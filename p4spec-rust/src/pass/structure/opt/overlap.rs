//! Best-effort syntactic condition overlap

use crate::pass::structure::error::{StructureError, StructureErrorKind};
use crate::{
    lang::{
        common::source::Span, il::ast::*, sl::ast::Guard, traits::eq::SyntaxEq, xl::bool as boolop,
    },
    runtime::{envs::algo::TDEnv, ops::typ::expand_typ, typdef::TypeDef},
};

pub(crate) fn exp_as_guard(exp_target: &Exp, exp_cond: &Exp) -> Option<Guard> {
    match &exp_cond.node {
        ExpKind::Un(UnOp::Bool(boolop::UnOp::Not), _, exp)
            if exp_target.syntax_eq(exp.as_ref()) =>
        {
            Some(Guard::Bool(false))
        }
        ExpKind::Cmp(op, optyp, exp_l, exp_r) => {
            exp_as_cmp_guard(exp_target, *op, *optyp, exp_l, exp_r)
        }
        ExpKind::Sub(exp, typ, subcheck) if exp_target.syntax_eq(exp.as_ref()) => {
            Some(Guard::Sub(typ.as_ref().clone(), subcheck.clone()))
        }
        ExpKind::Match(exp, pattern) if exp_target.syntax_eq(exp.as_ref()) => {
            Some(Guard::Match(pattern.clone()))
        }
        ExpKind::Mem(exp_elem, exp_set) if exp_target.syntax_eq(exp_elem.as_ref()) => {
            Some(Guard::Mem(exp_set.as_ref().clone()))
        }
        _ => None,
    }
}

fn exp_as_cmp_guard(
    exp_target: &Exp,
    op: CmpOp,
    optyp: OpTyp,
    exp_l: &Exp,
    exp_r: &Exp,
) -> Option<Guard> {
    if !matches!(op, CmpOp::Bool(boolop::CmpOp::Eq | boolop::CmpOp::Ne)) {
        return None;
    }
    if exp_target.syntax_eq(exp_l) {
        Some(Guard::Cmp(op, optyp, exp_r.clone()))
    } else if exp_target.syntax_eq(exp_r) {
        Some(Guard::Cmp(op, optyp, exp_l.clone()))
    } else {
        None
    }
}

pub(crate) fn guard_as_exp(exp_target: &Exp, guard: &Guard) -> Exp {
    let exp_kind = match guard {
        Guard::Bool(true) => return exp_target.clone(),
        Guard::Bool(false) => ExpKind::Un(
            UnOp::Bool(boolop::UnOp::Not),
            OpTyp::Bool,
            Box::new(exp_target.clone()),
        ),
        Guard::Cmp(op, optyp, exp) => ExpKind::Cmp(
            *op,
            *optyp,
            Box::new(exp_target.clone()),
            Box::new(exp.clone()),
        ),
        Guard::Sub(typ, subcheck) => ExpKind::Sub(
            Box::new(exp_target.clone()),
            Box::new(typ.clone()),
            subcheck.clone(),
        ),
        Guard::Match(pattern) => ExpKind::Match(Box::new(exp_target.clone()), pattern.clone()),
        Guard::Mem(exp) => ExpKind::Mem(Box::new(exp_target.clone()), Box::new(exp.clone())),
    };
    crate::note_phrase!(node: exp_kind, note: TypKind::Bool, span: exp_target.span.clone())
}

pub(crate) fn typ_as_variant(
    tdenv: &TDEnv,
    typ: &Typ,
) -> Result<Option<Vec<Mixop>>, StructureError> {
    let typ_unrolled = expand_typ(tdenv, typ)?;
    let TypKind::Var(id, _) = &typ_unrolled.node else {
        return Ok(None);
    };
    match tdenv.get(id) {
        Some(TypeDef::Defined(_, deftyp)) => Ok(match &deftyp.node {
            DefTypKind::Variant(typcases) => Some(
                typcases
                    .iter()
                    .map(|(nottyp, _, _)| nottyp.node.to_mixop())
                    .collect(),
            ),
            _ => None,
        }),
        _ => Ok(None),
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Overlap {
    Identical,
    Disjoint {
        exp: Exp,
        guard_a: Guard,
        guard_b: Guard,
    },
    Partition {
        exp: Exp,
        guard_a: Guard,
        guard_b: Guard,
    },
    Fuzzy,
}

fn partition_exp_literal(exp_a: &Exp, exp_b: &Exp) -> bool {
    matches!((&exp_a.node, &exp_b.node), (ExpKind::Bool(bool_a), ExpKind::Bool(bool_b)) if bool_a != bool_b)
}

fn disjoint_exp_literal(exp_a: &Exp, exp_b: &Exp) -> Result<bool, StructureError> {
    match (&exp_a.node, &exp_b.node) {
        (ExpKind::Bool(bool_a), ExpKind::Bool(bool_b)) => Ok(bool_a != bool_b),
        (ExpKind::Num(num_a), ExpKind::Num(num_b)) => Ok(num_a != num_b),
        (ExpKind::Text(text_a), ExpKind::Text(text_b)) => Ok(text_a != text_b),
        (ExpKind::UpCast(typ_a, exp_a), ExpKind::UpCast(typ_b, exp_b))
            if typ_a.syntax_eq(typ_b) =>
        {
            disjoint_exp_literal(exp_a, exp_b)
        }
        (ExpKind::Tuple(exps_a), ExpKind::Tuple(exps_b)) => disjoint_exps_literal(
            &exps_a.iter().collect::<Vec<_>>(),
            &exps_b.iter().collect::<Vec<_>>(),
            &exp_a.span,
        ),
        (ExpKind::Case(notexp_a), ExpKind::Case(notexp_b)) => {
            disjoint_notexp_literal(notexp_a, notexp_b, &exp_a.span)
        }
        (ExpKind::List(exps_a), ExpKind::List(exps_b)) => {
            disjoint_list_exp_literal(exps_a, exps_b, &exp_a.span)
        }
        _ => Ok(false),
    }
}

fn disjoint_exps_literal(
    exps_a: &[&Exp],
    exps_b: &[&Exp],
    span: &Span,
) -> Result<bool, StructureError> {
    if exps_a.len() != exps_b.len() {
        return Err(StructureError::new(
            StructureErrorKind::ArityMismatch {
                expected: exps_a.len(),
                actual: exps_b.len(),
            },
            span.clone(),
        ));
    }
    for (exp_a, exp_b) in exps_a.iter().zip(exps_b) {
        if disjoint_exp_literal(exp_a, exp_b)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn disjoint_notexp_literal(
    notexp_a: &NotExp,
    notexp_b: &NotExp,
    span: &Span,
) -> Result<bool, StructureError> {
    if !notexp_a.to_mixop().syntax_eq(&notexp_b.to_mixop()) {
        return Ok(true);
    }
    disjoint_exps_literal(&notexp_a.args(), &notexp_b.args(), span)
}

fn disjoint_list_exp_literal(
    exps_a: &[Exp],
    exps_b: &[Exp],
    span: &Span,
) -> Result<bool, StructureError> {
    if exps_a.len() != exps_b.len() {
        Ok(true)
    } else {
        disjoint_exps_literal(
            &exps_a.iter().collect::<Vec<_>>(),
            &exps_b.iter().collect::<Vec<_>>(),
            span,
        )
    }
}

fn overlap_typ(
    tdenv: &TDEnv,
    exp: &Exp,
    typ_a: &Typ,
    subcheck_a: &Subcheck,
    typ_b: &Typ,
    subcheck_b: &Subcheck,
) -> Result<Overlap, StructureError> {
    let mixops_a = typ_as_variant(tdenv, typ_a)?;
    let mixops_b = typ_as_variant(tdenv, typ_b)?;
    let (Some(mixops_a), Some(mixops_b)) = (mixops_a, mixops_b) else {
        return Ok(Overlap::Fuzzy);
    };
    let contains = |mixops: &[Mixop], mixop: &Mixop| {
        mixops
            .iter()
            .any(|mixop_other| mixop.syntax_eq(mixop_other))
    };
    if mixops_a.iter().all(|mixop| contains(&mixops_b, mixop))
        && mixops_b.iter().all(|mixop| contains(&mixops_a, mixop))
    {
        Ok(Overlap::Identical)
    } else if !mixops_a.iter().any(|mixop| contains(&mixops_b, mixop)) {
        Ok(Overlap::Disjoint {
            exp: exp.clone(),
            guard_a: Guard::Sub(typ_a.clone(), Box::new(subcheck_a.clone())),
            guard_b: Guard::Sub(typ_b.clone(), Box::new(subcheck_b.clone())),
        })
    } else {
        Ok(Overlap::Fuzzy)
    }
}

fn overlap_pattern(exp: &Exp, pattern_a: &Pattern, pattern_b: &Pattern) -> Overlap {
    if pattern_a.syntax_eq(pattern_b) {
        return Overlap::Identical;
    }
    let guard_a = Guard::Match(pattern_a.clone());
    let guard_b = Guard::Match(pattern_b.clone());
    match (pattern_a, pattern_b) {
        (Pattern::Case(_), Pattern::Case(_)) => Overlap::Disjoint {
            exp: exp.clone(),
            guard_a,
            guard_b,
        },
        (Pattern::List(pattern_a), Pattern::List(pattern_b)) => {
            overlap_list_pattern(exp, pattern_a, pattern_b, guard_a, guard_b)
        }
        (Pattern::Opt(OptPattern::Some), Pattern::Opt(OptPattern::None))
        | (Pattern::Opt(OptPattern::None), Pattern::Opt(OptPattern::Some)) => Overlap::Partition {
            exp: exp.clone(),
            guard_a,
            guard_b,
        },
        _ => Overlap::Fuzzy,
    }
}

fn overlap_list_pattern(
    exp: &Exp,
    pattern_a: &ListPattern,
    pattern_b: &ListPattern,
    guard_a: Guard,
    guard_b: Guard,
) -> Overlap {
    match (pattern_a, pattern_b) {
        (ListPattern::Cons, ListPattern::Fixed(num))
        | (ListPattern::Fixed(num), ListPattern::Cons) => {
            if *num == 0 {
                Overlap::Partition {
                    exp: exp.clone(),
                    guard_a,
                    guard_b,
                }
            } else {
                Overlap::Disjoint {
                    exp: exp.clone(),
                    guard_a,
                    guard_b,
                }
            }
        }
        (ListPattern::Cons, ListPattern::Nil) | (ListPattern::Nil, ListPattern::Cons) => {
            Overlap::Partition {
                exp: exp.clone(),
                guard_a,
                guard_b,
            }
        }
        (ListPattern::Fixed(_), ListPattern::Fixed(_)) => Overlap::Disjoint {
            exp: exp.clone(),
            guard_a,
            guard_b,
        },
        (ListPattern::Fixed(num), ListPattern::Nil)
        | (ListPattern::Nil, ListPattern::Fixed(num)) => {
            if *num == 0 {
                Overlap::Identical
            } else {
                Overlap::Disjoint {
                    exp: exp.clone(),
                    guard_a,
                    guard_b,
                }
            }
        }
        _ => Overlap::Fuzzy,
    }
}

fn overlap_typ_and_pattern(
    tdenv: &TDEnv,
    exp: &Exp,
    typ: &Typ,
    subcheck: &Subcheck,
    pattern: &Pattern,
) -> Result<Overlap, StructureError> {
    let Pattern::Case(mixop) = pattern else {
        return Ok(Overlap::Fuzzy);
    };
    let Some(mixops) = typ_as_variant(tdenv, typ)? else {
        return Ok(Overlap::Fuzzy);
    };
    if mixops
        .iter()
        .any(|mixop_other| mixop.as_ref().syntax_eq(mixop_other))
    {
        return Ok(Overlap::Fuzzy);
    }
    Ok(Overlap::Disjoint {
        exp: exp.clone(),
        guard_a: Guard::Sub(typ.clone(), Box::new(subcheck.clone())),
        guard_b: Guard::Match(pattern.clone()),
    })
}

fn overlap_pattern_and_typ(
    tdenv: &TDEnv,
    exp: &Exp,
    pattern: &Pattern,
    typ: &Typ,
    subcheck: &Subcheck,
) -> Result<Overlap, StructureError> {
    Ok(
        match overlap_typ_and_pattern(tdenv, exp, typ, subcheck, pattern)? {
            Overlap::Disjoint {
                exp,
                guard_a,
                guard_b,
            } => Overlap::Disjoint {
                exp,
                guard_a: guard_b,
                guard_b: guard_a,
            },
            overlap => overlap,
        },
    )
}

pub(crate) fn overlap_exp(
    tdenv: &TDEnv,
    exp_a: &Exp,
    exp_b: &Exp,
) -> Result<Overlap, StructureError> {
    if exp_a.syntax_eq(exp_b) {
        return Ok(Overlap::Identical);
    }
    if let ExpKind::Un(UnOp::Bool(boolop::UnOp::Not), _, exp_inner) = &exp_a.node
        && exp_inner.as_ref().syntax_eq(exp_b)
    {
        return Ok(Overlap::Partition {
            exp: exp_inner.as_ref().clone(),
            guard_a: Guard::Bool(false),
            guard_b: Guard::Bool(true),
        });
    }
    if let ExpKind::Un(UnOp::Bool(boolop::UnOp::Not), _, exp_inner) = &exp_b.node
        && exp_a.syntax_eq(exp_inner.as_ref())
    {
        return Ok(Overlap::Partition {
            exp: exp_inner.as_ref().clone(),
            guard_a: Guard::Bool(true),
            guard_b: Guard::Bool(false),
        });
    }
    match (&exp_a.node, &exp_b.node) {
        (ExpKind::Cmp(_, _, _, _), ExpKind::Cmp(_, _, _, _)) => overlap_cmp_exp(exp_a, exp_b),
        (ExpKind::Sub(exp_a, typ_a, subcheck_a), ExpKind::Sub(exp_b, typ_b, subcheck_b))
            if exp_a.syntax_eq(exp_b) =>
        {
            overlap_typ(tdenv, exp_a, typ_a, subcheck_a, typ_b, subcheck_b)
        }
        (ExpKind::Match(exp_a, pattern_a), ExpKind::Match(exp_b, pattern_b))
            if exp_a.syntax_eq(exp_b) =>
        {
            Ok(overlap_pattern(exp_a, pattern_a, pattern_b))
        }
        (ExpKind::Sub(exp_a, typ_a, subcheck_a), ExpKind::Match(exp_b, pattern_b))
            if exp_a.syntax_eq(exp_b) =>
        {
            overlap_typ_and_pattern(tdenv, exp_a, typ_a, subcheck_a, pattern_b)
        }
        (ExpKind::Match(exp_a, pattern_a), ExpKind::Sub(exp_b, typ_b, subcheck_b))
            if exp_a.syntax_eq(exp_b) =>
        {
            overlap_pattern_and_typ(tdenv, exp_a, pattern_a, typ_b, subcheck_b)
        }
        (ExpKind::Mem(exp_elem_a, exp_set_a), ExpKind::Mem(exp_elem_b, exp_set_b))
            if exp_elem_a.syntax_eq(exp_elem_b) =>
        {
            overlap_mem_exp(exp_elem_a, exp_set_a, exp_set_b)
        }
        _ => Ok(Overlap::Fuzzy),
    }
}

fn overlap_cmp_exp(exp_a: &Exp, exp_b: &Exp) -> Result<Overlap, StructureError> {
    let ExpKind::Cmp(op_a, optyp_a, exp_a_l, exp_a_r) = &exp_a.node else {
        return Ok(Overlap::Fuzzy);
    };
    let ExpKind::Cmp(op_b, optyp_b, exp_b_l, exp_b_r) = &exp_b.node else {
        return Ok(Overlap::Fuzzy);
    };
    if *op_a != CmpOp::Bool(boolop::CmpOp::Eq) || optyp_a != optyp_b {
        return Ok(Overlap::Fuzzy);
    }
    let exps = [(exp_b_l, exp_b_r), (exp_b_r, exp_b_l)];
    if *op_b == CmpOp::Bool(boolop::CmpOp::Eq) {
        for (exp_b_target, exp_b_literal) in exps {
            if exp_a_l.syntax_eq(exp_b_target) && partition_exp_literal(exp_a_r, exp_b_literal) {
                return Ok(Overlap::Partition {
                    exp: exp_a_l.as_ref().clone(),
                    guard_a: Guard::Cmp(*op_a, *optyp_a, exp_a_r.as_ref().clone()),
                    guard_b: Guard::Cmp(*op_b, *optyp_b, exp_b_literal.as_ref().clone()),
                });
            }
        }
        for (exp_b_target, exp_b_literal) in exps {
            if exp_a_l.syntax_eq(exp_b_target) && disjoint_exp_literal(exp_a_r, exp_b_literal)? {
                return Ok(Overlap::Disjoint {
                    exp: exp_a_l.as_ref().clone(),
                    guard_a: Guard::Cmp(*op_a, *optyp_a, exp_a_r.as_ref().clone()),
                    guard_b: Guard::Cmp(*op_b, *optyp_b, exp_b_literal.as_ref().clone()),
                });
            }
        }
    } else if *op_b == CmpOp::Bool(boolop::CmpOp::Ne) {
        for (exp_b_target, exp_b_literal) in exps {
            if exp_a_l.syntax_eq(exp_b_target) && exp_a_r.syntax_eq(exp_b_literal) {
                return Ok(Overlap::Partition {
                    exp: exp_a_l.as_ref().clone(),
                    guard_a: Guard::Cmp(*op_a, *optyp_a, exp_a_r.as_ref().clone()),
                    guard_b: Guard::Cmp(*op_b, *optyp_b, exp_b_literal.as_ref().clone()),
                });
            }
        }
    }
    Ok(Overlap::Fuzzy)
}

fn overlap_mem_exp(
    exp_elem: &Exp,
    exp_set_a: &Exp,
    exp_set_b: &Exp,
) -> Result<Overlap, StructureError> {
    let (ExpKind::List(exps_a), ExpKind::List(exps_b)) = (&exp_set_a.node, &exp_set_b.node) else {
        return Ok(Overlap::Fuzzy);
    };
    for exp_a in exps_a {
        for exp_b in exps_b {
            if !disjoint_exp_literal(exp_a, exp_b)? {
                return Ok(Overlap::Fuzzy);
            }
        }
    }
    Ok(Overlap::Disjoint {
        exp: exp_elem.clone(),
        guard_a: Guard::Mem(exp_set_a.clone()),
        guard_b: Guard::Mem(exp_set_b.clone()),
    })
}

pub(crate) fn overlap_guard(
    tdenv: &TDEnv,
    exp: &Exp,
    guard_a: &Guard,
    guard_b: &Guard,
) -> Result<Overlap, StructureError> {
    let exp_a = guard_as_exp(exp, guard_a);
    let exp_b = guard_as_exp(exp, guard_b);
    overlap_exp(tdenv, &exp_a, &exp_b)
}
