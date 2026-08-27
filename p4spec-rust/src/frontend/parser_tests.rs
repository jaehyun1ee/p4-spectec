use crate::{
    frontend::{
        error::{FrontendError, SyntaxErrorKind},
        lexer::{Lexer, Token},
        parser,
        parser_support::{ParserContext, ParserLocation, parser_tokens},
    },
    lang::{
        common::{notation::atom::Atom, source::Position},
        el::ast::{
            ArgKind, BinOp, DefKind, DefTypKind, ExpKind, Iter, NotTypKind, PlainTypKind, PremKind,
            Typ,
        },
        xl::{self, num::Natural},
    },
};

type ParseError = lalrpop_util::ParseError<ParserLocation, Token, FrontendError>;

fn try_parse_spec(source: &str) -> Result<crate::lang::el::ast::Spec, ParseError> {
    let context = ParserContext::default();
    let lexer =
        Lexer::with_uppercase_classifier("parser-test.watsup", source, |id| context.is_var(id));
    parser::SpecParser::new().parse(&context, parser_tokens(&context, lexer))
}

fn parse_spec(source: &str) -> crate::lang::el::ast::Spec {
    try_parse_spec(source).expect("valid SpecTec fixture")
}

#[test]
fn type_parameters_classify_uppercase_uses_and_build_spanned_el() {
    let source = "syntax pair<X, Y> = X ':' Y\n\nvar p : pair<nat, text>";
    let spec = parse_spec(source);

    assert_eq!(spec.len(), 2);
    assert_eq!(spec[0].span.left, Position::new("parser-test.watsup", 1, 0));
    assert_eq!(
        spec[0].span.right,
        Position::new("parser-test.watsup", 1, 27)
    );

    let DefKind::Typ(typ_def) = &spec[0].node else {
        panic!("expected syntax type definition")
    };
    assert_eq!(typ_def.id.node, "pair");
    assert_eq!(
        typ_def
            .tparams
            .iter()
            .map(|parameter| parameter.node.as_str())
            .collect::<Vec<_>>(),
        ["X", "Y"]
    );

    let DefTypKind::Variant(cases) = &typ_def.def_typ.node else {
        panic!("expected notation variant")
    };
    let Typ::Notation(notation) = &cases[0].0 else {
        panic!("expected notation case")
    };
    let NotTypKind::Seq(types) = &notation.node else {
        panic!("expected notation sequence")
    };
    assert!(matches!(
        &types[0],
        Typ::Plain(plain)
            if matches!(&plain.node, PlainTypKind::Var(id, args) if id.node == "X" && args.is_empty())
    ));
    assert!(matches!(
        &types[1],
        Typ::Notation(notation)
            if matches!(&notation.node, NotTypKind::Atom(atom) if atom.node == Atom::Operator(":".to_owned()))
    ));
    assert!(matches!(
        &types[2],
        Typ::Plain(plain)
            if matches!(&plain.node, PlainTypKind::Var(id, args) if id.node == "Y" && args.is_empty())
    ));

    assert_eq!(spec[1].span.left, Position::new("parser-test.watsup", 3, 0));
    assert_eq!(
        spec[1].span.right,
        Position::new("parser-test.watsup", 3, 23)
    );
    let DefKind::Var(var_def) = &spec[1].node else {
        panic!("expected variable definition")
    };
    assert_eq!(var_def.id.node, "p");
    assert!(matches!(
        &var_def.plain_typ.node,
        PlainTypKind::Var(id, args)
            if id.node == "pair"
                && matches!(args[0].node, PlainTypKind::Num(crate::lang::xl::num::Typ::Nat))
                && matches!(args[1].node, PlainTypKind::Text)
    ));
}

