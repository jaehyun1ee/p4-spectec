use std::collections::HashMap;

use crate::{
    lang::il::ast::{Id, Typ},
    runtime::value::ValueRef,
};

use super::{BuiltinError, BuiltinResult, fresh, ints, lists, maps, nats, numerics, sets, texts};

// Extensibility point: extra or override builtins per interface

pub type BuiltinImpl = fn(
    add: &mut dyn FnMut(ValueRef),
    span: &crate::domain::source::Region,
    type_args: &[Typ],
    values: &[ValueRef],
) -> BuiltinResult;

#[derive(Clone, Copy)]
enum Entry {
    Pure(BuiltinImpl),
    FreshTypeId,
}

// Create builtins from entries containing extensions

pub struct Builtins {
    counter: u64,
    functions: HashMap<String, Entry>,
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
                Entry::Pure(nats::sum_nat as BuiltinImpl),
            ),
            (
                "max_nat".to_owned(),
                Entry::Pure(nats::max_nat as BuiltinImpl),
            ),
            (
                "min_nat".to_owned(),
                Entry::Pure(nats::min_nat as BuiltinImpl),
            ),
            // Ints
            (
                "sum_int".to_owned(),
                Entry::Pure(ints::sum_int as BuiltinImpl),
            ),
            (
                "max_int".to_owned(),
                Entry::Pure(ints::max_int as BuiltinImpl),
            ),
            (
                "min_int".to_owned(),
                Entry::Pure(ints::min_int as BuiltinImpl),
            ),
            // Texts
            (
                "text_to_int".to_owned(),
                Entry::Pure(texts::text_to_int as BuiltinImpl),
            ),
            (
                "int_to_text".to_owned(),
                Entry::Pure(texts::int_to_text as BuiltinImpl),
            ),
            (
                "split_text".to_owned(),
                Entry::Pure(texts::split_text as BuiltinImpl),
            ),
            (
                "strip_prefix".to_owned(),
                Entry::Pure(texts::strip_prefix as BuiltinImpl),
            ),
            (
                "strip_suffix".to_owned(),
                Entry::Pure(texts::strip_suffix as BuiltinImpl),
            ),
            (
                "strip_all_whitespace".to_owned(),
                Entry::Pure(texts::strip_all_whitespace as BuiltinImpl),
            ),
            // Lists
            ("rev_".to_owned(), Entry::Pure(lists::rev_ as BuiltinImpl)),
            (
                "concat_".to_owned(),
                Entry::Pure(lists::concat_ as BuiltinImpl),
            ),
            (
                "distinct_".to_owned(),
                Entry::Pure(lists::distinct_ as BuiltinImpl),
            ),
            (
                "partition_".to_owned(),
                Entry::Pure(lists::partition_ as BuiltinImpl),
            ),
            (
                "assoc_".to_owned(),
                Entry::Pure(lists::assoc_ as BuiltinImpl),
            ),
            ("sort_".to_owned(), Entry::Pure(lists::sort_ as BuiltinImpl)),
            (
                "transpose_".to_owned(),
                Entry::Pure(lists::transpose_ as BuiltinImpl),
            ),
            // Sets
            (
                "intersect_set".to_owned(),
                Entry::Pure(sets::intersect_set as BuiltinImpl),
            ),
            (
                "union_set".to_owned(),
                Entry::Pure(sets::union_set as BuiltinImpl),
            ),
            (
                "unions_set".to_owned(),
                Entry::Pure(sets::unions_set as BuiltinImpl),
            ),
            (
                "diff_set".to_owned(),
                Entry::Pure(sets::diff_set as BuiltinImpl),
            ),
            (
                "sub_set".to_owned(),
                Entry::Pure(sets::sub_set as BuiltinImpl),
            ),
            (
                "eq_set".to_owned(),
                Entry::Pure(sets::eq_set as BuiltinImpl),
            ),
            // Maps
            (
                "find_map".to_owned(),
                Entry::Pure(maps::find_map as BuiltinImpl),
            ),
            (
                "find_maps".to_owned(),
                Entry::Pure(maps::find_maps as BuiltinImpl),
            ),
            (
                "add_map".to_owned(),
                Entry::Pure(maps::add_map as BuiltinImpl),
            ),
            (
                "adds_map".to_owned(),
                Entry::Pure(maps::adds_map as BuiltinImpl),
            ),
            (
                "update_map".to_owned(),
                Entry::Pure(maps::update_map as BuiltinImpl),
            ),
            // Fresh type id
            ("fresh_typeId".to_owned(), Entry::FreshTypeId),
            // Numerics
            ("shl".to_owned(), Entry::Pure(numerics::shl as BuiltinImpl)),
            ("shr".to_owned(), Entry::Pure(numerics::shr as BuiltinImpl)),
            (
                "shr_arith".to_owned(),
                Entry::Pure(numerics::shr_arith as BuiltinImpl),
            ),
            (
                "pow2".to_owned(),
                Entry::Pure(numerics::pow2 as BuiltinImpl),
            ),
            (
                "bitstr_to_int".to_owned(),
                Entry::Pure(numerics::bitstr_to_int as BuiltinImpl),
            ),
            (
                "int_to_bitstr".to_owned(),
                Entry::Pure(numerics::int_to_bitstr as BuiltinImpl),
            ),
            (
                "bits_to_int_unsigned".to_owned(),
                Entry::Pure(numerics::bits_to_int_unsigned as BuiltinImpl),
            ),
            (
                "bits_to_int_signed".to_owned(),
                Entry::Pure(numerics::bits_to_int_signed as BuiltinImpl),
            ),
            (
                "int_to_bits_unsigned".to_owned(),
                Entry::Pure(numerics::int_to_bits_unsigned as BuiltinImpl),
            ),
            (
                "int_to_bits_signed".to_owned(),
                Entry::Pure(numerics::int_to_bits_signed as BuiltinImpl),
            ),
            (
                "bneg".to_owned(),
                Entry::Pure(numerics::bneg as BuiltinImpl),
            ),
            (
                "band".to_owned(),
                Entry::Pure(numerics::band as BuiltinImpl),
            ),
            (
                "bxor".to_owned(),
                Entry::Pure(numerics::bxor as BuiltinImpl),
            ),
            ("bor".to_owned(), Entry::Pure(numerics::bor as BuiltinImpl)),
            (
                "bitacc".to_owned(),
                Entry::Pure(numerics::bitacc as BuiltinImpl),
            ),
            (
                "bitacc_replace".to_owned(),
                Entry::Pure(numerics::bitacc_replace as BuiltinImpl),
            ),
        ]);
        // Extension entries are merged last, allowing interface-specific overrides.
        for (name, implementation) in entries {
            functions.insert(name.to_owned(), Entry::Pure(implementation));
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

    // State management

    pub fn checkpoint(&self) -> u64 {
        self.counter
    }

    pub fn side_effected(before: u64, after: u64) -> bool {
        before != after
    }

    // Builtin calls

    pub fn invoke(
        &mut self,
        add: &mut dyn FnMut(ValueRef),
        id: &Id,
        type_args: &[Typ],
        values: &[ValueRef],
    ) -> BuiltinResult {
        let entry = self.functions.get(&id.node).copied().ok_or_else(|| {
            BuiltinError::new(
                id.span.clone(),
                format!("implementation for builtin {} is missing", id.node),
            )
        })?;
        match entry {
            Entry::Pure(implementation) => implementation(add, &id.span, type_args, values),
            Entry::FreshTypeId => {
                fresh::fresh_type_id(&mut self.counter, add, &id.span, type_args, values)
            }
        }
    }
}
