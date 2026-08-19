use crate::{
    domain::source::{Region, Spanned},
    interp::{common::InterpError, sl::context::Context},
    lang::il::ast as il,
    runtime::{
        r#type::{envs::TypeDefMap, typ::make as make_type, typdef::TypeDef},
        value::{ValueRef, get, make},
    },
};

use super::{FunctionCalls, eval_with_calls};

fn id(name: &str) -> il::Id {
    Spanned::new(name.to_owned(), Region::for_file(name))
}

struct RecordingCalls {
    id: Option<il::Id>,
    type_args: Vec<il::Typ>,
    values: Vec<ValueRef>,
}

impl FunctionCalls for RecordingCalls {
    fn invoke_func(
        &mut self,
        _context: &mut Context,
        id: &il::Id,
        type_args: &[il::Typ],
        values: &[ValueRef],
    ) -> Result<ValueRef, InterpError> {
        self.id = Some(id.clone());
        self.type_args = type_args.to_vec();
        self.values = values.to_vec();
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
    };

    let result = eval_with_calls(&mut context, &mut calls, &call).expect("call succeeds");
    assert_eq!(get::bool(&result), Ok(true));
    assert_eq!(calls.id.as_ref().map(|id| id.node.as_str()), Some("callee"));
    assert_eq!(calls.type_args, vec![make_type::text_type()]);
    assert_eq!(get::text(&calls.values[0]), Ok("input"));
    assert_eq!(get::func(&calls.values[1]).unwrap().node, "higher");
}
