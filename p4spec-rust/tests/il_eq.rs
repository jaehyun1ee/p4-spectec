use p4spec_rust::{
    domain::source::{Region, Spanned},
    lang::il::{ast, eq},
};

fn span(file: &str) -> Region {
    Region::for_file(file)
}

fn bool_typ(file: &str) -> ast::Typ {
    Spanned::new(ast::TypKind::BoolT, span(file))
}

fn bool_exp(value: bool, file: &str) -> ast::Exp {
    ast::Exp::new(ast::ExpKind::BoolE(value), ast::TypKind::BoolT, span(file))
}

#[test]
fn subtype_expressions_ignore_compiled_subcheck_metadata_in_equality() {
    let operand = bool_exp(true, "operand");
    let left = ast::Exp::new(
        ast::ExpKind::SubE(
            Box::new(operand.clone()),
            bool_typ("target"),
            Box::new(ast::Subcheck::SkipSC),
        ),
        ast::TypKind::BoolT,
        span("left"),
    );
    let right = ast::Exp::new(
        ast::ExpKind::SubE(
            Box::new(operand),
            bool_typ("target"),
            Box::new(ast::Subcheck::RecurseSC(bool_typ("recurse"))),
        ),
        ast::TypKind::BoolT,
        span("right"),
    );

    assert!(eq::eq_exp(&left, &right));
}
