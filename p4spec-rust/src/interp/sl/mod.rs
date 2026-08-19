pub mod assignment;
pub mod context;
pub mod expression;
mod instruction;
mod interpreter;

pub use interpreter::{Interpreter, Options};
