use std::{path::Path, process::Command};

use p4spec_rust::{
    lang::{
        common::{
            notation::mixfix::Mixfix,
            source::{Span, Spanned},
        },
        il::ast::{self, DefTypKind, Iter, Subcheck, TypKind},
        traits::print::Print,
    },
    runtime::{
        static_env::TypeDimension,
        types::{
            Substitution, TypeDefinition, TypeEnvironment, equivalent_function_type, expand_type,
            is_subtype, optimize_subtype, reset_fresh_type_ids, substitute_type,
        },
    },
};
use serde_json::{Value, json};

fn id(name: &str) -> ast::Id {
    Spanned::new(name.to_owned(), Span::default())
}

fn typ(kind: TypKind) -> ast::Typ {
    Spanned::new(kind, Span::default())
}

fn var(name: &str, arguments: Vec<ast::Targ>) -> ast::Typ {
    typ(TypKind::Var(id(name), arguments))
}

fn variant(types: Vec<ast::Typ>) -> ast::DefTyp {
    Spanned::new(
        DefTypKind::Variant(
            types
                .into_iter()
                .map(|typ| {
                    (
                        Spanned::new(Mixfix::Arg(typ), Span::default()),
                        Spanned::new((id("Origin"), vec![]), Span::default()),
                        vec![],
                    )
                })
                .collect(),
        ),
        Span::default(),
    )
}

fn subcheck_name(subcheck: &Subcheck) -> Value {
    match subcheck {
        Subcheck::Skip => json!({ "kind": "skip" }),
        Subcheck::Mixop(mixops) => json!({ "kind": "mixop", "count": mixops.len() }),
        Subcheck::Tuple(subchecks) => json!({
            "kind": "tuple",
            "items": subchecks.iter().map(subcheck_name).collect::<Vec<_>>()
        }),
        Subcheck::Iter(iter, subcheck) => json!({
            "kind": "iter",
            "iter": match iter { Iter::Opt => "?", Iter::List => "*" },
            "inner": subcheck_name(subcheck)
        }),
        Subcheck::Recurse(typ) => {
            json!({ "kind": "recurse", "type": Print::to_string(typ) })
        }
    }
}

fn fresh_parameter_name(typ: ast::Typ) -> String {
    let TypKind::Func(parameters, _, _) = typ.node else {
        panic!("freshness fixture is a function type")
    };
    parameters[0].node.clone()
}

fn rust_results() -> Value {
    reset_fresh_type_ids();
    let bool_type = typ(TypKind::Bool);
    let text_type = typ(TypKind::Text);
    let mut substitution = Substitution::new();
    substitution.insert(id("T"), text_type.clone());
    let substituted = substitute_type(
        &substitution,
        &typ(TypKind::Tuple(vec![var("T", vec![]), bool_type.clone()])),
    )
    .expect("substitute comparison fixture");
    reset_fresh_type_ids();
    let mut freshness_substitution = Substitution::new();
    freshness_substitution.insert(id("X"), bool_type.clone());
    let freshness_fixture = typ(TypKind::Func(
        vec![id("T")],
        vec![var("T", vec![])],
        Box::new(var("T", vec![])),
    ));
    let fresh_sequence = [
        fresh_parameter_name(
            substitute_type(&freshness_substitution, &freshness_fixture)
                .expect("first fresh comparison fixture"),
        ),
        fresh_parameter_name(
            substitute_type(&freshness_substitution, &freshness_fixture)
                .expect("second fresh comparison fixture"),
        ),
    ];

    let mut environment = TypeEnvironment::new();
    environment.insert(
        id("Pair"),
        TypeDefinition::Defined(
            vec![id("T")],
            Box::new(Spanned::new(
                DefTypKind::Plain(typ(TypKind::Tuple(vec![
                    var("T", vec![]),
                    var("T", vec![]),
                ]))),
                Span::default(),
            )),
        ),
    );
    environment.insert(
        id("Small"),
        TypeDefinition::Defined(vec![], Box::new(variant(vec![bool_type.clone()]))),
    );
    environment.insert(
        id("Large"),
        TypeDefinition::Defined(
            vec![],
            Box::new(variant(vec![bool_type.clone(), text_type.clone()])),
        ),
    );

    let expanded = expand_type(&environment, &var("Pair", vec![bool_type.clone()]))
        .expect("expand comparison fixture");
    let function_equivalent = equivalent_function_type(
        &environment,
        &Span::default(),
        &[id("T")],
        &[var("T", vec![])],
        &var("T", vec![]),
        &[id("U")],
        &[var("U", vec![])],
        &var("U", vec![]),
    )
    .expect("compare function fixture");
    let optional_bool = typ(TypKind::Iter(Box::new(bool_type.clone()), Iter::Opt));
    let list_bool = typ(TypKind::Iter(Box::new(bool_type.clone()), Iter::List));
    let optimized = optimize_subtype(&environment, &var("Large", vec![]), &var("Small", vec![]))
        .expect("optimize comparison fixture");
    let dimension_l = TypeDimension::new(bool_type.clone(), vec![Iter::Opt]);
    let dimension_r = TypeDimension::new(bool_type, vec![Iter::Opt, Iter::List]);

    json!({
        "substitution": Print::to_string(&substituted),
        "fresh_sequence": fresh_sequence,
        "expansion": Print::to_string(&expanded),
        "function_equivalent": function_equivalent,
        "variant_subtype": is_subtype(
            &environment,
            &var("Small", vec![]),
            &var("Large", vec![]),
        ).expect("compare variant subtype"),
        "iteration_subtype": is_subtype(&environment, &optional_bool, &list_bool)
            .expect("compare iteration subtype"),
        "optimized": subcheck_name(&optimized),
        "dimension_compare": match dimension_l.compare(&dimension_r) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => 1,
        },
        "dimension_sub": dimension_l.is_subdimension_of(&dimension_r),
    })
}

#[test]
#[ignore = "requires the pinned OCaml toolchain"]
fn runtime_type_operations_match_ocaml() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("Rust crate is inside the repository");
    let output = Command::new("opam")
        .args([
            "exec",
            "--",
            "dune",
            "exec",
            "--root",
            repository.to_str().expect("UTF-8 repository path"),
            "./p4spec/test/g04-oracle/g04_oracle.exe",
        ])
        .current_dir(repository)
        .output()
        .expect("run pinned OCaml G04 oracle");
    assert!(
        output.status.success(),
        "G04 oracle failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let expected: Value = serde_json::from_slice(&output.stdout).expect("decode G04 oracle JSON");
    assert_eq!(rust_results(), expected);
}
