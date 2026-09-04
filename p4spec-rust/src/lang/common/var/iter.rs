//! Iteration operators

use std::{cmp::Ordering, fmt};

use crate::lang::{
    common::ds::set::IdSet,
    traits::{
        cmp::SyntaxCmp,
        eq::SyntaxEq,
        free::Free,
        print::{Print, Printer},
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Iter {
    /// `?`
    Opt,
    /// `*`
    List,
}

impl Print for Iter {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        printer.write(match self {
            Self::Opt => "?",
            Self::List => "*",
        })
    }
}

impl SyntaxEq for Iter {
    fn syntax_eq(&self, other: &Self) -> bool {
        self == other
    }
}

impl SyntaxCmp for Iter {
    fn syntax_cmp(&self, other: &Self) -> Ordering {
        self.cmp(other)
    }
}

impl Free for Iter {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}
