use super::*;
use crate::diagnostic::{LabelStyle, ReportKind};
use crate::lang::common::source::Position;

#[test]
fn type_shape_checks_preserve_fatal_errors_and_locate_mismatches() {
    let ctx = Context::new();
    let span_typ = Span::new(Position::new("shape", 1, 0), Position::new("shape", 1, 4));
    let span_exp = Span::new(Position::new("shape", 2, 0), Position::new("shape", 2, 3));
    let typ_il = typ_at(il::TypKind::Bool, &span_typ);
    let id = phrase!(node: "undefined".to_owned(), span: span_typ.clone());
    let typ_missing_il = typ_at(il::TypKind::Var(id, vec![]), &span_typ);
    let checks: &[(
        fn(&Context, &il::Typ) -> Backtrack<()>,
        fn(&Context, &il::Typ, &Span) -> Backtrack<()>,
    )] = &[
        (
            |ctx, typ_il| as_text_typ_unavailable(ctx, typ_il),
            |ctx, typ_il, span| {
                as_text_typ_mismatch(ctx, typ_il, || {
                    error::exp::expression_type_shape_mismatch(span, typ_il, "text")
                })
            },
        ),
        (
            |ctx, typ_il| as_iter_typ_unavailable(ctx, typ_il).map(|_| ()),
            |ctx, typ_il, span| {
                as_iter_typ_mismatch(ctx, typ_il, || {
                    error::exp::expression_type_shape_mismatch(span, typ_il, "an iteration")
                })
                .map(|_| ())
            },
        ),
        (
            |ctx, typ_il| as_tuple_typ_unavailable(ctx, typ_il).map(|_| ()),
            |ctx, typ_il, span| {
                as_tuple_typ_mismatch(ctx, typ_il, || {
                    error::exp::expression_type_shape_mismatch(span, typ_il, "a tuple")
                })
                .map(|_| ())
            },
        ),
        (
            |ctx, typ_il| as_list_typ_unavailable(ctx, typ_il).map(|_| ()),
            |ctx, typ_il, span| {
                as_list_typ_mismatch(ctx, typ_il, || {
                    error::exp::expression_type_shape_mismatch(span, typ_il, "a list")
                })
                .map(|_| ())
            },
        ),
        (
            |ctx, typ_il| as_struct_typ_unavailable(ctx, typ_il).map(|_| ()),
            |ctx, typ_il, span| {
                as_struct_typ_mismatch(ctx, typ_il, || {
                    error::exp::expression_type_shape_mismatch(span, typ_il, "a struct")
                })
                .map(|_| ())
            },
        ),
    ];
    for (check_unavailable, check_mismatch) in checks {
        assert!(matches!(check_unavailable(&ctx, &typ_il), Backtrack::Unavailable(_)));
        let Backtrack::Mismatch(reports) = check_mismatch(&ctx, &typ_il, &span_exp) else {
            panic!("type requirement must mismatch");
        };
        assert_eq!(reports.len(), 1);
        let ReportKind::Cause(diagnostic) = &reports[0].kind else { panic!("expected cause") };
        assert_eq!(diagnostic.labels[0].style, LabelStyle::Primary);
        assert_eq!(diagnostic.labels[0].span, span_exp);
        assert!(diagnostic.message.contains("'bool'"));
        let Backtrack::Fatal(reports_unavailable) = check_unavailable(&ctx, &typ_missing_il) else {
            panic!("undefined type must be fatal");
        };
        let Backtrack::Fatal(reports_mismatch) = check_mismatch(&ctx, &typ_missing_il, &span_exp)
        else {
            panic!("type requirement must preserve fatal");
        };
        assert_eq!(format!("{reports_unavailable:?}"), format!("{reports_mismatch:?}"));
    }
}
