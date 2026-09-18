//! Booleans

use std::fmt;

use crate::lang::traits::print::{Print, Printer};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Typ {
    Bool,
}

// Operations

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UnOp {
    Not,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BinOp {
    And,
    Or,
    Impl,
    Equiv,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CmpOp {
    Eq,
    Ne,
}

// Stringifiers

impl Print for Typ {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        printer.write("bool")
    }
}

impl Print for UnOp {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        printer.write(match self {
            Self::Not => "~",
        })
    }
}

impl Print for BinOp {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        printer.write(match self {
            Self::And => "/\\",
            Self::Or => "\\/",
            Self::Impl => "=>",
            Self::Equiv => "<=>",
        })
    }
}

impl Print for CmpOp {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        printer.write(match self {
            Self::Eq => "=",
            Self::Ne => "=/=",
        })
    }
}
