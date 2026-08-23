//! Adapter between the owned wire AST and shared runtime values

use crate::{
    lang::il::ast as il,
    runtime::value::{self as runtime, ValueRef},
};

fn to_runtime_inner(value_il: &il::Value) -> ValueRef {
    let kind = match &value_il.kind {
        il::ValueKind::BoolV(value) => runtime::ValueKind::BoolV(*value),
        il::ValueKind::NumV(value) => runtime::ValueKind::NumV(value.clone()),
        il::ValueKind::TextV(value) => runtime::ValueKind::TextV(value.clone()),
        il::ValueKind::StructV(fields) => runtime::ValueKind::StructV(
            fields
                .iter()
                .map(|(atom, value)| (atom.clone(), to_runtime_inner(value)))
                .collect(),
        ),
        il::ValueKind::CaseV(value_case) => {
            runtime::ValueKind::CaseV(value_case.map(to_runtime_inner))
        }
        il::ValueKind::TupleV(values) => {
            runtime::ValueKind::TupleV(values.iter().map(to_runtime_inner).collect())
        }
        il::ValueKind::OptV(value) => {
            runtime::ValueKind::OptV(value.as_deref().map(to_runtime_inner))
        }
        il::ValueKind::ListV(values) => {
            runtime::ValueKind::ListV(values.iter().map(to_runtime_inner).collect())
        }
        il::ValueKind::FuncV(id) => runtime::ValueKind::FuncV(id.clone()),
        il::ValueKind::ExternV(value) => runtime::ValueKind::ExternV(value.clone()),
    };
    runtime::make::new(kind, value_il.ty.clone(), value_il.span.clone())
}

pub fn to_runtime(value_il: &il::Value) -> ValueRef {
    super::ocaml::on_codec_stack(|| to_runtime_inner(value_il))
}

fn to_canonical_inner(value: &runtime::Value) -> il::Value {
    let kind = match &value.kind {
        runtime::ValueKind::BoolV(value) => il::ValueKind::BoolV(*value),
        runtime::ValueKind::NumV(value) => il::ValueKind::NumV(value.clone()),
        runtime::ValueKind::TextV(value) => il::ValueKind::TextV(value.clone()),
        runtime::ValueKind::StructV(fields) => il::ValueKind::StructV(
            fields
                .iter()
                .map(|(atom, value)| (atom.clone(), to_canonical_inner(value)))
                .collect(),
        ),
        runtime::ValueKind::CaseV(value_case) => {
            il::ValueKind::CaseV(Box::new(value_case.map(|value| to_canonical_inner(value))))
        }
        runtime::ValueKind::TupleV(values) => il::ValueKind::TupleV(
            values
                .iter()
                .map(|value| to_canonical_inner(value))
                .collect(),
        ),
        runtime::ValueKind::OptV(value) => il::ValueKind::OptV(
            value
                .as_deref()
                .map(|value| Box::new(to_canonical_inner(value))),
        ),
        runtime::ValueKind::ListV(values) => il::ValueKind::ListV(
            values
                .iter()
                .map(|value| to_canonical_inner(value))
                .collect(),
        ),
        runtime::ValueKind::FuncV(id) => il::ValueKind::FuncV(id.clone()),
        runtime::ValueKind::ExternV(value) => il::ValueKind::ExternV(value.clone()),
    };
    il::Value::new(kind, value.ty.clone(), value.span.region().clone())
}

pub fn to_canonical(value: &runtime::Value) -> il::Value {
    super::ocaml::on_codec_stack(|| to_canonical_inner(value))
}
