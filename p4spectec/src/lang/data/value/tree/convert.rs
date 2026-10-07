//! Interning tree values as flat values
//!
//! `into_flat` interns child values before their parent,
//! preserving types, source spans, and field and argument order.

use crate::lang::data::{
    arena::Arena,
    notation::{self, flat::make as make_notation},
};

use super::super::{ValueError, flat};
use super::{Value, ValueCase, ValueKind};

// = Values

impl Value {
    /// Interns the tree in the target arena, preserving its type and source span.
    pub fn into_flat(self, arena: &mut Arena) -> Result<flat::Value, ValueError> {
        let kind = self.node.into_flat(arena)?;
        arena.alloc(kind, self.note.into(), self.span)
    }
}

// = Bodies

impl ValueKind {
    /// Interns child trees in field and element order.
    pub(in crate::lang::data::value) fn into_flat(
        self,
        arena: &mut Arena,
    ) -> Result<flat::ValueKind, ValueError> {
        Ok(match self {
            Self::Bool(value) => flat::ValueKind::Bool(value),
            Self::Num(num) => flat::ValueKind::Num(num),
            Self::Text(text) => flat::ValueKind::Text(text),
            Self::Struct(value_fields) => flat::ValueKind::Struct(
                value_fields
                    .into_iter()
                    .map(|(atom, value)| Ok((atom, value.into_flat(arena)?)))
                    .collect::<Result<_, ValueError>>()?,
            ),
            Self::Case(value_case) => flat::ValueKind::Case(value_case.into_flat(arena)?),
            Self::Tuple(values) => flat::ValueKind::Tuple(
                values
                    .into_iter()
                    .map(|value| value.into_flat(arena))
                    .collect::<Result<_, _>>()?,
            ),
            Self::Opt(value) => {
                flat::ValueKind::Opt(value.map(|value| value.into_flat(arena)).transpose()?)
            }
            Self::List(values) => flat::ValueKind::List(
                values
                    .into_iter()
                    .map(|value| value.into_flat(arena))
                    .collect::<Result<_, _>>()?,
            ),
            Self::Func(id) => flat::ValueKind::Func(id),
            Self::Extern(json) => flat::ValueKind::Extern(json),
        })
    }
}

// = Cases

impl ValueCase {
    /// Interns arguments in notation order, then their mixop.
    fn into_flat(self, arena: &mut Arena) -> Result<flat::ValueCase, ValueError> {
        // Intern argument values before the case's mixop
        let (mixop, values) = self.into_parts();
        let values = values
            .into_iter()
            .map(|value| value.into_flat(arena))
            .collect::<Result<_, _>>()?;
        let arena_mixop = arena.arena_mixop_mut();
        let mixop = mixop.into_flat(arena_mixop)?;

        // A filled tree supplies one value per argument position
        Ok(make_notation::new(arena_mixop, mixop, values)
            .expect("a tree case fills every position"))
    }

    /// Splits a filled tree into its mixop and arguments in notation order.
    fn into_parts(self) -> (notation::tree::Mixop, Vec<Value>) {
        let mut values = Vec::new();
        let mixop = self.into_parts_inner(&mut values);
        (mixop, values)
    }

    /// Moves each argument into the output as its position is visited.
    fn into_parts_inner(self, values: &mut Vec<Value>) -> notation::tree::Mixop {
        match self {
            Self::Arg(value) => {
                values.push(*value);
                notation::tree::Mixop::Arg
            }
            Self::Atom(atom) => notation::tree::Mixop::Atom(atom),
            Self::Brack(atom_l, value_case, atom_r) => notation::tree::Mixop::Brack(
                atom_l,
                Box::new(value_case.into_parts_inner(values)),
                atom_r,
            ),
            Self::Infix(value_case_l, atom, value_case_r) => {
                let mixop_l = value_case_l.into_parts_inner(values);
                let mixop_r = value_case_r.into_parts_inner(values);
                notation::tree::Mixop::Infix(Box::new(mixop_l), atom, Box::new(mixop_r))
            }
            Self::Seq(value_cases) => notation::tree::Mixop::Seq(
                value_cases
                    .into_iter()
                    .map(|value_case| value_case.into_parts_inner(values))
                    .collect(),
            ),
        }
    }
}