#[test]
fn expressions_preserve_precedence_and_premise_bindings() {
    let source = concat!(
        "def $calc(x) = 1 + 2 * 3 :: [ 4 ] ++ [ 5 ]\n",
        "-- var y : nat\n",
        "-- if y = 0",
    );
    let spec = parse_spec(source);

    let DefKind::FuncDef(function) = &spec[0].node else {
        panic!("expected function definition")
    };
    assert_eq!(function.id.node, "calc");
    assert!(matches!(
        &function.args[0].node,
        ArgKind::Exp(exp) if matches!(&exp.node, ExpKind::Var(id) if id.node == "x")
    ));

    let ExpKind::Cons(left, right) = &function.exp.node else {
        panic!("expected list cons at the lowest operator precedence")
    };
    assert!(matches!(
        &left.node,
        ExpKind::Bin(one, BinOp::Num(xl::num::BinOp::Add), product)
            if matches!(&one.node, ExpKind::Num(_, xl::num::Number::Nat(value)) if value == &Natural::from(1))
                && matches!(&product.node,
                    ExpKind::Bin(two, BinOp::Num(xl::num::BinOp::Mul), three)
                        if matches!(&two.node, ExpKind::Num(_, xl::num::Number::Nat(value)) if value == &Natural::from(2))
                            && matches!(&three.node, ExpKind::Num(_, xl::num::Number::Nat(value)) if value == &Natural::from(3)))
    ));
    assert!(matches!(
        &right.node,
        ExpKind::Cat(first, second)
            if matches!(&first.node, ExpKind::List(values)
                if matches!(&values[0].node, ExpKind::Num(_, xl::num::Number::Nat(value)) if value == &Natural::from(4)))
            && matches!(&second.node, ExpKind::List(values)
                if matches!(&values[0].node, ExpKind::Num(_, xl::num::Number::Nat(value)) if value == &Natural::from(5)))
    ));

    assert!(matches!(
        &function.prems[0].node,
        PremKind::Var(premise) if premise.id.node == "y"
    ));
    assert!(matches!(
        &function.prems[1].node,
        PremKind::If(premise)
            if matches!(&premise.exp.node, ExpKind::Cmp(_, _, right)
                if matches!(&right.node, ExpKind::Num(_, xl::num::Number::Nat(value)) if value == &Natural::from(0)))
    ));
}

#[test]
fn stars_distinguish_type_and_expression_iteration_from_multiplication() {
    let source = concat!(
        "var xs : nat*\n\n",
        "def $repeat(x) = x*\n\n",
        "def $multiply() = 2*3",
    );
    let spec = parse_spec(source);

    assert!(matches!(
        &spec[0].node,
        DefKind::Var(definition)
            if matches!(&definition.plain_typ.node, PlainTypKind::Iter(inner, Iter::List)
                if matches!(inner.node, PlainTypKind::Num(xl::num::Typ::Nat)))
    ));
    assert!(matches!(
        &spec[1].node,
        DefKind::FuncDef(definition)
            if matches!(&definition.exp.node, ExpKind::Iter(inner, Iter::List)
                if matches!(&inner.node, ExpKind::Var(id) if id.node == "x"))
    ));
    assert!(matches!(
        &spec[2].node,
        DefKind::FuncDef(definition)
            if matches!(&definition.exp.node,
                ExpKind::Bin(left, BinOp::Num(xl::num::BinOp::Mul), right)
                    if matches!(&left.node, ExpKind::Num(_, xl::num::Number::Nat(value))
                        if value == &Natural::from(2))
                    && matches!(&right.node, ExpKind::Num(_, xl::num::Number::Nat(value))
                        if value == &Natural::from(3)))
    ));
}

#[test]
fn adjacency_and_whitespace_do_not_change_sequence_or_iteration_parsing() {
    let adjacent = parse_spec("syntax t = ':'B |\n\ndef $f(x) = x*[0]");
    let spaced = parse_spec("syntax t = ':' B |\n\ndef $f(x) = x *[0]");

    for spec in [&adjacent, &spaced] {
        assert!(matches!(
            &spec[0].node,
            DefKind::Typ(definition)
                if matches!(&definition.def_typ.node, DefTypKind::Variant(cases)
                    if matches!(&cases[0].0, Typ::Notation(notation)
                        if matches!(&notation.node, NotTypKind::Seq(types) if types.len() == 2)))
        ));
        assert!(matches!(
            &spec[1].node,
            DefKind::FuncDef(definition)
                if matches!(&definition.exp.node, ExpKind::Idx(iterated, _)
                    if matches!(&iterated.node, ExpKind::Iter(_, Iter::List)))
        ));
    }
}

