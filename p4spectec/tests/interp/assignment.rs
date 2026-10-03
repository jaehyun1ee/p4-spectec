use std::rc::Rc;

use p4spectec::lang::{
    common::source::Span,
    data::{
        typ,
        value::{get, make},
    },
    traits::print::Print,
};

use super::support::sl_runner;

#[test]
fn list_patterns_keep_optional_rows_and_column_order() {
    let source = r#"
var x : nat
var y : nat
dec $columns((nat, nat?)*) : (nat*, nat?*)
def $columns((x, y?)*) = (x*, y?*)
"#;
    let mut runner = sl_runner(source);
    let typ_opt: Rc<_> = typ::make::opt(typ::make::nat()).node.into();
    let typ_row = typ::make::tuple(vec![typ::make::nat(), typ::make::opt(typ::make::nat())]);
    let typ_rows: Rc<_> = typ::make::list(typ_row.clone()).node.into();
    let typ_row: Rc<_> = typ_row.node.into();
    let mut values = Vec::new();
    for (num, num_opt) in [(11_u64, Some(12)), (21, None), (31, Some(32)), (41, None)] {
        let arena = runner.arena_mut();
        let value = make::nat(arena, num.into(), Span::default()).unwrap();
        let value_opt =
            num_opt.map(|num: u64| make::nat(arena, num.into(), Span::default()).unwrap());
        let value_opt = make::opt(arena, typ_opt.clone(), value_opt, Span::default()).unwrap();
        values.push(
            make::tuple(arena, typ_row.clone(), vec![value, value_opt], Span::default()).unwrap(),
        );
    }
    let value_rows =
        make::list(runner.arena_mut(), typ_rows.clone(), values, Span::default()).unwrap();
    let value = runner
        .context()
        .call_func("columns", &[], &[value_rows])
        .unwrap();
    let arena = runner.arena();
    let values = get::tuple(arena, &value).unwrap();
    let nums = get::list(arena, &values[0])
        .unwrap()
        .iter()
        .map(|value| get::num(arena, value).unwrap().to_string())
        .collect::<Vec<_>>();
    let nums_opt = get::list(arena, &values[1])
        .unwrap()
        .iter()
        .map(|value| {
            get::opt(arena, value)
                .unwrap()
                .map(|value| get::num(arena, &value).unwrap().to_string())
        })
        .collect::<Vec<_>>();
    assert_eq!(nums, ["11", "21", "31", "41"]);
    assert_eq!(nums_opt, [Some("12".into()), None, Some("32".into()), None]);

    let value_rows = make::list(runner.arena_mut(), typ_rows, vec![], Span::default()).unwrap();
    let value = runner
        .context()
        .call_func("columns", &[], &[value_rows])
        .unwrap();
    let arena = runner.arena();
    for value in get::tuple(arena, &value).unwrap() {
        assert!(get::list(arena, value).unwrap().is_empty());
    }
}
