use std::{cell::RefCell, rc::Rc};

use p4spectec::{
    interp::{
        shared::{
            context::{ReadContext, WriteContext},
            error::Error,
            eval::assign::assign_exp,
            prepare::ast,
        },
        sl::context::{Context, Global},
    },
    lang::{
        common::{
            Iter,
            notation::atom::Atom,
            prim::num::Number,
            source::{FileId, Position, Span},
        },
        data::{
            notation::{Mixfix, Mixop},
            typ,
            value::{Arena, Value, ValueKind, get, make},
            var::{IdSlot, SlotIdx, Var, VarSlot},
        },
    },
    note_phrase, phrase,
    runtime::{envs::interp::shared::frame::FrameLayout, typdef::TypeDef},
};

/// A public context implementation that keeps the default assignment path.
struct Ordinary<'a>(Context<'a>, Rc<RefCell<Vec<&'static str>>>);
impl Clone for Ordinary<'_> {
    fn clone(&self) -> Self {
        self.1.borrow_mut().push("clone");
        Self(self.0.clone(), self.1.clone())
    }
}
impl ReadContext for Ordinary<'_> {
    type Func = <Context<'static> as ReadContext>::Func;
    fn find_value_at_slot(&self, slot: SlotIdx) -> Option<&Value> {
        self.1.borrow_mut().push("find");
        self.0.find_value_at_slot(slot)
    }
    fn find_var_iterated(&self, var: &VarSlot, iter: Iter) -> VarSlot {
        self.0.find_var_iterated(var, iter)
    }
    fn find_slot_iterated(&self, var: &VarSlot, iter: Iter) -> SlotIdx {
        self.0.find_slot_iterated(var, iter)
    }
    fn find_typdef_opt(&self, id: &ast::Id) -> Option<&TypeDef> {
        self.0.find_typdef_opt(id)
    }
    fn find_typdef_local_opt(&self, id: &ast::Id) -> Option<&TypeDef> {
        self.0.find_typdef_local_opt(id)
    }
    fn find_defined_typdef(&self, id: &ast::Id) -> Result<(&[ast::TParam], &ast::DefTyp), Error> {
        self.0.find_defined_typdef(id)
    }
    fn find_func(&self, id: &ast::Id) -> Result<&Rc<Self::Func>, Error> {
        self.0.find_func(id)
    }
    fn find_func_typ(&self, id: &ast::Id) -> Result<ast::FuncTyp, Error> {
        self.0.find_func_typ(id)
    }
}
impl WriteContext for Ordinary<'_> {
    fn add_typdef_local(&mut self, id: ast::Id, typdef: TypeDef) -> Result<(), Error> {
        self.0.add_typdef_local(id, typdef)
    }
    fn add_value_at_slot(&mut self, slot: SlotIdx, value: Value) {
        self.1.borrow_mut().push("set");
        self.0.add_value_at_slot(slot, value)
    }
    fn remove_value_at_slot(&mut self, slot: SlotIdx) {
        self.1.borrow_mut().push("remove");
        self.0.remove_value_at_slot(slot)
    }
    fn clear_value_bindings(&mut self) {
        self.1.borrow_mut().push("clear");
        self.0.clear_value_bindings()
    }
    fn add_func(&mut self, id: ast::Id, func: Rc<Self::Func>) -> Result<(), Error> {
        self.0.add_func(id, func)
    }
}

#[derive(Clone, Copy, Debug)]
enum Root {
    Tuple,
    Case,
    Struct,
    List,
}

struct Fixture {
    layout: Rc<FrameLayout>,
    vars: Vec<VarSlot>,
    slots_outer: Vec<SlotIdx>,
    leaves: Vec<usize>,
    outputs: Vec<usize>,
    exp: ast::Exp,
}

fn mixop(width: usize, name: &str) -> Rc<Mixop> {
    Rc::new(Mixop::Seq(
        std::iter::once(Mixop::Atom(
            phrase!(node: Atom::Keyword(name.into()), span: Span::default()),
        ))
        .chain(std::iter::repeat_n(Mixop::Arg, width))
        .collect(),
    ))
}

