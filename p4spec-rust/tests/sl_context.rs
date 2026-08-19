use std::rc::Rc;

use p4spec_rust::{
    domain::{
        mixfix::Mixfix,
        source::{Region, Spanned},
    },
    interp::sl::context::{Context, Cursor},
    lang::{il::ast as il, sl::ast as sl},
    runtime::{
        dynamic::var::Variable,
        dynamic_sl::{func::Function, rel::Relation},
        r#type::{typ::make as make_type, typdef::TypeDef},
        value::{get, make},
    },
};

fn span(file: &str) -> Region {
    Region::for_file(file)
}

fn id(name: &str, file: &str) -> il::Id {
    Spanned::new(name.to_owned(), span(file))
}

fn bool_exp(value: bool, file: &str) -> il::Exp {
    il::Exp::new(il::ExpKind::BoolE(value), il::TypKind::BoolT, span(file))
}

fn signature(file: &str) -> sl::RelSignature {
    (
        Spanned::new(Mixfix::Seq(Vec::new()), span(file)),
        Vec::new(),
    )
}

#[test]
fn loading_a_spec_builds_global_tables_and_rejects_duplicates() {
    let spec = vec![
        Spanned::new(
            sl::DefKind::ExternTypD(id("T", "type"), Vec::new()),
            span("type-def"),
        ),
        Spanned::new(
            sl::DefKind::ExternRelD((
                id("R", "relation"),
                signature("signature"),
                Vec::new(),
                Vec::new(),
            )),
            span("relation-def"),
        ),
        Spanned::new(
            sl::DefKind::BuiltinDecD((
                id("f", "function"),
                Vec::new(),
                Vec::new(),
                make_type::bool_type(),
                Vec::new(),
            )),
            span("function-def"),
        ),
        Spanned::new(
            sl::DefKind::VarD(
                id("ignored", "variable"),
                make_type::bool_type(),
                Vec::new(),
            ),
            span("variable-def"),
        ),
    ];
    let context = Context::from_spec(false, &spec).unwrap();
    assert_eq!(
        context.find_type_def(&id("T", "lookup")).unwrap(),
        &TypeDef::Extern
    );
    assert!(matches!(
        context.find_relation(&id("R", "lookup")),
        Ok(relation) if matches!(relation.as_ref(), Relation::Extern(_))
    ));
    assert!(matches!(
        context.find_function(&id("f", "lookup")),
        Ok((Cursor::Global, function)) if matches!(function.as_ref(), Function::Builtin(..))
    ));
    assert!(!context.deterministic());

    let duplicate = vec![
        spec[0].clone(),
        Spanned::new(
            sl::DefKind::ExternTypD(id("T", "duplicate"), Vec::new()),
            span("duplicate-def"),
        ),
    ];
    let error = Context::from_spec(true, &duplicate).unwrap_err();
    assert_eq!(error.span, span("duplicate"));
    assert!(error.message.contains("type `T` was already defined"));
}

#[test]
fn loaded_relations_and_functions_are_shared_across_lookups_and_local_bindings() {
    let spec = vec![
        Spanned::new(
            sl::DefKind::ExternRelD((
                id("R", "relation"),
                signature("signature"),
                Vec::new(),
                Vec::new(),
            )),
            span("relation-def"),
        ),
        Spanned::new(
            sl::DefKind::BuiltinDecD((
                id("f", "function"),
                Vec::new(),
                Vec::new(),
                make_type::bool_type(),
                Vec::new(),
            )),
            span("function-def"),
        ),
    ];
    let mut context = Context::from_spec(false, &spec).unwrap();

    let relation = Rc::clone(context.find_relation(&id("R", "first")).unwrap());
    let relation_again = context.find_relation(&id("R", "second")).unwrap();
    assert!(Rc::ptr_eq(&relation, relation_again));

    let function = Rc::clone(context.find_function(&id("f", "first")).unwrap().1);
    context.enter_function(id("caller", "caller"), Vec::new(), Default::default());
    context
        .bind_function(id("local", "local"), Rc::clone(&function))
        .unwrap();
    let local = context.find_function(&id("local", "lookup")).unwrap().1;
    assert!(Rc::ptr_eq(&function, local));
}

