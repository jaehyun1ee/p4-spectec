//! Detection of calls contained in language syntax
//!
//! `HasCall` recursively reports whether syntax contains an expression call.

use crate::lang::common::source::NotePhrase;

/// Reports whether syntax contains an expression call.
pub trait HasCall {
    fn has_call(&self) -> bool;
}

impl<T: HasCall, N, S> HasCall for NotePhrase<T, N, S> {
    fn has_call(&self) -> bool {
        self.node.has_call()
    }
}
