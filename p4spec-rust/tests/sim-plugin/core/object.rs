#[path = "object/packet_in.rs"]
mod packet_in;
#[path = "object/packet_out.rs"]
mod packet_out;

use super::{TestError, packet_runner};
use num_bigint::BigInt;
use p4spec_rust::{
    lang::{
        common::source::Span,
        data::{
            typ::{self, TypKind},
            value::{Value, ValueArena, ValueError, get, make},
        },
        xl::num,
    },
    runner::ExternError,
    sim_plugin::{
        core::object::{self, PacketIn, PacketOut},
        spec_impl::pack,
    },
};

fn bits(arena: &ValueArena, value: &Value) -> Vec<bool> {
    get::list(arena, value)
        .unwrap()
        .iter()
        .map(|value| get::bool(arena, value).unwrap())
        .collect()
}

fn reject(arena: &ValueArena, value: &Value) -> String {
    let value_err = get::case(arena, value).unwrap().args()[0];
    let value_name = get::case(arena, value_err).unwrap().args()[0];
    get::text(arena, value_name).unwrap().to_owned()
}