#[test]
fn relation_and_function_frames_expose_inputs_and_expected_local_capabilities() {
    let mut context = Context::from_spec(false, &[]).unwrap();
    let input = make::bool(true, span("input"));
    context.enter_relation(id("R", "relation"), vec![input.clone()]);
    assert_eq!(
        context.input_values().unwrap(),
        std::slice::from_ref(&input)
    );
    context
        .bind_value(Variable::new(id("x", "binding"), Vec::new()), input.clone())
        .unwrap();
    assert_eq!(
        context
            .find_value(&Variable::new(id("x", "lookup"), Vec::new()))
            .unwrap(),
        &input
    );
    let error = context
        .bind_type(id("LocalT", "local-type"), TypeDef::Param)
        .unwrap_err();
    assert!(error.message.contains("relation context"));

    context.enter_function(
        id("f", "function"),
        vec![input],
        [("P".to_owned(), TypeDef::Param)].into_iter().collect(),
    );
    assert_eq!(
        context.find_type_def(&id("P", "lookup")).unwrap(),
        &TypeDef::Param
    );
    context
        .bind_function(
            id("local", "local-function"),
            Rc::new(Function::Builtin(
                Vec::new(),
                Vec::new(),
                make_type::bool_type(),
            )),
        )
        .unwrap();
    assert!(matches!(
        context.find_function(&id("local", "lookup")),
        Ok((Cursor::Local, _))
    ));
}

#[test]
fn nested_marks_restore_replaced_and_new_bindings_without_copying_the_frame() {
    let mut context = Context::from_spec(false, &[]).unwrap();
    context.enter_function(id("f", "function"), Vec::new(), Default::default());
    let variable = Variable::new(id("x", "x"), Vec::new());
    let original = make::bool(false, span("original"));
    context
        .bind_value(variable.clone(), original.clone())
        .unwrap();

    let outer = context.mark();
    let replacement = make::bool(true, span("replacement"));
    context
        .bind_value(variable.clone(), replacement.clone())
        .unwrap();
    context.bind_type(id("T", "type"), TypeDef::Param).unwrap();
    let inner = context.mark();
    let temporary = Variable::new(id("temporary", "temporary"), Vec::new());
    context
        .bind_value(
            temporary.clone(),
            make::text("temp".to_owned(), span("temp")),
        )
        .unwrap();

    context.reset(inner).unwrap();
    assert!(context.find_value(&temporary).is_err());
    assert_eq!(context.find_value(&variable).unwrap(), &replacement);
    assert_eq!(
        context.find_type_def(&id("T", "lookup")).unwrap(),
        &TypeDef::Param
    );

    context.reset(outer).unwrap();
    assert_eq!(context.find_value(&variable).unwrap(), &original);
    assert!(context.find_type_def(&id("T", "lookup")).is_err());
    assert_eq!(get::bool(context.find_value(&variable).unwrap()), Ok(false));
}

#[test]
fn marks_cannot_be_reused_after_switching_frames() {
    let mut context = Context::from_spec(false, &[]).unwrap();
    context.enter_relation(id("R", "relation"), Vec::new());
    let mark = context.mark();
    context.enter_relation(id("S", "other-relation"), Vec::new());
    let error = context.reset(mark).unwrap_err();
    assert!(error.message.contains("different local frame"));
}

