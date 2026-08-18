mod api;
pub mod builtin;
mod p4;

pub use api::{BuiltinInterface, Interface, InterfaceError, NullInterface};
pub use p4::P4Interface;
