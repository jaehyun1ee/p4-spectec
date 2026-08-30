//! Stage-independent execution contracts

mod externs;
mod interface;

pub use externs::{Extern, ExternError, ExternErrorKind, NullExtern, SpecCall};
pub use interface::{
    BuiltinInterface, Interface, InterfaceError, InterfaceErrorKind, NullInterface,
};
