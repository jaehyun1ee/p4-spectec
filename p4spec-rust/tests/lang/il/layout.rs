use std::mem::size_of;

use p4spec_rust::lang::il::ast::{Exp, ExpKind, Path, PathKind, Typ};

#[test]
fn test_hot_ast_values_stay_compact() {
    let typ = size_of::<Typ>();
    let exp_kind = size_of::<ExpKind>();
    let exp = size_of::<Exp>();
    let path_kind = size_of::<PathKind>();
    let path = size_of::<Path>();
    assert!(
        typ <= 176 && exp_kind <= 136 && exp <= 208 && path_kind <= 120 && path <= 192,
        "Typ: {typ}, ExpKind: {exp_kind}, Exp: {exp}, PathKind: {path_kind}, Path: {path}"
    );
}
