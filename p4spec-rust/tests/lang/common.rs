use p4spec_rust::{
    lang::common::source::{Position, Span},
    lang::common::{
        Id,
        ds::{
            map::{ArityMismatch, IdMap},
            set::IdSet,
        },
    },
    lang::{il, traits::free::Free},
};

fn id(name: &str, file: &str) -> Id {
    p4spec_rust::phrase! {
        node: name.to_owned(),
        span: Span::new(Position::new(file, 0, 0), Position::new(file, 0, 0)),
    }
}

#[path = "common/ds.rs"]
mod ds;
