//! Registers parser-visible names from completed P4 parse-tree values.
//!
//! P4 token classification depends on declarations reduced earlier in the
//! grammar. These helpers extract names and referenced types from completed
//! values, then update the active parser scope. List-shaped declarations are
//! traversed left to right so every name is available to following tokens.

use crate::lang::data::value::{Value, get};

use super::{
    context::{Context, Namespace, TypeId},
    extract,
};

// == Individual names

pub(super) fn typ(context: &Context, value: &Value, has_params: bool) {
    let id = extract::id_name(value).expect("P4 declaration name");
    context
        .declare_typ(id, has_params)
        .expect("P4 parser scope");
}

pub(super) fn var(context: &Context, value: &Value, has_params: bool, type_ref: Option<&Value>) {
    let id = extract::id_name(value).expect("P4 declaration name");
    let type_id = match type_ref {
        Some(type_ref) => extract::type_id_type_ref(type_ref).expect("P4 type reference"),
        None => TypeId::Empty,
    };
    context
        .declare_var(id, has_params, type_id)
        .expect("P4 parser scope");
}

// == Name lists

pub(super) fn vars(context: &Context, value: &Value) {
    get::matches! {
        value,
        "nameList ',' name" => |values| {
            vars(context, values[0]);
            var(context, values[1], false, None);
        },
        _ => var(context, value, false, None),
    }
}

pub(super) fn typs(context: &Context, value: &Value) {
    get::matches! {
        value,
        "typeParameterList ',' typeParameter" => |values| {
            typs(context, values[0]);
            typ(context, values[1], false);
        },
        _ => typ(context, value, false),
    }
}

// == Type namespaces

pub(super) fn type_namespace(context: &Context, value: &Value, namespace: Namespace) {
    let id = extract::id_name(value).expect("P4 type name");
    context.namespace_set_typ(&id, namespace);
}