#[test]
fn defined_relation_payload_survives_loading() {
    let relation = (
        id("R", "relation"),
        signature("signature"),
        vec![bool_exp(true, "match")],
        Vec::new(),
        Some(Vec::new()),
        Vec::new(),
    );
    let spec = vec![Spanned::new(
        sl::DefKind::RelD(relation),
        span("definition"),
    )];
    let context = Context::from_spec(false, &spec).unwrap();
    assert!(
        matches!(context.find_relation(&id("R", "lookup")), Ok(relation) if matches!(relation.as_ref(), Relation::Defined(_, matches, _, Some(_)) if matches.len() == 1))
    );
}

#[test]
fn optional_bindings_require_all_variables_to_agree() {
    let mut context = Context::from_spec(false, &[]).unwrap();
    context.enter_relation(id("R", "relation"), Vec::new());
    let vars = vec![
        (id("x", "x"), make_type::bool_type(), Vec::new()),
        (id("y", "y"), make_type::bool_type(), Vec::new()),
    ];
    let x = make::bool(true, span("x-value"));
    let y = make::bool(false, span("y-value"));
    context
        .bind_value(
            Variable::new(id("x", "x-bound"), vec![il::Iter::Opt]),
            make::opt(
                &make_type::opt_type(make_type::bool_type()),
                Some(x.clone()),
                span("x-opt"),
            ),
        )
        .unwrap();
    context
        .bind_value(
            Variable::new(id("y", "y-bound"), vec![il::Iter::Opt]),
            make::opt(
                &make_type::opt_type(make_type::bool_type()),
                Some(y.clone()),
                span("y-opt"),
            ),
        )
        .unwrap();
    let bindings = context.optional_bindings(&vars).unwrap().unwrap();
    assert_eq!(bindings.len(), 2);
    assert_eq!(bindings[0].0, Variable::new(id("x", "other"), Vec::new()));
    assert_eq!(bindings[0].1, x);
    assert_eq!(bindings[1].1, y);

    context
        .bind_value(
            Variable::new(id("y", "y-bound"), vec![il::Iter::Opt]),
            make::opt(
                &make_type::opt_type(make_type::bool_type()),
                None,
                span("y-none"),
            ),
        )
        .unwrap();
    let error = context.optional_bindings(&vars).unwrap_err();
    assert!(error.message.contains("mismatch in optionality"));
}

#[test]
fn list_binding_batches_transpose_rows_and_reject_ragged_values() {
    let mut context = Context::from_spec(false, &[]).unwrap();
    context.enter_relation(id("R", "relation"), Vec::new());
    let vars = vec![
        (id("x", "x"), make_type::bool_type(), Vec::new()),
        (id("y", "y"), make_type::bool_type(), Vec::new()),
    ];
    let x0 = make::bool(false, span("x0"));
    let x1 = make::bool(true, span("x1"));
    let y0 = make::bool(true, span("y0"));
    let y1 = make::bool(false, span("y1"));
    let list_type = make_type::list_type(make_type::bool_type());
    context
        .bind_value(
            Variable::new(id("x", "x-bound"), vec![il::Iter::List]),
            make::list(&list_type, vec![x0.clone(), x1.clone()], span("xs")),
        )
        .unwrap();
    context
        .bind_value(
            Variable::new(id("y", "y-bound"), vec![il::Iter::List]),
            make::list(&list_type, vec![y0.clone(), y1.clone()], span("ys")),
        )
        .unwrap();
    let batches = context.list_binding_batches(&vars).unwrap();
    assert_eq!(batches.len(), 2);
    assert_eq!(batches[0][0].1, x0);
    assert_eq!(batches[0][1].1, y0);
    assert_eq!(batches[1][0].1, x1);
    assert_eq!(batches[1][1].1, y1);

    context
        .bind_value(
            Variable::new(id("y", "y-bound"), vec![il::Iter::List]),
            make::list(
                &list_type,
                vec![make::bool(true, span("only"))],
                span("ragged"),
            ),
        )
        .unwrap();
    let error = context.list_binding_batches(&vars).unwrap_err();
    assert!(error.message.contains("cannot transpose"));
}
