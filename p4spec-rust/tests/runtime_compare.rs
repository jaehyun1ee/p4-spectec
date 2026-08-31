use std::{path::Path, process::Command};

use p4spec_rust::{
    lang::{
        common::{notation::mixfix::Mixfix, source::Span},
        il::ast::{self, DefTypKind, FuncTyp, Iter, Subcheck, TypKind},
        traits::print::Print,
    },
    runtime::{
        sta::Dim,
        types::{
            TDEnv, Theta, TypeDef, equiv_func_typ, expand_typ, optimize_sub_typ, sub_typ, subst_typ,
        },
    },
};
use serde_json::{Value, json};

fn id(name: &str) -> ast::Id {
    p4spec_rust::phrase! {
        node: name.to_owned(),
        span: Span::default(),
    }
}

fn typ(kind: TypKind) -> ast::Typ {
    p4spec_rust::phrase! {
        node: kind,
        span: Span::default(),
    }
}

fn var(name: &str, targs: Vec<ast::Targ>) -> ast::Typ {
    typ(TypKind::Var(id(name), targs))
}

fn func_typ(tparams: Vec<ast::TParam>, typs_params: Vec<ast::Typ>, typ_ret: ast::Typ) -> FuncTyp {
    FuncTyp {
        tparams,
        typs_params,
        typ_ret: Box::new(typ_ret),
    }
}

fn variant(typs: Vec<ast::Typ>) -> ast::DefTyp {
    p4spec_rust::phrase! { node: DefTypKind::Variant(
        typs.into_iter()
            .map(|typ| {
                (
                    p4spec_rust::phrase! {
                        node: Mixfix::Arg(typ),
                        span: Span::default(),
                    },
                    p4spec_rust::phrase! {
                        node: (id("Origin"), vec![]),
                        span: Span::default(),
                    },
                    vec![],
                )
            })
            .collect(),
    ), span: Span::default() }
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

fn rust_results() -> Value {
    let bool_type = typ(TypKind::Bool);
    let text_type = typ(TypKind::Text);
    let mut theta = Theta::new();
    theta.insert(id("T"), text_type.clone());
    let substituted = subst_typ(
        &theta,
        &typ(TypKind::Tuple(vec![var("T", vec![]), bool_type.clone()])),
    )
    .expect("substitute comparison fixture");
    let mut tdenv = TDEnv::new();
    tdenv.insert(
        id("Pair"),
        TypeDef::Defined(
            vec![id("T")],
            Box::new(
                p4spec_rust::phrase! { node: DefTypKind::Plain(typ(TypKind::Tuple(vec![
                    var("T", vec![]),
                    var("T", vec![]),
                ]))), span: Span::default() },
            ),
        ),
    );
    tdenv.insert(
        id("Small"),
        TypeDef::Defined(vec![], Box::new(variant(vec![bool_type.clone()]))),
    );
    tdenv.insert(
        id("Large"),
        TypeDef::Defined(
            vec![],
            Box::new(variant(vec![bool_type.clone(), text_type.clone()])),
        ),
    );

    let expanded = expand_typ(&tdenv, &var("Pair", vec![bool_type.clone()]))
        .expect("expand comparison fixture");
    let func_typ_l = func_typ(vec![id("T")], vec![var("T", vec![])], var("T", vec![]));
    let func_typ_r = func_typ(vec![id("U")], vec![var("U", vec![])], var("U", vec![]));
    let function_equivalent = equiv_func_typ(&tdenv, &Span::default(), &func_typ_l, &func_typ_r)
        .expect("compare function fixture");
    let optional_bool = typ(TypKind::Iter(Box::new(bool_type.clone()), Iter::Opt));
    let list_bool = typ(TypKind::Iter(Box::new(bool_type.clone()), Iter::List));
    let optimized = optimize_sub_typ(&tdenv, &var("Large", vec![]), &var("Small", vec![]))
        .expect("optimize comparison fixture");
    let dimension_l = Dim::new(bool_type.clone(), vec![Iter::Opt]);
    let dimension_r = Dim::new(bool_type, vec![Iter::Opt, Iter::List]);

    json!({
        "substitution": Print::to_string(&substituted),
        "expansion": Print::to_string(&expanded),
        "function_equivalent": function_equivalent,
        "variant_subtype": sub_typ(
            &tdenv,
            &var("Small", vec![]),
            &var("Large", vec![]),
        ).expect("compare variant subtype"),
        "iteration_subtype": sub_typ(&tdenv, &optional_bool, &list_bool)
            .expect("compare iteration subtype"),
        "optimized": subcheck_name(&optimized),
        "dimension_sub": dimension_l.sub(&dimension_r),
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
