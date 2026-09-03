//! Dispatch specification builtin calls to their Rust implementations.
//!
//! Construction installs the standard entries in OCaml declaration order and
//! then applies interface-specific overrides. Invocation resolves one name and
//! calls its implementation; for example, `sum_nat` dispatches to
//! `nats::sum_nat`, while `fresh_typeId` also advances instance-local state.

use std::collections::HashMap;

use crate::{
    lang::il::ast::{Id, Typ},
    runtime::value::ValueRef,
};

use super::{
    BuiltinError, BuiltinErrorKind, BuiltinResult, fresh, ints, lists, maps, nats, numerics, sets,
    texts,
};

// == Extensibility point: extra or override builtins per interface

pub type BuiltinImpl = fn(
    span: &crate::lang::common::source::Span,
    targs: &[Typ],
    values: &[ValueRef],
) -> BuiltinResult;

#[derive(Clone, Copy)]
enum BuiltinEntry {
    Pure(BuiltinImpl),
    FreshTypeId,
}

// == Builtin registry

pub struct Builtins {
    counter: u64,
    functions: HashMap<String, BuiltinEntry>,
}

impl Default for Builtins {
    fn default() -> Self {
        Self::new()
    }
}

impl Builtins {
    pub fn new() -> Self {
        Self::with_extensions([])
    }

