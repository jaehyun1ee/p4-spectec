//! Direct columns from flat structural list patterns
//!
//! A temporary plan contains only slot indices for the current assignment.
//! Rows are validated in their original order before output lists are built;
//! immutable row bodies then supply one final column at a time.

use smallvec::SmallVec;

use crate::lang::{
    common::source::Span,
    data::{
        value::{Arena, Value, ValueField, ValueKind, get, make},
        var::SlotIdx,
    },
};

use crate::interp::shared::{
    backtrack::{Backtrack, ok, unwrap_from_result},
    context::WriteContext,
    prepare::ast,
    util::VarIter,
};

enum RowKind {
    Tuple,
    Case,
    Struct,
    List,
}

/// A flat pattern's leaf slots and the final leaf for each output variable.
struct RowPlan {
    kind: RowKind,
    slots: SmallVec<[SlotIdx; 4]>,
    columns: SmallVec<[Option<usize>; 4]>,
}

fn collect_slots<'a>(exps: impl Iterator<Item = &'a ast::Exp>) -> Option<SmallVec<[SlotIdx; 4]>> {
    exps.map(|exp| match &exp.node {
        ast::ExpKind::Id(id) => Some(id.slot),
        _ => None,
    })
    .collect()
}

impl RowPlan {
    /// Classifies syntax without reading values or checking frame bounds.
    fn new(exp: &ast::Exp, vars: &[ast::Var]) -> Option<Self> {
        // A single structural parent may contain only identifier leaves
        let (kind, slots) = match &exp.node {
            ast::ExpKind::Tuple(exps) => (RowKind::Tuple, collect_slots(exps.iter())?),
            ast::ExpKind::Case(not_exp) => (RowKind::Case, collect_slots(not_exp.args().iter())?),
            ast::ExpKind::Str(exp_fields) => {
                (RowKind::Struct, collect_slots(exp_fields.iter().map(|exp_field| &exp_field.exp))?)
            }
            ast::ExpKind::List(exps) => (RowKind::List, collect_slots(exps.iter())?),
            _ => return None,
        };
        // Repeated writes bind the last leaf; repeated outputs keep every column
        let columns = vars
            .iter()
            .map(|var| slots.iter().rposition(|slot| *slot == var.slot))
            .collect();
        Some(Self { kind, slots, columns })
    }

    /// Borrows row children while retaining the ordinary parent-kind check.
    fn children<'a>(&self, arena: &'a Arena, value: &Value) -> RowChildren<'a> {
        match (&self.kind, arena.kind(value)) {
            (RowKind::Tuple, ValueKind::Tuple(values))
            | (RowKind::List, ValueKind::List(values)) => RowChildren::Sequence(values),
            (RowKind::Case, ValueKind::Case(value_case)) => {
                RowChildren::Sequence(value_case.args())
            }
            (RowKind::Struct, ValueKind::Struct(value_fields)) => RowChildren::Fields(value_fields),
            _ => unreachable!("assignment pattern must match the value"),
        }
    }
}

enum RowChildren<'a> {
    Sequence(&'a [Value]),
    Fields(&'a [ValueField]),
}

impl RowChildren<'_> {
    fn len(&self) -> usize {
        match self {
            Self::Sequence(values) => values.len(),
            Self::Fields(value_fields) => value_fields.len(),
        }
    }

    fn get(&self, idx: usize) -> Value {
        match self {
            Self::Sequence(values) => values[idx],
            Self::Fields(value_fields) => value_fields[idx].1,
        }
    }
}

/// Collects flat rows only for contexts whose private bindings have no effects.
pub(crate) fn assign(
    arena: &mut Arena,
    ctx: &mut impl WriteContext,
    span: &Span,
    exp: &ast::Exp,
    vars: &[ast::Var],
    vars_outer: &[VarIter<'_>],
    value: Value,
) -> Option<Backtrack<()>> {
    // Custom contexts retain their ordinary clone, clear, read, and write calls
    if !ctx.can_collect_pattern_rows() {
        return None;
    }
    let plan = RowPlan::new(exp, vars)?;
    Some(assign_plan(arena, ctx, span, vars, vars_outer, value, &plan))
}

/// Validates all rows before constructing output columns in variable order.
fn assign_plan(
    arena: &mut Arena,
    ctx: &mut impl WriteContext,
    span: &Span,
    vars: &[ast::Var],
    vars_outer: &[VarIter<'_>],
    value: Value,
    plan: &RowPlan,
) -> Backtrack<()> {
    let values_rows = get::list(arena, &value).expect("iteration assignment value must be a list");
    let len = values_rows.len();
    // Row clearing checks every output slot before checking the row pattern
    for value in values_rows {
        for var in vars {
            let _ = ctx.find_value_at_slot(var.slot);
        }
        let children = plan.children(arena, value);
        assert_eq!(plan.slots.len(), children.len(), "assignment arity mismatch");
        // Every identifier still reads its body and checks its destination slot
        for (idx, slot) in plan.slots.iter().enumerate() {
            let _ = arena.kind(&children.get(idx));
            let _ = ctx.find_value_at_slot(*slot);
        }
    }
    // Each column retains its fresh type and its original allocation position
    for (var_outer, col) in vars_outer.iter().zip(&plan.columns) {
        let typ = var_outer.typ();
        // Missing columns fail only after all rows and all earlier output lists
        let col = if len == 0 { None } else { Some(col.expect("value must be bound")) };
        let values = if let Some(col) = col {
            // Prior output construction may move arena storage; refetch each column
            get::list(arena, &value)
                .expect("iteration assignment value must be a list")
                .iter()
                .map(|value| plan.children(arena, value).get(col))
                .collect()
        } else {
            Vec::new()
        };
        let value =
            unwrap_from_result!(make::list(arena, typ.node.into(), values, Span::default()), span);
        ctx.add_value_at_slot(var_outer.slot, value);
    }
    ok!(())
}
