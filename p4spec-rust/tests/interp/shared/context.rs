//! Function signatures across AL, SL, and PL contexts
//!
//! Each loader preserves value and higher-order parameter types;
//! local function arguments expose the same signatures as global definitions.

use p4spec_rust::{
    interp::{al, pl, shared::context::WriteContext, sl},
    lang::{
        common::{Id, source::Span},
        data::typ::{self, FuncTyp},
        traits::eq::SyntaxEq,
    },
    pass::{algo, elaborate, prosify, structure},
    phrase,
};

fn id(name: &str) -> Id {
    phrase!(node: name.to_owned(), span: Span::default())
}

/// Checks every definition kind through global and local function lookup.
fn check_signatures(mut ctx: impl WriteContext) {
    let typ_t = typ::make::var(id("T"), vec![]);
    let typ_u = typ::make::var(id("U"), vec![]);
    let typ_nested = typ::make::func(vec![], vec![typ::make::bool()], typ::make::nat());
    let typ_func = typ::make::func(vec![id("U")], vec![typ_u.clone(), typ_nested], typ_u);
    // Definition kinds differ in their type and value parameter lists
    let signatures = [
        ("external", vec![id("T")], vec![typ_t.clone(), typ_func], typ_t),
        (
            "builtin",
            vec![id("X")],
            vec![typ::make::var(id("X"), vec![])],
            typ::make::var(id("X"), vec![]),
        ),
        (
            "defined",
            vec![],
            vec![
                typ::make::nat(),
                typ::make::func(vec![], vec![typ::make::nat()], typ::make::nat()),
            ],
            typ::make::nat(),
        ),
        ("table", vec![], vec![typ::make::var(id("choice"), vec![])], typ::make::bool()),
    ];
    for (name, tparams, typs_params, typ_ret) in signatures {
        let typ_expect = FuncTyp { tparams, typs_params, typ_ret: Box::new(typ_ret) };
        // Compare syntax independently of loader-preserved source spans
        let typ_global = ctx.find_func_typ(&id(name)).unwrap();
        assert!(typ_global.syntax_eq(&typ_expect), "global signature of {name}");

        // A local alias must expose the definition's original signature
        let func = ctx.find_func(&id(name)).unwrap().clone();
        let id_local = id(&format!("local_{name}"));
        ctx.add_func(id_local.clone(), func).unwrap();
        let typ_local = ctx.find_func_typ(&id_local).unwrap();
        assert!(typ_local.syntax_eq(&typ_expect), "local signature of {name}");
    }
}

#[test]
fn function_signatures_preserve_nested_parameters_in_all_interpreters() {
    let source = r#"
extern dec $external<T>(T, def $f<U>(U, def $g(bool) : nat) : U) : T
builtin dec $builtin<X>(X) : X
var n : nat
dec $defined(nat, def $f(nat) : nat) : nat
def $defined(n, def $f) = $f(n)
syntax choice
syntax firstChoice = A
syntax secondChoice = B
syntax choice =
  | firstChoice
  | secondChoice
tbl dec $table(choice) : bool
tbl def $table =
  | A => true
  | B => false
"#;
    // Convert one source so each context loads the corresponding stage
    let spec_el = crate::spec_fixture::parse(source).unwrap();
    let spec_il = elaborate::convert(spec_el).unwrap();
    let spec_al = algo::convert(spec_il).unwrap();
    let spec_sl = structure::convert(spec_al.clone(), false).unwrap();
    let spec_pl = prosify::convert(spec_sl.clone()).unwrap();

    // Exercise all adapters through the existing context interfaces
    let global_al = al::context::Global::load(spec_al).unwrap();
    let global_sl = sl::context::Global::load(spec_sl).unwrap();
    let global_pl = pl::context::Global::load(spec_pl).unwrap();
    check_signatures(al::context::Context::new(&global_al));
    check_signatures(sl::context::Context::new(&global_sl));
    check_signatures(pl::context::Context::new(&global_pl));
}
