//! Borrowed views of values for syntax comparison
//!
//! A `ValueRef` pairs a value with its arena, so comparisons can read bodies:
//! values of one arena compare by canonical id,
//! values of different arenas structurally, each through its own arena.

use std::cmp::Ordering;

use crate::lang::{
    common::prim::num,
    data::arena::Arena,
    traits::{cmp::SyntaxCmp, eq::SyntaxEq},
};

use super::{Value, ValueFlatKind};

// = Borrowed views

/// A value together with its arena, for comparisons that must read bodies.
#[derive(Clone, Copy, Debug)]
pub struct ValueRef<'a> {
    pub(in crate::lang::data) arena: &'a Arena,
    pub(in crate::lang::data) value: Value,
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
            (ValueFlatKind::Bool(value_l), ValueFlatKind::Bool(value_r)) => value_l.cmp(value_r),
            (ValueFlatKind::Num(value_l), ValueFlatKind::Num(value_r)) => {
                num::compare(value_l, value_r)
            }
            (ValueFlatKind::Text(value_l), ValueFlatKind::Text(value_r)) => value_l.cmp(value_r),
            (ValueFlatKind::Struct(value_fields_l), ValueFlatKind::Struct(value_fields_r)) => {
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
            (ValueFlatKind::Case(value_case_l), ValueFlatKind::Case(value_case_r)) => value_case_l
                .cmp_in_by(
                    self.arena.arena_mixop(),
                    value_case_r,
                    value_other.arena.arena_mixop(),
                    compare_value,
                ),
            (ValueFlatKind::Tuple(values_l), ValueFlatKind::Tuple(values_r))
            | (ValueFlatKind::List(values_l), ValueFlatKind::List(values_r)) => {
                compare_values(values_l, values_r)
            }
            (ValueFlatKind::Opt(value_l), ValueFlatKind::Opt(value_r)) => {
                match (value_l, value_r) {
                    (Some(value_l), Some(value_r)) => compare_value(value_l, value_r),
                    (None, Some(_)) => Ordering::Less,
                    (Some(_), None) => Ordering::Greater,
                    (None, None) => Ordering::Equal,
                }
            }
            (ValueFlatKind::Func(id_l), ValueFlatKind::Func(id_r)) => id_l.node.cmp(&id_r.node),
            (ValueFlatKind::Extern(json_l), ValueFlatKind::Extern(json_r)) => {
                crate::util::json::compare(json_l, json_r)
            }
            // Different kinds order by tag
            _ => kind_l.tag().cmp(&kind_r.tag()),
        }
    }
}