#[test]
fn definitions_build_their_el_payloads() {
    let source = concat!(
        "extern syntax ext hint(doc \"external\")\n\n",
        "syntax a, b\n\n",
        "extern relation Eval : A ':' B\n\n",
        "relation Step : A '->' B\n\n",
        "rulegroup Eval/main { rule Eval/main/one : A }\n\n",
        "rule Step/solo : A\n\n",
        "extern dec $ext<X>(nat) : bool\n\n",
        "builtin dec $builtin(nat) : nat\n\n",
        "tbl dec $lookup(nat) : bool\n\n",
        "dec $plain(nat) : nat\n\n",
        "tbl def $lookup =\n",
        "| 0 => false\n",
        "| 1 => true\n\n",
        "def $plain(x) = x",
    );
    let spec = parse_spec(source);

    assert_eq!(spec.len(), 12);
    assert!(matches!(&spec[0].node, DefKind::ExternSyntax(definition)
        if definition.id.node == "ext"
            && definition.hints[0].0.node == "doc"
            && matches!(&definition.hints[0].1.node, ExpKind::Text(text) if text == "external")));
    assert!(matches!(&spec[1].node, DefKind::Syntax(definition)
        if definition.entries.iter().map(|entry| entry.id.node.as_str()).collect::<Vec<_>>() == ["a", "b"]));
    assert!(matches!(&spec[2].node, DefKind::ExternRel(definition)
        if definition.id.node == "Eval"));
    assert!(matches!(&spec[3].node, DefKind::Rel(definition)
        if definition.id.node == "Step"));
    assert!(matches!(&spec[4].node, DefKind::RuleGroup(definition)
        if definition.relid.node == "Eval" && definition.groupid.node == "main" && definition.rules.len() == 1));
    assert_eq!(
        spec[4].span.right,
        Position::new("parser-test.watsup", 9, 46)
    );
    assert!(matches!(&spec[5].node, DefKind::RuleGroup(definition)
        if definition.relid.node == "Step" && definition.groupid.node == "solo" && definition.rules.len() == 1));
    assert!(matches!(&spec[6].node, DefKind::ExternDec(definition)
        if definition.id.node == "ext" && definition.tparams[0].node == "X" && definition.params.len() == 1));
    assert!(matches!(&spec[7].node, DefKind::BuiltinDec(definition)
        if definition.id.node == "builtin" && definition.params.len() == 1));
    assert!(matches!(&spec[8].node, DefKind::TableDec(definition)
        if definition.id.node == "lookup" && definition.params.len() == 1));
    assert!(matches!(&spec[9].node, DefKind::FuncDec(definition)
        if definition.id.node == "plain" && definition.params.len() == 1));
    assert!(matches!(&spec[10].node, DefKind::TableDef(definition)
        if definition.id.node == "lookup"
            && definition.rows.len() == 2
            && matches!(&definition.rows[1].node.1.node, ExpKind::Bool(true))));
    assert!(matches!(&spec[11].node, DefKind::FuncDef(definition)
        if definition.id.node == "plain"
            && matches!(&definition.exp.node, ExpKind::Var(id) if id.node == "x")));
}

