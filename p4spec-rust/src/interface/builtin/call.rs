//! Dispatch specification builtin calls to their Rust implementations.
//!
//! Construction installs the standard entries in specification order and
//! then applies interface-specific overrides. Invocation resolves one name and
//! calls its implementation; for example, `sum_nat` dispatches to
//! `nats::sum_nat`, while `fresh_typeId` advances state hidden in `fresh`.

use std::{collections::HashMap, rc::Rc};

use crate::{
    lang::data::value::Value,
    lang::il::ast::{Id, Typ},
};

use super::{
    BuiltinError, BuiltinErrorKind, fresh, ints, lists, maps, nats, numerics, sets, texts,
};

// == Extensibility point: extra or override builtins per interface

pub type BuiltinImpl = fn(targs: &[Typ], values: &[Rc<Value>]) -> Result<Rc<Value>, BuiltinError>;

#[derive(Clone, Copy)]
enum BuiltinEntry {
    Pure(BuiltinImpl),
    Impure(BuiltinImpl),
}

// == Builtin registry

pub struct Builtins {
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
            ("sum_nat".to_owned(), BuiltinEntry::Pure(nats::sum_nat)),
            ("max_nat".to_owned(), BuiltinEntry::Pure(nats::max_nat)),
            ("min_nat".to_owned(), BuiltinEntry::Pure(nats::min_nat)),
            // Ints
            ("sum_int".to_owned(), BuiltinEntry::Pure(ints::sum_int)),
            ("max_int".to_owned(), BuiltinEntry::Pure(ints::max_int)),
            ("min_int".to_owned(), BuiltinEntry::Pure(ints::min_int)),
            // Texts
            (
                "text_to_int".to_owned(),
                BuiltinEntry::Pure(texts::text_to_int),
            ),
            (
                "int_to_text".to_owned(),
                BuiltinEntry::Pure(texts::int_to_text),
            ),
            (
                "split_text".to_owned(),
                BuiltinEntry::Pure(texts::split_text),
            ),
            (
                "strip_prefix".to_owned(),
                BuiltinEntry::Pure(texts::strip_prefix),
            ),
            (
                "strip_suffix".to_owned(),
                BuiltinEntry::Pure(texts::strip_suffix),
            ),
            (
                "strip_all_whitespace".to_owned(),
                BuiltinEntry::Pure(texts::strip_all_whitespace),
            ),
            // Lists
            ("rev_".to_owned(), BuiltinEntry::Pure(lists::rev_)),
            ("concat_".to_owned(), BuiltinEntry::Pure(lists::concat_)),
            ("distinct_".to_owned(), BuiltinEntry::Pure(lists::distinct_)),
            (
                "partition_".to_owned(),
                BuiltinEntry::Pure(lists::partition_),
            ),
            ("assoc_".to_owned(), BuiltinEntry::Pure(lists::assoc_)),
            ("sort_".to_owned(), BuiltinEntry::Pure(lists::sort_)),
            (
                "transpose_".to_owned(),
                BuiltinEntry::Pure(lists::transpose_),
            ),
            // Sets
            (
                "intersect_set".to_owned(),
                BuiltinEntry::Pure(sets::intersect_set),
            ),
            ("union_set".to_owned(), BuiltinEntry::Pure(sets::union_set)),
            (
                "unions_set".to_owned(),
                BuiltinEntry::Pure(sets::unions_set),
            ),
            ("diff_set".to_owned(), BuiltinEntry::Pure(sets::diff_set)),
            ("sub_set".to_owned(), BuiltinEntry::Pure(sets::sub_set)),
            ("eq_set".to_owned(), BuiltinEntry::Pure(sets::eq_set)),
            // Maps
            ("find_map".to_owned(), BuiltinEntry::Pure(maps::find_map)),
            ("find_maps".to_owned(), BuiltinEntry::Pure(maps::find_maps)),
            ("add_map".to_owned(), BuiltinEntry::Pure(maps::add_map)),
            ("adds_map".to_owned(), BuiltinEntry::Pure(maps::adds_map)),
            (
                "update_map".to_owned(),
                BuiltinEntry::Pure(maps::update_map),
            ),
            // Fresh type id
            (
                "fresh_typeId".to_owned(),
                BuiltinEntry::Impure(fresh::fresh_type_id),
            ),
            // Numerics
            ("shl".to_owned(), BuiltinEntry::Pure(numerics::shl)),
            ("shr".to_owned(), BuiltinEntry::Pure(numerics::shr)),
            (
                "shr_arith".to_owned(),
                BuiltinEntry::Pure(numerics::shr_arith),
            ),
            ("pow2".to_owned(), BuiltinEntry::Pure(numerics::pow2)),
            (
                "bitstr_to_int".to_owned(),
                BuiltinEntry::Pure(numerics::bitstr_to_int),
            ),
            (
                "int_to_bitstr".to_owned(),
                BuiltinEntry::Pure(numerics::int_to_bitstr),
            ),
            (
                "bits_to_int_unsigned".to_owned(),
                BuiltinEntry::Pure(numerics::bits_to_int_unsigned),
            ),
            (
                "bits_to_int_signed".to_owned(),
                BuiltinEntry::Pure(numerics::bits_to_int_signed),
            ),
            (
                "int_to_bits_unsigned".to_owned(),
                BuiltinEntry::Pure(numerics::int_to_bits_unsigned),
            ),
            (
                "int_to_bits_signed".to_owned(),
                BuiltinEntry::Pure(numerics::int_to_bits_signed),
            ),
            ("bneg".to_owned(), BuiltinEntry::Pure(numerics::bneg)),
            ("band".to_owned(), BuiltinEntry::Pure(numerics::band)),
            ("bxor".to_owned(), BuiltinEntry::Pure(numerics::bxor)),
            ("bor".to_owned(), BuiltinEntry::Pure(numerics::bor)),
            ("bitacc".to_owned(), BuiltinEntry::Pure(numerics::bitacc)),
            (
                "bitacc_replace".to_owned(),
                BuiltinEntry::Pure(numerics::bitacc_replace),
            ),
        ]);
        // Extension entries are merged last, allowing interface-specific overrides.
        for (name, builtin_impl) in entries {
            functions.insert(name.to_owned(), BuiltinEntry::Pure(builtin_impl));
        }
        Self { functions }
    }

    // Initializer

    pub fn init(&mut self) {
        fresh::init();
    }

    // Builtin calls

    pub fn invoke(
        &mut self,
        id: &Id,
        targs: &[Typ],
        values: &[Rc<Value>],
    ) -> Result<(Rc<Value>, bool), BuiltinError> {
        let entry = self
            .functions
            .get(&id.node)
            .copied()
            .ok_or_else(|| BuiltinError {
                kind: BuiltinErrorKind::MissingImplementation(id.node.clone()),
            })?;
        let (value, side_effected) = match entry {
            BuiltinEntry::Pure(builtin_impl) => {
                let value = builtin_impl(targs, values)?;
                (value, false)
            }
            BuiltinEntry::Impure(builtin_impl) => {
                let value = builtin_impl(targs, values)?;
                (value, true)
            }
        };
        Ok((value, side_effected))
    }
}
