use p4spec_rust::{
    domain::source::{Region, Spanned},
    lang::{al, il},
};

fn span(name: &str) -> Region {
    Region::for_file(name)
}

#[test]
fn as_exp_nests_iterators_in_order_and_tracks_dimension_binders() {
    let id = Spanned::new("x".to_owned(), span("identifier"));
    let typ = Spanned::new(il::ast::TypKind::BoolT, span("type"));
    let variable = (
        id.clone(),
        typ.clone(),
        vec![il::ast::Iter::Opt, il::ast::Iter::List],
    );

    for dim in [false, true] {
        let exp = al::var::as_exp(&variable, dim);
        let il::ast::Exp {
            kind: il::ast::ExpKind::IterE(middle, (il::ast::Iter::List, outer_binders)),
            ty: il::ast::TypKind::IterT(outer_inner_typ, il::ast::Iter::List),
            span: outer_span,
        } = exp
        else {
            panic!("expected outer list iteration")
        };
        let il::ast::TypKind::IterT(base_typ, il::ast::Iter::Opt) = outer_inner_typ.node else {
            panic!("expected option type inside list type")
        };
        assert_eq!(base_typ.node, il::ast::TypKind::BoolT);
        assert_eq!(base_typ.span, id.span);
        assert_eq!(outer_inner_typ.span, id.span);
        assert_eq!(outer_span, id.span);

        let il::ast::Exp {
            kind: il::ast::ExpKind::IterE(base, (il::ast::Iter::Opt, inner_binders)),
            ty: il::ast::TypKind::IterT(inner_base_typ, il::ast::Iter::Opt),
            span: middle_span,
        } = *middle
        else {
            panic!("expected inner option iteration")
        };
        assert_eq!(inner_base_typ.node, il::ast::TypKind::BoolT);
        assert_eq!(inner_base_typ.span, id.span);
        assert_eq!(middle_span, id.span);
        assert_eq!(base.ty, il::ast::TypKind::BoolT);
        assert_eq!(base.span, id.span);

        match (dim, inner_binders.as_slice(), outer_binders.as_slice()) {
            (false, [], []) => {}
            (true, [(inner_id, inner_typ, inner_prior)], [(outer_id, outer_typ, outer_prior)]) => {
                assert_eq!(inner_id, &id);
                assert_eq!(inner_typ.span, typ.span);
                assert_eq!(inner_prior, &Vec::<il::ast::Iter>::new());
                let il::ast::TypKind::IterT(inner_binder_base, il::ast::Iter::Opt) =
                    &inner_typ.node
                else {
                    panic!("expected option binder type")
                };
                assert_eq!(inner_binder_base.node, il::ast::TypKind::BoolT);
                assert_eq!(inner_binder_base.span, id.span);

                assert_eq!(outer_id, &id);
                assert_eq!(outer_typ.span, typ.span);
                assert_eq!(outer_prior, &[il::ast::Iter::Opt]);
                let il::ast::TypKind::IterT(outer_binder_inner, il::ast::Iter::List) =
                    &outer_typ.node
                else {
                    panic!("expected list binder type")
                };
                assert_eq!(outer_binder_inner.span, id.span);
                let il::ast::TypKind::IterT(outer_binder_base, il::ast::Iter::Opt) =
                    &outer_binder_inner.node
                else {
                    panic!("expected option type inside list binder type")
                };
                assert_eq!(outer_binder_base.node, il::ast::TypKind::BoolT);
                assert_eq!(outer_binder_base.span, id.span);
            }
            _ => panic!("unexpected binders for dim={dim}"),
        }
    }
}