    pub fn with_extensions<const N: usize>(entries: [(&str, BuiltinImpl); N]) -> Self {
        let mut functions = HashMap::from([
            // Nats
            (
                "sum_nat".to_owned(),
                BuiltinEntry::Pure(nats::sum_nat as BuiltinImpl),
            ),
            (
                "max_nat".to_owned(),
                BuiltinEntry::Pure(nats::max_nat as BuiltinImpl),
            ),
            (
                "min_nat".to_owned(),
                BuiltinEntry::Pure(nats::min_nat as BuiltinImpl),
            ),
            // Ints
            (
                "sum_int".to_owned(),
                BuiltinEntry::Pure(ints::sum_int as BuiltinImpl),
            ),
            (
                "max_int".to_owned(),
                BuiltinEntry::Pure(ints::max_int as BuiltinImpl),
            ),
            (
                "min_int".to_owned(),
                BuiltinEntry::Pure(ints::min_int as BuiltinImpl),
            ),
            // Texts
            (
                "text_to_int".to_owned(),
                BuiltinEntry::Pure(texts::text_to_int as BuiltinImpl),
            ),
            (
                "int_to_text".to_owned(),
                BuiltinEntry::Pure(texts::int_to_text as BuiltinImpl),
            ),
            (
                "split_text".to_owned(),
                BuiltinEntry::Pure(texts::split_text as BuiltinImpl),
            ),
            (
                "strip_prefix".to_owned(),
                BuiltinEntry::Pure(texts::strip_prefix as BuiltinImpl),
            ),
            (
                "strip_suffix".to_owned(),
                BuiltinEntry::Pure(texts::strip_suffix as BuiltinImpl),
            ),
            (
                "strip_all_whitespace".to_owned(),
                BuiltinEntry::Pure(texts::strip_all_whitespace as BuiltinImpl),
            ),
            // Lists
            (
                "rev_".to_owned(),
                BuiltinEntry::Pure(lists::rev_ as BuiltinImpl),
            ),
            (
                "concat_".to_owned(),
                BuiltinEntry::Pure(lists::concat_ as BuiltinImpl),
            ),
            (
                "distinct_".to_owned(),
                BuiltinEntry::Pure(lists::distinct_ as BuiltinImpl),
            ),
            (
                "partition_".to_owned(),
                BuiltinEntry::Pure(lists::partition_ as BuiltinImpl),
            ),
            (
                "assoc_".to_owned(),
                BuiltinEntry::Pure(lists::assoc_ as BuiltinImpl),
            ),
            (
                "sort_".to_owned(),
                BuiltinEntry::Pure(lists::sort_ as BuiltinImpl),
            ),
            (
                "transpose_".to_owned(),
                BuiltinEntry::Pure(lists::transpose_ as BuiltinImpl),
            ),
            // Sets
            (
                "intersect_set".to_owned(),
                BuiltinEntry::Pure(sets::intersect_set as BuiltinImpl),
            ),
            (
                "union_set".to_owned(),
                BuiltinEntry::Pure(sets::union_set as BuiltinImpl),
            ),
            (
                "unions_set".to_owned(),
                BuiltinEntry::Pure(sets::unions_set as BuiltinImpl),
            ),
            (
                "diff_set".to_owned(),
                BuiltinEntry::Pure(sets::diff_set as BuiltinImpl),
            ),
            (
                "sub_set".to_owned(),
                BuiltinEntry::Pure(sets::sub_set as BuiltinImpl),
            ),
            (
                "eq_set".to_owned(),
                BuiltinEntry::Pure(sets::eq_set as BuiltinImpl),
            ),
            // Maps
            (
                "find_map".to_owned(),
                BuiltinEntry::Pure(maps::find_map as BuiltinImpl),
            ),
            (
                "find_maps".to_owned(),
                BuiltinEntry::Pure(maps::find_maps as BuiltinImpl),
            ),
            (
                "add_map".to_owned(),
                BuiltinEntry::Pure(maps::add_map as BuiltinImpl),
            ),
            (
                "adds_map".to_owned(),
                BuiltinEntry::Pure(maps::adds_map as BuiltinImpl),
            ),
            (
                "update_map".to_owned(),
                BuiltinEntry::Pure(maps::update_map as BuiltinImpl),
            ),
            // Fresh type id
            ("fresh_typeId".to_owned(), BuiltinEntry::FreshTypeId),
            // Numerics
            (
                "shl".to_owned(),
                BuiltinEntry::Pure(numerics::shl as BuiltinImpl),
            ),
            (
                "shr".to_owned(),
                BuiltinEntry::Pure(numerics::shr as BuiltinImpl),
            ),
            (
                "shr_arith".to_owned(),
                BuiltinEntry::Pure(numerics::shr_arith as BuiltinImpl),
            ),
            (
                "pow2".to_owned(),
                BuiltinEntry::Pure(numerics::pow2 as BuiltinImpl),
            ),
            (
                "bitstr_to_int".to_owned(),
                BuiltinEntry::Pure(numerics::bitstr_to_int as BuiltinImpl),
            ),
            (
                "int_to_bitstr".to_owned(),
                BuiltinEntry::Pure(numerics::int_to_bitstr as BuiltinImpl),
            ),
            (
                "bits_to_int_unsigned".to_owned(),
                BuiltinEntry::Pure(numerics::bits_to_int_unsigned as BuiltinImpl),
            ),
            (
                "bits_to_int_signed".to_owned(),
                BuiltinEntry::Pure(numerics::bits_to_int_signed as BuiltinImpl),
            ),
            (
                "int_to_bits_unsigned".to_owned(),
                BuiltinEntry::Pure(numerics::int_to_bits_unsigned as BuiltinImpl),
            ),
            (
                "int_to_bits_signed".to_owned(),
                BuiltinEntry::Pure(numerics::int_to_bits_signed as BuiltinImpl),
            ),
            (
                "bneg".to_owned(),
                BuiltinEntry::Pure(numerics::bneg as BuiltinImpl),
            ),
            (
                "band".to_owned(),
                BuiltinEntry::Pure(numerics::band as BuiltinImpl),
            ),
            (
                "bxor".to_owned(),
                BuiltinEntry::Pure(numerics::bxor as BuiltinImpl),
            ),
            (
                "bor".to_owned(),
                BuiltinEntry::Pure(numerics::bor as BuiltinImpl),
            ),
            (
                "bitacc".to_owned(),
                BuiltinEntry::Pure(numerics::bitacc as BuiltinImpl),
            ),
            (
                "bitacc_replace".to_owned(),
                BuiltinEntry::Pure(numerics::bitacc_replace as BuiltinImpl),
            ),
        ]);
        // Extension entries are merged last, allowing interface-specific overrides.
        for (name, implementation) in entries {
            functions.insert(name.to_owned(), BuiltinEntry::Pure(implementation));
        }
        Self {
            counter: 0,
            functions,
        }
    }

    // Initializer

    pub fn init(&mut self) {
        self.counter = 0;
    }

    // Builtin calls

    pub fn invoke(
        &mut self,
        id: &Id,
        targs: &[Typ],
        values: &[ValueRef],
    ) -> Result<(ValueRef, bool), BuiltinError> {
        let entry = self
            .functions
            .get(&id.node)
            .copied()
            .ok_or_else(|| BuiltinError {
                kind: BuiltinErrorKind::MissingImplementation(id.node.clone()),
                span: id.span.clone(),
            })?;
        let (value, side_effected) = match entry {
            BuiltinEntry::Pure(implementation) => {
                let value = implementation(&id.span, targs, values)?;
                (value, false)
            }
            BuiltinEntry::FreshTypeId => {
                let value = fresh::fresh_type_id(&mut self.counter, &id.span, targs, values)?;
                (value, true)
            }
        };
        Ok((value, side_effected))
    }
}
