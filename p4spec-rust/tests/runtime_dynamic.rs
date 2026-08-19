use std::{
    collections::{BTreeSet, HashMap, HashSet},
    rc::Rc,
};

use p4spec_rust::{
    domain::{
        mixfix::Mixfix,
        source::{Region, Spanned},
    },
    lang::{il::ast as il, sl::ast as sl},
    runtime::{
        dynamic::{envs::ValueEnv, var::Variable},
        dynamic_sl::{
            envs::{FunctionEnv, RelationEnv},
            func::Function,
            rel::Relation,
        },
        r#type::typ::make as make_type,
        value::make as make_value,
    },
};

fn span(file: &str) -> Region {
    Region::for_file(file)
}

fn id(name: &str, file: &str) -> il::Id {
    Spanned::new(name.to_owned(), span(file))
}

fn exp_param(name: &str, typ: il::Typ, file: &str) -> sl::Param {
    Spanned::new(
        sl::ParamKind::ExpP(
            typ.clone(),
            il::Exp::new(il::ExpKind::VarE(id(name, file)), typ.node, span(file)),
        ),
        span(file),
    )
}

#[test]
fn variables_ignore_id_regions_and_order_by_name_then_iterators() {
    let plain_a = Variable::new(id("value", "left"), Vec::new());
    let plain_b = Variable::new(id("value", "right"), Vec::new());
    let optional = Variable::new(id("value", "optional"), vec![il::Iter::Opt]);
    let listed = Variable::new(id("value", "listed"), vec![il::Iter::List]);
    let other = Variable::new(id("z", "other"), Vec::new());

    assert_eq!(plain_a, plain_b);
    assert_eq!(plain_a.to_string(), "value");
    assert_eq!(optional.to_string(), "value?");
    assert_eq!(listed.to_string(), "value*");

    let mut hashed = HashSet::new();
    hashed.insert(plain_a.clone());
    hashed.insert(plain_b);
    assert_eq!(hashed.len(), 1);

    let ordered: BTreeSet<_> = [
        other.clone(),
        listed.clone(),
        optional.clone(),
        plain_a.clone(),
    ]
    .into_iter()
    .collect();
    assert_eq!(
        ordered.into_iter().collect::<Vec<_>>(),
        vec![plain_a, optional, listed, other],
    );
}

#[test]
fn value_environment_is_one_hash_map_keyed_by_semantic_variables() {
    let mut values = ValueEnv::new();
    let variable_a = Variable::new(id("x", "first"), vec![il::Iter::Opt]);
    let variable_b = Variable::new(id("x", "second"), vec![il::Iter::Opt]);
    let value_a = make_value::bool(true, span("value-a"));
    let value_b = make_value::bool(false, span("value-b"));

    values.insert(variable_a, value_a);
    values.insert(variable_b.clone(), value_b.clone());

    assert_eq!(values.len(), 1);
    assert_eq!(values.get(&variable_b), Some(&value_b));
}

#[test]
fn relations_retain_defined_payloads_and_expose_signatures() {
    let signature: sl::RelSignature = (
        Spanned::new(Mixfix::Seq(Vec::new()), span("relation-type")),
        vec![0, 2],
    );
    let extern_relation = Relation::Extern(signature.clone());
    let defined_relation = Relation::Defined(
        signature.clone(),
        vec![il::Exp::new(
            il::ExpKind::BoolE(true),
            il::TypKind::BoolT,
            span("match"),
        )],
        Vec::new(),
        Some(Vec::new()),
    );

    assert_eq!(extern_relation.to_string(), "extern relation");
    assert_eq!(defined_relation.to_string(), "defined relation");
    assert_eq!(extern_relation.get_signature(), &signature);
    assert_eq!(defined_relation.get_signature(), &signature);

    let mut relations = RelationEnv::new();
    relations.insert("R".to_owned(), Rc::new(defined_relation));
    assert!(matches!(
        relations.get("R").map(Rc::as_ref),
        Some(Relation::Defined(..))
    ));
}

#[test]
fn every_function_kind_computes_the_ocaml_signature() {
    let type_param = id("T", "type-param");
    let value_param = exp_param("x", make_type::bool_type(), "value-param");
    let higher_order_param = Spanned::new(
        sl::ParamKind::DefP(
            id("callback", "callback"),
            vec![id("U", "callback-type")],
            vec![exp_param("arg", make_type::text_type(), "callback-arg")],
            make_type::int_type(),
        ),
        span("callback"),
    );
    let params = vec![value_param, higher_order_param];
    let return_type = make_type::text_type();
    let functions = [
        Function::Extern(
            vec![type_param.clone()],
            params.clone(),
            return_type.clone(),
        ),
        Function::Builtin(
            vec![type_param.clone()],
            params.clone(),
            return_type.clone(),
        ),
        Function::Table(params.clone(), return_type.clone(), Vec::new()),
        Function::Defined(
            vec![type_param.clone()],
            params,
            return_type.clone(),
            Vec::new(),
            Some(Vec::new()),
        ),
    ];

    for (index, function) in functions.iter().enumerate() {
        let signature = function.get_signature();
        if index == 2 {
            assert!(signature.type_params.is_empty());
        } else {
            assert_eq!(signature.type_params, vec![type_param.clone()]);
        }
        assert_eq!(signature.param_types.len(), 2);
        assert!(matches!(signature.param_types[0].node, il::TypKind::BoolT));
        assert!(matches!(
            signature.param_types[1].node,
            il::TypKind::FuncT(..)
        ));
        assert_eq!(signature.return_type, return_type);
    }

    assert_eq!(functions[0].to_string(), "extern function");
    assert_eq!(functions[1].to_string(), "builtin function");
    assert_eq!(functions[2].to_string(), "table function");
    assert_eq!(functions[3].to_string(), "defined function");

    let mut functions_by_name: FunctionEnv = HashMap::new();
    functions_by_name.insert("f".to_owned(), Rc::new(functions[0].clone()));
    assert!(matches!(
        functions_by_name.get("f").map(Rc::as_ref),
        Some(Function::Extern(..))
    ));
}
