use crate::{
    lang::{il::ast::*, xl::bool as boolop},
    note_phrase,
    pass::structure::opt::overlap::{Overlap, overlap_exp},
    runtime::envs::algo::TDEnv,
};
fn exp(exp_kind: ExpKind) -> Exp {
    note_phrase!(node: exp_kind, note: TypKind::Bool, span: Default::default())
}
fn cmp(exp_l: Exp, exp_r: Exp) -> Exp {
    exp(ExpKind::Cmp(
        CmpOp::Bool(boolop::CmpOp::Eq),
        OpTyp::Bool,
        Box::new(exp_l),
        Box::new(exp_r),
    ))
}
#[test]
fn test_boolean_partition_and_literal_disjointness() {
    let exp_target = exp(ExpKind::Var(
        crate::phrase!(node: "flag".into(), span: Default::default()),
    ));
    let exp_a = cmp(exp_target.clone(), exp(ExpKind::Bool(true)));
    let exp_b = cmp(exp_target.clone(), exp(ExpKind::Bool(false)));
    assert!(matches!(
        overlap_exp(&TDEnv::new(), &exp_a, &exp_b).unwrap(),
        Overlap::Partition { .. }
    ));
    let exp_a = cmp(exp_target.clone(), exp(ExpKind::Text("a".into())));
    let exp_b = cmp(exp_target, exp(ExpKind::Text("b".into())));
    assert!(matches!(
        overlap_exp(&TDEnv::new(), &exp_a, &exp_b).unwrap(),
        Overlap::Disjoint { .. }
    ));
}

#[test]
fn test_comparison_order_negation_and_fuzzy() {
    let exp_target = exp(ExpKind::Var(
        crate::phrase!(node: "flag".into(), span: Default::default()),
    ));
    let exp_a = cmp(exp_target.clone(), exp(ExpKind::Bool(true)));
    let exp_b = cmp(exp(ExpKind::Bool(false)), exp_target.clone());
    assert!(matches!(
        overlap_exp(&TDEnv::new(), &exp_a, &exp_b).unwrap(),
        Overlap::Partition { .. }
    ));
    let exp_ne = exp(ExpKind::Cmp(
        CmpOp::Bool(boolop::CmpOp::Ne),
        OpTyp::Bool,
        Box::new(exp(ExpKind::Bool(true))),
        Box::new(exp_target.clone()),
    ));
    assert!(matches!(
        overlap_exp(&TDEnv::new(), &exp_a, &exp_ne).unwrap(),
        Overlap::Partition {
            guard_b: crate::lang::sl::ast::Guard::Cmp(CmpOp::Bool(boolop::CmpOp::Ne), _, _),
            ..
        }
    ));
    assert!(matches!(
        overlap_exp(&TDEnv::new(), &exp_ne, &exp_a).unwrap(),
        Overlap::Fuzzy
    ));
    let exp_not = exp(ExpKind::Un(
        UnOp::Bool(boolop::UnOp::Not),
        OpTyp::Bool,
        Box::new(exp_target.clone()),
    ));
    assert!(matches!(
        overlap_exp(&TDEnv::new(), &exp_not, &exp_target).unwrap(),
        Overlap::Partition {
            guard_a: crate::lang::sl::ast::Guard::Bool(false),
            ..
        }
    ));
    assert!(matches!(
        overlap_exp(&TDEnv::new(), &exp_a, &exp_a).unwrap(),
        Overlap::Identical
    ));
    assert!(matches!(
        overlap_exp(&TDEnv::new(), &exp_a, &exp_target).unwrap(),
        Overlap::Fuzzy
    ));
}