fn fixture(root: Root, width: usize, missing: bool) -> Fixture {
    let mut layout = FrameLayout::default();
    let vars: Vec<_> = (0..=width)
        .map(|idx| {
            layout.resolve_var(Var {
                id: phrase!(node: Rc::from(format!("v{idx}")), span: Span::default()),
                typ: typ::make::nat(),
                iters: vec![],
            })
        })
        .collect();
    let slots_outer = vars
        .iter()
        .map(|var| {
            let mut var = var.var.clone();
            var.iters.push(Iter::List);
            layout.resolve_var(var).slot
        })
        .collect();
    let mut leaves: Vec<_> = (0..width).collect();
    if width > 1 {
        leaves[width - 1] = 0;
    }
    let mut outputs: Vec<_> = (0..width)
        .rev()
        .filter(|idx| leaves.contains(idx))
        .collect();
    if width > 0 {
        outputs.push(0);
    }
    if missing {
        outputs.insert(outputs.len().min(1), width);
    }
    let exps: Vec<_> = leaves.iter().map(|idx| {
        let var=&vars[*idx];
        note_phrase!(node: ast::ExpKind::Id(IdSlot { id: var.var.id.clone(), slot: var.slot }), note: typ::make::nat().node, span: Span::default())
    }).collect();
    let exp_kind=match root {
        Root::Tuple=>ast::ExpKind::Tuple(exps),
        Root::Case=>ast::ExpKind::Case(Box::new(Mixfix::new(mixop(width,"PATTERN"),exps).unwrap())),
        Root::Struct=>ast::ExpKind::Str(exps.into_iter().enumerate().map(|(idx,exp)|ast::ExpField { atom:phrase!(node: Atom::Keyword(format!("pattern{idx}")), span: Span::default()),exp }).collect()),
        Root::List=>ast::ExpKind::List(exps),
    };
    let exp_inner =
        note_phrase!(node: exp_kind, note: typ::make::nat().node, span: Span::default());
    let exp = note_phrase!(node: ast::ExpKind::Iter(Box::new(exp_inner),ast::ExpIter {iter: Iter::List,vars: outputs.iter().map(|idx|vars[*idx].clone()).collect()}),note: typ::make::list(typ::make::nat()).node,span: Span::default());
    Fixture { layout: Rc::new(layout), vars, slots_outer, leaves, outputs, exp }
}

fn rows(
    arena: &mut Arena,
    root: Root,
    width: usize,
    len: usize,
    bad: Option<(usize, bool)>,
) -> (Value, Vec<Vec<Value>>) {
    let mut rows = Vec::new();
    let mut values_by_row = Vec::new();
    for idx in 0..len {
        let file = FileId::intern("flat-pattern-input.p4");
        let span = Span::new(Position::new(file, idx + 1, 1), Position::new(file, idx + 1, 4));
        let values: Vec<_> = (0..width)
            .map(|col| {
                make::new(
                    arena,
                    ValueKind::Num(Number::Nat(((idx * width + col + 1) as u64).into())),
                    typ::make::nat().node.into(),
                    span,
                )
                .unwrap()
            })
            .collect();
        values_by_row.push(values.clone());
        let typ = typ::make::nat().node.into();
        let mut values = values;
        if bad == Some((idx, true)) {
            values.pop();
        }
        let value = if bad == Some((idx, false)) {
            make::bool(arena, false, span).unwrap()
        } else {
            match root {
                Root::Tuple => make::tuple(arena, typ, values, span).unwrap(),
                Root::Case => make::case(
                    arena,
                    typ,
                    Mixfix::new(mixop(values.len(), "VALUE"), values).unwrap(),
                    span,
                )
                .unwrap(),
                Root::Struct => make::structure(
                    arena,
                    typ,
                    values
                        .into_iter()
                        .enumerate()
                        .map(|(idx, value)| {
                            (phrase!(node: Atom::Keyword(format!("value{idx}")),span: span), value)
                        })
                        .collect(),
                    span,
                )
                .unwrap(),
                Root::List => make::list(arena, typ, values, span).unwrap(),
            }
        };
        rows.push(value);
    }
    (
        make::list(arena, typ::make::list(typ::make::nat()).node.into(), rows, Span::default())
            .unwrap(),
        values_by_row,
    )
}

fn sentinel(arena: &mut Arena) -> Value {
    make::new(arena, ValueKind::Bool(true), Rc::new(typ::make::bool().node), Span::default())
        .unwrap()
}

#[test]
fn flat_rows_keep_duplicates_columns_annotations_and_fresh_handles() {
    let global = Global::load(vec![]).unwrap();
    for root in [Root::Tuple, Root::Case, Root::Struct, Root::List] {
        for width in [0, 1, 3, 7, 23] {
            let fixture = fixture(root, width, false);
            for len in [0, 1, 9] {
                for _ in 0..2 {
                    let mut arena_a = Arena::new();
                    let mut arena_b = Arena::new();
                    let (value_a, values_a) = rows(&mut arena_a, root, width, len, None);
                    let (value_b, values_b) = rows(&mut arena_b, root, width, len, None);
                    assert_eq!(value_a, value_b);
                    assert_eq!(values_a, values_b);
                    let mut ctx = Context::new(&global).localize_with_layout(&fixture.layout);
                    for var in &fixture.vars {
                        ctx.add_value_at_slot(var.slot, value_a);
                    }
                    let trace = Rc::new(RefCell::new(Vec::new()));
                    let ctx_a =
                        assign_exp(&mut arena_a, ctx.clone(), &fixture.exp, value_a).unwrap();
                    let ctx_b = assign_exp(
                        &mut arena_b,
                        Ordinary(ctx.clone(), trace.clone()),
                        &fixture.exp,
                        value_b,
                    )
                    .unwrap();
                    for idx in &fixture.outputs {
                        let slot = fixture.slots_outer[*idx];
                        let value_a = *ctx_a.find_value_at_slot(slot).unwrap();
                        let value_b = *ctx_b.find_value_at_slot(slot).unwrap();
                        assert_eq!(value_a, value_b, "{root:?} width={width} len={len}");
                        let col = fixture.leaves.iter().rposition(|leaf| leaf == idx).unwrap();
                        let expected: Vec<_> = values_a.iter().map(|values| values[col]).collect();
                        assert_eq!(get::list(&arena_a, &value_a).unwrap(), expected);
                        assert_eq!(get::list(&arena_b, &value_b).unwrap(), expected);
                        assert_eq!(arena_a.to_string(&value_a), arena_b.to_string(&value_b));
                    }
                    for var in &fixture.vars {
                        assert_eq!(ctx.find_value_at_slot(var.slot), Some(&value_a));
                    }
                    assert!(trace.borrow().contains(&"clear"));
                    if len > 0 && !fixture.outputs.is_empty() {
                        assert!(trace.borrow().contains(&"remove"));
                    }
                    assert_eq!(sentinel(&mut arena_a), sentinel(&mut arena_b));
                }
            }
        }
    }
}