#[test]
fn definition_and_nested_iteration_spans_include_all_consumed_tokens() {
    let syntax = parse_spec("syntax pair<X, Y>");
    assert_eq!(
        syntax[0].span.right,
        Position::new("parser-test.watsup", 1, 17)
    );

    let table = parse_spec("tbl def $t = |");
    assert_eq!(
        table[0].span.right,
        Position::new("parser-test.watsup", 1, 14)
    );

    let trailing = parse_spec("syntax t = ':'B |");
    assert_eq!(
        trailing[0].span.right,
        Position::new("parser-test.watsup", 1, 17)
    );

    let iterated = parse_spec("def $f() = [A]*?");
    let DefKind::FuncDef(function) = &iterated[0].node else {
        panic!("expected function definition")
    };
    let ExpKind::Iter(inner, Iter::Opt) = &function.exp.node else {
        panic!("expected outer optional iteration")
    };
    assert_eq!(
        function.exp.span.right,
        Position::new("parser-test.watsup", 1, 16)
    );
    assert!(matches!(&inner.node, ExpKind::Iter(_, Iter::List)));
    assert_eq!(inner.span.right, Position::new("parser-test.watsup", 1, 15));
}

#[test]
fn notation_call_atom_span_includes_the_consumed_left_parenthesis() {
    let spec = parse_spec("def $notation() = Wrap(1)");
    let DefKind::FuncDef(function) = &spec[0].node else {
        panic!("expected function definition")
    };
    let ExpKind::Seq(expressions) = &function.exp.node else {
        panic!("expected notation application sequence")
    };
    let ExpKind::Atom(atom) = &expressions[0].node else {
        panic!("expected leading notation atom")
    };

    assert_eq!(atom.span.left, Position::new("parser-test.watsup", 1, 18));
    assert_eq!(atom.span.right, Position::new("parser-test.watsup", 1, 23));
}

#[test]
fn exp_bin_contexts_reject_unbracketed_relation_expressions() {
    parse_spec("def $relation() = A |- B");

    for source in [
        "def $list() = [A |- B]",
        "def $index() = [A][0]",
        "def $tuple() = (A |- B, C)",
        "def $arg() = $f(A |- B)",
        "tbl def $table =\n| A => B |- C",
    ] {
        assert!(
            try_parse_spec(source).is_err(),
            "exp_bin context accepted relation expression: {source}"
        );
    }
}

#[test]
fn table_patterns_accept_exp_seq_lists_and_fusion() {
    let spec = parse_spec("tbl def $table =\n| [A] B # C => D");
    let DefKind::TableDef(definition) = &spec[0].node else {
        panic!("expected table definition")
    };

    assert!(matches!(
        &definition.rows[0].node.0.node,
        ExpKind::Fuse(sequence, right)
            if matches!(&sequence.node, ExpKind::Seq(expressions) if expressions.len() == 2)
                && matches!(&right.node, ExpKind::Atom(_))
    ));
}

#[test]
fn semantic_syntax_failures_keep_their_categories_and_locations() {
    let cases = [
        (
            "syntax",
            SyntaxErrorKind::EmptySyntaxDeclaration,
            Position::new("parser-test.watsup", 1, 0),
            Position::new("parser-test.watsup", 1, 6),
        ),
        (
            "relation R : nat",
            SyntaxErrorKind::ExpectedNotationType,
            Position::new("parser-test.watsup", 1, 13),
            Position::new("parser-test.watsup", 1, 16),
        ),
        (
            "syntax empty = {}",
            SyntaxErrorKind::EmptyStructType,
            Position::new("parser-test.watsup", 1, 15),
            Position::new("parser-test.watsup", 1, 17),
        ),
        (
            "syntax empty =",
            SyntaxErrorKind::EmptyType,
            Position::new("parser-test.watsup", 1, 14),
            Position::new("parser-test.watsup", 1, 14),
        ),
        (
            "syntax empty = |",
            SyntaxErrorKind::EmptyVariantType,
            Position::new("parser-test.watsup", 1, 15),
            Position::new("parser-test.watsup", 1, 16),
        ),
    ];

    for (source, kind, left, right) in cases {
        let error = try_parse_spec(source).expect_err("invalid SpecTec fixture");
        let lalrpop_util::ParseError::User {
            error: FrontendError::Syntax(error),
        } = error
        else {
            panic!("expected typed semantic syntax error")
        };
        assert_eq!(error.kind, kind);
        assert_eq!(error.span.left, left);
        assert_eq!(error.span.right, right);
    }
}