#[test]
fn test_guards_preserve_annotations_and_subchecks() {
    use crate::lang::sl::ast::Guard;
    use crate::pass::structure::opt::overlap::{exp_as_guard, guard_as_exp};
    let mut exp_target = exp(ExpKind::Bool(true));
    exp_target.span = crate::lang::common::source::Span::new(
        crate::lang::common::source::Position::new("guard", 4, 2),
        crate::lang::common::source::Position::new("guard", 4, 8),
    );
    let typ = crate::phrase!(node: TypKind::Bool, span: exp_target.span.clone());
    let guard = Guard::Sub(typ, Box::new(Subcheck::Tuple(vec![Subcheck::Skip])));
    let exp_cond = guard_as_exp(&exp_target, &guard);
    assert_eq!(exp_cond.span, exp_target.span);
    assert_eq!(*exp_cond.note, TypKind::Bool);
    assert_eq!(exp_as_guard(&exp_target, &exp_cond), Some(guard));
    assert_eq!(guard_as_exp(&exp_target, &Guard::Bool(true)), exp_target);
    assert_eq!(exp_as_guard(&exp_target, &exp_target), None);
    for guard in [
        Guard::Bool(false),
        Guard::Cmp(
            CmpOp::Bool(boolop::CmpOp::Ne),
            OpTyp::Int,
            exp(ExpKind::Bool(false)),
        ),
        Guard::Match(Pattern::Opt(OptPattern::Some)),
        Guard::Mem(exp(ExpKind::List(vec![]))),
    ] {
        assert_eq!(
            exp_as_guard(&exp_target, &guard_as_exp(&exp_target, &guard)),
            Some(guard)
        );
    }
}

#[test]
fn test_list_patterns_follow_source_heuristic() {
    use crate::lang::sl::ast::Guard;
    use crate::pass::structure::opt::overlap::overlap_guard;
    let exp_target = exp(ExpKind::Bool(true));
    let guard_cons = Guard::Match(Pattern::List(ListPattern::Cons));
    let guard_empty = Guard::Match(Pattern::List(ListPattern::Fixed(0)));
    let guard_nonempty = Guard::Match(Pattern::List(ListPattern::Fixed(2)));
    assert!(matches!(
        overlap_guard(&TDEnv::new(), &exp_target, &guard_cons, &guard_empty).unwrap(),
        Overlap::Partition { .. }
    ));
    assert!(matches!(
        overlap_guard(&TDEnv::new(), &exp_target, &guard_cons, &guard_nonempty).unwrap(),
        Overlap::Disjoint { .. }
    ));
    assert!(matches!(
        overlap_guard(
            &TDEnv::new(),
            &exp_target,
            &guard_empty,
            &Guard::Match(Pattern::List(ListPattern::Nil))
        )
        .unwrap(),
        Overlap::Identical
    ));
}

#[test]
fn test_membership_nested_literals_and_malformed_tuples() {
    let exp_target = exp(ExpKind::Bool(true));
    let typ = crate::phrase!(node: TypKind::Bool, span: Default::default());
    let exp_a = exp(ExpKind::UpCast(
        Box::new(typ.clone()),
        Box::new(exp(ExpKind::Tuple(vec![exp(ExpKind::Bool(false))]))),
    ));
    let exp_b = exp(ExpKind::UpCast(
        Box::new(typ),
        Box::new(exp(ExpKind::Tuple(vec![exp(ExpKind::Bool(true))]))),
    ));
    let exp_a = exp(ExpKind::Mem(
        Box::new(exp_target.clone()),
        Box::new(exp(ExpKind::List(vec![exp_a]))),
    ));
    let exp_b = exp(ExpKind::Mem(
        Box::new(exp_target.clone()),
        Box::new(exp(ExpKind::List(vec![exp_b]))),
    ));
    assert!(matches!(
        overlap_exp(&TDEnv::new(), &exp_a, &exp_b).unwrap(),
        Overlap::Disjoint { .. }
    ));
    let exp_a = cmp(exp_target.clone(), exp(ExpKind::Tuple(vec![])));
    let exp_b = cmp(
        exp_target,
        exp(ExpKind::Tuple(vec![exp(ExpKind::Bool(true))])),
    );
    let error = overlap_exp(&TDEnv::new(), &exp_a, &exp_b).unwrap_err();
    assert_eq!(
        error.kind,
        crate::pass::structure::error::StructureErrorKind::ArityMismatch {
            expected: 0,
            actual: 1
        }
    );
}

