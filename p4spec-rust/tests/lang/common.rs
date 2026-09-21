use p4spec_rust::{
    lang::common::source::{Position, Span},
    lang::common::{
        Id,
        ds::{
            map::{ArityMismatch, IdMap},
            set::IdSet,
        },
    },
    lang::{il, traits::free::FreeIds},
};

fn id(name: &str, file: &str) -> Id {
    p4spec_rust::phrase! {
        node: name.to_owned(),
        span: Span::new(Position::new(file, 0, 0), Position::new(file, 0, 0)),
    }
}

#[path = "common/ds.rs"]
mod ds;
#[path = "common/notation.rs"]
mod notation;

#[path = "common/source.rs"]
mod source;

#[path = "common/ids.rs"]
mod ids;
#[path = "common/prim.rs"]
mod prim;
