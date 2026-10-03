use std::rc::Rc;

use p4spectec::{
    lang::{
        common::{
            notation::atom::Atom,
            source::{FileId, Position, Span},
        },
        data::{
            intern::CanonId,
            notation::{Mixfix, Mixop},
            typ,
            value::{Arena, Value, ValueArgs, ValueKind, make},
        },
    },
    phrase,
};

#[derive(Clone, Copy)]
enum Kind {
    Case,
    Tuple,
    List,
}

/// Runs mixed exact hits and fresh annotations with either constructor path.
fn sequence(
    kind: Kind,
    len: usize,
    optimized: bool,
    query_first: bool,
) -> Vec<(Value, CanonId<ValueKind>, String)> {
    let mut arena = Arena::new();
    let file = FileId::intern("args-test.watsup");
    let span = Span::new(Position::new(file, 3, 1), Position::new(file, 3, 2));
    let mut values: Vec<_> = (0..len)
        .map(|idx| make::nat(&mut arena, (idx as u64).into(), Span::default()).unwrap())
        .collect();
    let mut records = Vec::new();
    let typ = Rc::new(match kind {
        Kind::Tuple => typ::make::tuple(vec![typ::make::nat(); len]).node,
        Kind::List => typ::make::list(typ::make::nat()).node,
        Kind::Case => {
            typ::make::var(phrase!(node: Rc::from("choice"), span: Span::default()), vec![]).node
        }
    });
    // Change child notes and notation spans independently of canonical meaning
    for idx in 0..6 {
        if idx == 2 && !values.is_empty() {
            values[0] = make::new(
                &mut arena,
                ValueKind::Num(p4spectec::lang::common::prim::num::Number::Nat(0_u64.into())),
                Rc::new(typ::make::nat().node),
                span,
            )
            .unwrap();
        }
        let span_atom = if idx >= 4 { span } else { Span::default() };
        let mut mixops =
            vec![Mixop::Atom(phrase!(node: Atom::Keyword("C".into()), span: span_atom))];
        mixops.extend((0..len).map(|_| Mixop::Arg));
        let mixop = Rc::new(Mixop::Seq(mixops));
        let typ = if idx == 3 { Rc::new(typ.as_ref().clone()) } else { Rc::clone(&typ) };
        let span_value = if idx == 3 { span } else { Span::default() };
        let query = optimized && ((idx % 2 == 0) == query_first);
        // Alternate query and owned construction to test both lookup directions
        let value = match (kind, query) {
            (Kind::Case, true) => make::case_from_args(
                &mut arena,
                typ,
                &mixop,
                ValueArgs::from_slice(&values),
                span_value,
            ),
            (Kind::Case, false) => {
                make::case(&mut arena, typ, Mixfix::new(mixop, values.clone()).unwrap(), span_value)
            }
            (Kind::Tuple, true) => {
                make::tuple_from_args(&mut arena, typ, ValueArgs::from_slice(&values), span_value)
            }
            (Kind::Tuple, false) => make::tuple(&mut arena, typ, values.clone(), span_value),
            (Kind::List, true) => {
                make::list_from_args(&mut arena, typ, ValueArgs::from_slice(&values), span_value)
            }
            (Kind::List, false) => make::list(&mut arena, typ, values.clone(), span_value),
        }
        .unwrap();
        records.push((value, arena.canon_id(&value), arena.to_string(&value)));
        // A sentinel reveals any annotation or body insertion shifted by a hit
        let value = make::text(&mut arena, format!("sentinel-{idx}"), Span::default()).unwrap();
        records.push((value, arena.canon_id(&value), arena.to_string(&value)));
    }
    records
}

#[test]
fn composite_queries_preserve_handles_annotations_and_canonical_classes() {
    for kind in [Kind::Case, Kind::Tuple, Kind::List] {
        for len in [0, 1, 4, 5] {
            let records = sequence(kind, len, false, false);
            for query_first in [false, true] {
                assert_eq!(sequence(kind, len, true, query_first), records);
            }
            if len > 0 {
                assert_ne!(records[0].0.node, records[4].0.node);
                assert_eq!(records[0].1, records[4].1);
            }
            assert_eq!(records[4].0.node, records[6].0.node);
            assert_ne!(records[4].0.note, records[6].0.note);
            assert_ne!(records[4].0.span, records[6].0.span);
            if matches!(kind, Kind::Case) {
                assert_ne!(records[6].0.node, records[8].0.node);
                assert_eq!(records[6].1, records[8].1);
            }
        }
    }
}