#[test]
fn test_variant_alias_overlap_and_type_errors() {
    use crate::{
        lang::{common::notation::mixfix::Mixfix, sl::ast::Guard},
        pass::structure::opt::overlap::{overlap_guard, typ_as_variant},
        runtime::typdef::TypeDef,
    };
    let id_a = crate::phrase!(node: "A".into(), span: Default::default());
    let id_b = crate::phrase!(node: "B".into(), span: Default::default());
    let id_alias = crate::phrase!(node: "Alias".into(), span: Default::default());
    let typ_a = crate::phrase!(node: TypKind::Var(id_a.clone(), vec![]), span: Default::default());
    let typ_b = crate::phrase!(node: TypKind::Var(id_b.clone(), vec![]), span: Default::default());
    let typ_alias =
        crate::phrase!(node: TypKind::Var(id_alias.clone(), vec![]), span: Default::default());
    let mixop_a = crate::frontend::parse::parse_mixop("A").unwrap();
    let mixop_b = crate::frontend::parse::parse_mixop("B").unwrap();
    let mut tdenv = TDEnv::new();
    for (id, mixop) in [
        (id_a.clone(), mixop_a.clone()),
        (id_b.clone(), mixop_b.clone()),
    ] {
        let nottyp: Mixfix<Typ> =
            mixop.map(|_| crate::phrase!(node: TypKind::Bool, span: Default::default()));
        let typcase = (
            crate::phrase!(node: nottyp, span: Default::default()),
            crate::phrase!(node: (id.clone(), vec![]), span: Default::default()),
            vec![],
        );
        tdenv.insert(id, TypeDef::Defined(vec![], Box::new(crate::phrase!(node: DefTypKind::Variant(vec![typcase]), span: Default::default()))));
    }
    tdenv.insert(
        id_alias,
        TypeDef::Defined(
            vec![],
            Box::new(
                crate::phrase!(node: DefTypKind::Plain(typ_a.clone()), span: Default::default()),
            ),
        ),
    );
    assert_eq!(
        typ_as_variant(&tdenv, &typ_alias).unwrap(),
        Some(vec![mixop_a])
    );
    let guard_a = Guard::Sub(typ_alias, Box::new(Subcheck::Tuple(vec![Subcheck::Skip])));
    let guard_b = Guard::Sub(typ_b, Box::new(Subcheck::Skip));
    let exp_target = exp(ExpKind::Bool(true));
    assert!(
        matches!(overlap_guard(&tdenv, &exp_target, &guard_a, &guard_b).unwrap(), Overlap::Disjoint { guard_a: Guard::Sub(_, subcheck), .. } if *subcheck == Subcheck::Tuple(vec![Subcheck::Skip]))
    );
    let guard_pattern = Guard::Match(Pattern::Case(Box::new(mixop_b)));
    assert!(matches!(
        overlap_guard(&tdenv, &exp_target, &guard_pattern, &guard_a).unwrap(),
        Overlap::Disjoint {
            guard_a: Guard::Match(_),
            ..
        }
    ));
    assert!(matches!(
        overlap_guard(
            &tdenv,
            &exp_target,
            &guard_a,
            &Guard::Sub(typ_a.clone(), Box::new(Subcheck::Skip))
        )
        .unwrap(),
        Overlap::Identical
    ));
    assert!(matches!(
        typ_as_variant(&TDEnv::new(), &typ_a).unwrap_err().kind,
        crate::pass::structure::error::StructureErrorKind::Type(_)
    ));
}

#[test]
fn test_numeric_list_and_case_literals_are_disjoint() {
    let exp_target = exp(ExpKind::Bool(true));
    let mixop = crate::lang::common::notation::mixfix::Mixfix::Seq(vec![
        crate::frontend::parse::parse_mixop("C").unwrap(),
        crate::lang::common::notation::mixfix::Mixfix::Arg(()),
    ]);
    let exp_num_a = exp(ExpKind::Num(Num::Int(1.into())));
    let exp_num_b = exp(ExpKind::Num(Num::Int(2.into())));
    let notexp_a = Mixop::fill(&mixop, vec![exp_num_a.clone()]).unwrap();
    let notexp_b = Mixop::fill(&mixop, vec![exp_num_b.clone()]).unwrap();
    for (exp_a, exp_b) in [
        (exp_num_a, exp_num_b),
        (
            exp(ExpKind::List(vec![])),
            exp(ExpKind::List(vec![exp(ExpKind::Bool(true))])),
        ),
        (
            exp(ExpKind::Case(Box::new(notexp_a))),
            exp(ExpKind::Case(Box::new(notexp_b))),
        ),
    ] {
        let exp_a = cmp(exp_target.clone(), exp_a);
        let exp_b = cmp(exp_target.clone(), exp_b);
        assert!(matches!(
            overlap_exp(&TDEnv::new(), &exp_a, &exp_b).unwrap(),
            Overlap::Disjoint { .. }
        ));
    }
}