fn outcome<Ctx: WriteContext>(arena: &mut Arena, ctx: Ctx, exp: &ast::Exp, value: Value) -> String {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        assign_exp(arena, ctx, exp, value)
    })) {
        Ok(Ok(_)) => "ok".into(),
        Ok(Err(error)) => format!("error {error:?}"),
        Err(error) => error
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| error.downcast_ref::<&str>().map(|text| (*text).into()))
            .unwrap(),
    }
}

#[test]
fn flat_rows_keep_late_row_errors_before_missing_columns() {
    let global = Global::load(vec![]).unwrap();
    for root in [Root::Tuple, Root::Case, Root::Struct, Root::List] {
        for (len, bad) in
            [(0, None), (1, None), (3, None), (3, Some((2, false))), (3, Some((2, true)))]
        {
            let fixture = fixture(root, 3, true);
            let mut arena_a = Arena::new();
            let mut arena_b = Arena::new();
            let (value_a, _) = rows(&mut arena_a, root, 3, len, bad);
            let (value_b, _) = rows(&mut arena_b, root, 3, len, bad);
            let mut ctx = Context::new(&global).localize_with_layout(&fixture.layout);
            for var in &fixture.vars {
                ctx.add_value_at_slot(var.slot, value_a);
            }
            let text_a = outcome(&mut arena_a, ctx.clone(), &fixture.exp, value_a);
            let text_b = outcome(
                &mut arena_b,
                Ordinary(ctx, Rc::new(RefCell::new(Vec::new()))),
                &fixture.exp,
                value_b,
            );
            assert_eq!(text_a, text_b);
            assert!(if len == 0 {
                text_a == "ok"
            } else if bad == Some((2, false)) {
                text_a.contains("assignment pattern must match the value")
            } else if bad == Some((2, true)) {
                text_a.contains("assignment arity mismatch")
            } else {
                text_a.contains("value must be bound")
            });
            assert_eq!(sentinel(&mut arena_a), sentinel(&mut arena_b));
        }
    }
}

#[test]
fn flat_rows_check_overwritten_uncollected_leaf_slots_only_for_present_rows() {
    let global = Global::load(vec![]).unwrap();
    for root in [Root::Tuple, Root::Case, Root::Struct, Root::List] {
        for len in [0, 1] {
            let mut fixture = fixture(root, 3, false);
            let ast::ExpKind::Iter(exp_inner, _) = &mut fixture.exp.node else { unreachable!() };
            let exp = match &mut exp_inner.node {
                ast::ExpKind::Tuple(exps) | ast::ExpKind::List(exps) => &mut exps[0],
                ast::ExpKind::Case(not_exp) => &mut not_exp.args_mut()[0],
                ast::ExpKind::Str(fields) => &mut fields[0].exp,
                _ => unreachable!(),
            };
            let ast::ExpKind::Id(id) = &mut exp.node else { unreachable!() };
            let mut layout_invalid = FrameLayout::default();
            for idx in 0..=fixture.layout.len() {
                id.slot = layout_invalid
                    .resolve_id(
                        phrase!(node: Rc::from(format!("outside{idx}")),span: Span::default()),
                    )
                    .slot;
            }
            let mut arena_a = Arena::new();
            let mut arena_b = Arena::new();
            let (value_a, _) = rows(&mut arena_a, root, 3, len, None);
            let (value_b, _) = rows(&mut arena_b, root, 3, len, None);
            let ctx = Context::new(&global).localize_with_layout(&fixture.layout);
            let text_a = outcome(&mut arena_a, ctx.clone(), &fixture.exp, value_a);
            let text_b = outcome(
                &mut arena_b,
                Ordinary(ctx, Rc::new(RefCell::new(Vec::new()))),
                &fixture.exp,
                value_b,
            );
            assert_eq!(text_a, text_b);
            assert!(if len == 0 { text_a == "ok" } else { text_a.contains("index out of bounds") });
            assert_eq!(sentinel(&mut arena_a), sentinel(&mut arena_b));
        }
    }
}
