//! Borrowed views of values for syntax comparison
//!
//! A `ValueRef` pairs a value with its arena, so comparisons can read bodies:
//! values of one arena compare by canonical id,
//! values of different arenas structurally, each through its own arena.

use std::cmp::Ordering;

use crate::lang::{
    common::prim::num,
    traits::{cmp::SyntaxCmp, eq::SyntaxEq},
};

use super::{
    arena::Arena,
    flat::{Value, ValueKind},
};

// = Borrowed views

/// A value together with its arena, for comparisons that must read bodies.
#[derive(Clone, Copy, Debug)]
pub struct ValueRef<'a> {
    pub(super) arena: &'a Arena,
    pub(super) value: Value,
}

// = Syntax comparison

impl SyntaxEq for ValueRef<'_> {
    fn syntax_eq(&self, value_other: &Self) -> bool {
        // Same arena: canonical ids decide; otherwise compare structurally
        if std::ptr::eq(self.arena, value_other.arena) {
            self.arena.canon_id(&self.value) == value_other.arena.canon_id(&value_other.value)
        } else {
            self.syntax_cmp(value_other).is_eq()
        }
    }
}

impl SyntaxCmp for ValueRef<'_> {
    fn syntax_cmp(&self, value_other: &Self) -> Ordering {
        // Children are compared through their own arenas
        let compare_value = |value_l: &Value, value_r: &Value| {
            self.arena
                .view(*value_l)
                .syntax_cmp(&value_other.arena.view(*value_r))
        };
        let compare_values = |values_l: &[Value], values_r: &[Value]| {
            values_l
                .iter()
                .zip(values_r)
                .map(|(value_l, value_r)| compare_value(value_l, value_r))
                .find(|order| !order.is_eq())
                .unwrap_or_else(|| values_l.len().cmp(&values_r.len()))
        };
        let kind_l = self.arena.kind(&self.value);
        let kind_r = value_other.arena.kind(&value_other.value);
        match (kind_l, kind_r) {
            (ValueKind::Bool(value_l), ValueKind::Bool(value_r)) => value_l.cmp(value_r),
            (ValueKind::Num(value_l), ValueKind::Num(value_r)) => num::compare(value_l, value_r),
            (ValueKind::Text(value_l), ValueKind::Text(value_r)) => value_l.cmp(value_r),
            (ValueKind::Struct(value_fields_l), ValueKind::Struct(value_fields_r)) => {
                value_fields_l
                    .iter()
                    .zip(value_fields_r)
                    .map(|((atom_l, value_l), (atom_r, value_r))| {
                        atom_l
                            .node
                            .cmp(&atom_r.node)
                            .then_with(|| compare_value(value_l, value_r))
                    })
                    .find(|order| !order.is_eq())
                    .unwrap_or_else(|| value_fields_l.len().cmp(&value_fields_r.len()))
            }
            (ValueKind::Case(value_case_l), ValueKind::Case(value_case_r)) => value_case_l
                .cmp_in_by(
                    self.arena.arena_shape(),
                    value_case_r,
                    value_other.arena.arena_shape(),
                    compare_value,
                ),
            (ValueKind::Tuple(values_l), ValueKind::Tuple(values_r))
            | (ValueKind::List(values_l), ValueKind::List(values_r)) => {
                compare_values(values_l, values_r)
            }
            (ValueKind::Opt(value_l), ValueKind::Opt(value_r)) => match (value_l, value_r) {
                (Some(value_l), Some(value_r)) => compare_value(value_l, value_r),
                (None, Some(_)) => Ordering::Less,
                (Some(_), None) => Ordering::Greater,
                (None, None) => Ordering::Equal,
            },
            (ValueKind::Func(id_l), ValueKind::Func(id_r)) => id_l.node.cmp(&id_r.node),
            (ValueKind::Extern(json_l), ValueKind::Extern(json_r)) => {
                crate::util::json::compare(json_l, json_r)
            }
            // Different kinds order by tag
            _ => kind_l.tag().cmp(&kind_r.tag()),
        }
    }
}
