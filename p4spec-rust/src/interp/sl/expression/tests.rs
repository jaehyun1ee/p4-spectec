use crate::{
    domain::{
        atom::Atom,
        mixfix::Mixfix,
        source::{Region, Spanned},
    },
    interp::{common::InterpError, sl::context::Context},
    lang::{il::ast as il, sl::ast as sl},
    runtime::{
        r#type::{envs::TypeDefMap, typ::make as make_type, typdef::TypeDef},
        value::{ValueRef, get, make, r#match::SubCache},
    },
};

use super::{Calls, eval_with_calls, value_is_subtype, value_matches_subcheck};

fn id(name: &str) -> il::Id {
    Spanned::new(name.to_owned(), Region::for_file(name))
}

struct RecordingCalls {
    id: Option<il::Id>,
    type_args: Vec<il::Typ>,
    values: Vec<ValueRef>,
    subtype_checks: usize,
}

impl Calls for RecordingCalls {
    fn value_is_subtype(
        &mut self,
        _context: &Context,
        _typ: &il::Typ,
        _value: &ValueRef,
    ) -> Result<bool, InterpError> {
        self.subtype_checks += 1;
        Ok(false)
    }

    fn invoke_func(
        &mut self,
        _context: &mut Context,
        id: &il::Id,
        type_args: Vec<il::Typ>,
        values: Vec<ValueRef>,
    ) -> Result<ValueRef, InterpError> {
        self.id = Some(id.clone());
        self.type_args = type_args;
        self.values = values;
        Ok(make::bool(true, Region::for_file("result")))
    }
}

#[test]
fn call_expression_resolves_type_args_and_evaluates_both_argument_kinds() {
    let signature = |name: &str| {
        Spanned::new(
            crate::lang::sl::ast::DefKind::BuiltinDecD((
                id(name),
                Vec::new(),
                Vec::new(),
                make_type::bool_type(),
                Vec::new(),
            )),
            Region::for_file("definition"),
        )
    };
    let mut context =
        Context::from_spec(false, &[signature("callee"), signature("higher")]).expect("valid spec");
    let local_types: TypeDefMap = [(
        "P".to_owned(),
        TypeDef::Defined(
            Vec::new(),
            Box::new(Spanned::new(
                il::DefTypKind::PlainT(make_type::text_type()),
                Region::for_file("alias"),
            )),
        ),
    )]
    .into_iter()
    .collect();
    context.enter_function(id("caller"), Vec::new(), local_types);
    let type_arg = Spanned::new(
        il::TypKind::VarT(id("P"), Vec::new()),
        Region::for_file("type-arg"),
    );
    let text = il::Exp::new(
        il::ExpKind::TextE("input".to_owned()),
        il::TypKind::TextT,
        Region::for_file("input"),
    );
    let call = il::Exp::new(
        il::ExpKind::CallE(
            id("callee"),
            vec![type_arg],
            vec![
                Spanned::new(il::ArgKind::ExpA(text), Region::for_file("exp-arg")),
                Spanned::new(il::ArgKind::DefA(id("higher")), Region::for_file("def-arg")),
            ],
        ),
        il::TypKind::BoolT,
        Region::for_file("call"),
    );
    let mut calls = RecordingCalls {
        id: None,
        type_args: Vec::new(),
        values: Vec::new(),
        subtype_checks: 0,
    };

    let result = eval_with_calls(&mut context, &mut calls, &call).expect("call succeeds");
    assert_eq!(get::bool(&result), Ok(true));
    assert_eq!(calls.id.as_ref().map(|id| id.node.as_str()), Some("callee"));
    assert_eq!(calls.type_args, vec![make_type::text_type()]);
    assert_eq!(get::text(&calls.values[0]), Ok("input"));
    assert_eq!(get::func(&calls.values[1]).unwrap().node, "higher");
}

#[test]
fn subtype_expressions_use_the_callers_cache() {
    let mut context = Context::from_spec(false, &[]).expect("valid empty spec");
    let value = il::Exp::new(
        il::ExpKind::BoolE(true),
        il::TypKind::BoolT,
        Region::for_file("value"),
    );
    let subtype = il::Exp::new(
        il::ExpKind::SubE(
            Box::new(value),
            make_type::bool_type(),
            Box::new(il::Subcheck::RecurseSC(make_type::bool_type())),
        ),
        il::TypKind::BoolT,
        Region::for_file("subtype"),
    );
    let mut calls = RecordingCalls {
        id: None,
        type_args: Vec::new(),
        values: Vec::new(),
        subtype_checks: 0,
    };

    let result = eval_with_calls(&mut context, &mut calls, &subtype).expect("subtype succeeds");

    assert_eq!(get::bool(&result), Ok(false));
    assert_eq!(calls.subtype_checks, 1);
}

#[test]
fn skipped_subtype_expressions_do_not_call_the_generic_matcher() {
    let mut context = Context::from_spec(false, &[]).expect("valid empty spec");
    let value = il::Exp::new(
        il::ExpKind::BoolE(true),
        il::TypKind::BoolT,
        Region::for_file("value"),
    );
    let subtype = il::Exp::new(
        il::ExpKind::SubE(
            Box::new(value),
            make_type::bool_type(),
            Box::new(il::Subcheck::SkipSC),
        ),
        il::TypKind::BoolT,
        Region::for_file("subtype"),
    );
    let mut calls = RecordingCalls {
        id: None,
        type_args: Vec::new(),
        values: Vec::new(),
        subtype_checks: 0,
    };

    let result = eval_with_calls(&mut context, &mut calls, &subtype).expect("subtype succeeds");

    assert_eq!(get::bool(&result), Ok(true));
    assert_eq!(calls.subtype_checks, 0);
}

#[test]
fn recursive_subtype_expressions_call_the_generic_matcher() {
    let mut context = Context::from_spec(false, &[]).expect("valid empty spec");
    let value = il::Exp::new(
        il::ExpKind::BoolE(true),
        il::TypKind::BoolT,
        Region::for_file("value"),
    );
    let typ = make_type::bool_type();
    let subtype = il::Exp::new(
        il::ExpKind::SubE(
            Box::new(value),
            typ.clone(),
            Box::new(il::Subcheck::RecurseSC(typ)),
        ),
        il::TypKind::BoolT,
        Region::for_file("subtype"),
    );
    let mut calls = RecordingCalls {
        id: None,
        type_args: Vec::new(),
        values: Vec::new(),
        subtype_checks: 0,
    };

    let result = eval_with_calls(&mut context, &mut calls, &subtype).expect("subtype succeeds");

    assert_eq!(get::bool(&result), Ok(false));
    assert_eq!(calls.subtype_checks, 1);
}

#[test]
fn structural_subtype_operations_check_only_value_shapes() {
    let context = Context::from_spec(false, &[]).expect("valid empty spec");
    let atom = Spanned::new(Atom::Tag("SOME".to_owned()), Region::for_file("some"));
    let mixop: il::Mixop = Mixfix::Atom(atom.clone());
    let value_case = make::case_kind(
        &il::TypKind::BoolT,
        Mixfix::Atom(atom),
        Region::for_file("case"),
    );
    let value_opt = make::opt_kind(&il::TypKind::BoolT, None, Region::for_file("opt"));
    let value_list = make::list_kind(
        &il::TypKind::BoolT,
        vec![make::bool(true, Region::for_file("element"))],
        Region::for_file("list"),
    );
    let value = make::tuple_kind(
        &il::TypKind::BoolT,
        vec![value_case, value_opt, value_list],
        Region::for_file("tuple"),
    );
    let subcheck = il::Subcheck::TupleSC(vec![
        il::Subcheck::MixopSC(vec![mixop]),
        il::Subcheck::IterSC(il::Iter::Opt, Box::new(il::Subcheck::SkipSC)),
        il::Subcheck::IterSC(il::Iter::List, Box::new(il::Subcheck::SkipSC)),
    ]);
    let mut calls = RecordingCalls {
        id: None,
        type_args: Vec::new(),
        values: Vec::new(),
        subtype_checks: 0,
    };

    let result = value_matches_subcheck(&context, &mut calls, &subcheck, &value)
        .expect("subtype check succeeds");

    assert!(result);
    assert_eq!(calls.subtype_checks, 0);
}

#[test]
fn named_subtype_results_are_cached_by_type_and_value() {
    let type_def = Spanned::new(
        sl::DefKind::TypD(
            id("T"),
            Vec::new(),
            Spanned::new(
                il::DefTypKind::PlainT(make_type::bool_type()),
                Region::for_file("plain"),
            ),
            Vec::new(),
        ),
        Region::for_file("type-def"),
    );
    let context = Context::from_spec(false, &[type_def]).expect("valid spec");
    let typ = make_type::var_type(id("T"), Vec::new());
    let value = make::bool(true, Region::for_file("value"));
    let mut cache = SubCache::new();

    assert!(value_is_subtype(&mut cache, &context, &typ, &value).unwrap());
    assert_eq!(cache.len(), 1);
    assert!(value_is_subtype(&mut cache, &context, &typ, &value).unwrap());
    assert_eq!(cache.len(), 1);
}

#[test]
fn nested_named_subtype_results_use_the_same_cache() {
    let type_t = Spanned::new(
        sl::DefKind::TypD(
            id("T"),
            Vec::new(),
            Spanned::new(
                il::DefTypKind::StructT(Vec::new()),
                Region::for_file("struct-t"),
            ),
            Vec::new(),
        ),
        Region::for_file("type-t"),
    );
    let type_pair = Spanned::new(
        sl::DefKind::TypD(
            id("Pair"),
            Vec::new(),
            Spanned::new(
                il::DefTypKind::PlainT(make_type::tuple_type(vec![
                    make_type::var_type(id("T"), Vec::new()),
                    make_type::var_type(id("T"), Vec::new()),
                ])),
                Region::for_file("plain-pair"),
            ),
            Vec::new(),
        ),
        Region::for_file("type-pair"),
    );
    let context = Context::from_spec(false, &[type_t, type_pair]).expect("valid spec");
    let typ = make_type::var_type(id("Pair"), Vec::new());
    let type_t = make_type::var_type(id("T"), Vec::new());
    let tuple_type = make_type::tuple_type(vec![type_t.clone(), type_t.clone()]);
    let value = make::tuple_kind(
        &tuple_type.node,
        vec![
            make::structure_kind(&type_t.node, Vec::new(), Region::for_file("left")),
            make::structure_kind(&type_t.node, Vec::new(), Region::for_file("right")),
        ],
        Region::for_file("pair"),
    );
    let mut cache = SubCache::new();

    assert!(value_is_subtype(&mut cache, &context, &typ, &value).unwrap());
    assert!(cache.keys().any(|key| key.id() == "Pair"));
    assert!(cache.keys().any(|key| key.id() == "T"));
    assert_eq!(cache.len(), 2);
}

#[test]
fn nested_local_type_definitions_do_not_share_cache_entries() {
    let mut context = Context::from_spec(false, &[]).expect("valid spec");
    let typ = make_type::tuple_type(vec![make_type::var_type(id("T"), Vec::new())]);
    let value_type = make_type::tuple_type(vec![make_type::bool_type()]);
    let value = make::tuple_kind(
        &value_type.node,
        vec![make::bool(true, Region::for_file("value"))],
        Region::for_file("tuple"),
    );
    let local_bool: TypeDefMap = [(
        "T".to_owned(),
        TypeDef::Defined(
            Vec::new(),
            Box::new(Spanned::new(
                il::DefTypKind::PlainT(make_type::bool_type()),
                Region::for_file("local-bool"),
            )),
        ),
    )]
    .into_iter()
    .collect();
    let local_text: TypeDefMap = [(
        "T".to_owned(),
        TypeDef::Defined(
            Vec::new(),
            Box::new(Spanned::new(
                il::DefTypKind::PlainT(make_type::text_type()),
                Region::for_file("local-text"),
            )),
        ),
    )]
    .into_iter()
    .collect();
    let mut cache = SubCache::new();

    context.enter_function(id("first"), Vec::new(), local_bool);
    assert!(value_is_subtype(&mut cache, &context, &typ, &value).unwrap());
    context.enter_function(id("second"), Vec::new(), local_text);
    assert!(!value_is_subtype(&mut cache, &context, &typ, &value).unwrap());
}
