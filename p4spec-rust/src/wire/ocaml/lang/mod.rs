//! OCaml AST codecs

mod el;
pub mod il;
pub mod sl;
mod xl;

const CODEC_STACK_SIZE: usize = 32 * 1024 * 1024;

fn on_codec_stack<T>(codec: impl FnOnce() -> T) -> T {
    stacker::grow(CODEC_STACK_SIZE, codec)
}
