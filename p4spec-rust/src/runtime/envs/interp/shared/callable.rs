//! Prepared callable syntax and its local frame layout

use std::{fmt, rc::Rc};

use super::frame::FrameLayout;
use crate::{
    interp::shared::prepare::Prepare,
    lang::traits::{
        eq::SyntaxEq,
        print::{Print, Printer},
    },
};

/// Callable syntax paired with its interpreter-owned local layout
#[derive(Clone, Debug, PartialEq)]
pub struct Callable<T> {
    pub def: T,
    pub layout: Rc<FrameLayout>,
}

impl<T> Callable<T> {
    pub fn prepare<S: Prepare<Output = T>>(source: S) -> Self {
        let mut layout = FrameLayout::default();
        let def = source.prepare(&mut layout);
        Self { def, layout: Rc::new(layout) }
    }
}

impl<T: Print> Print for Callable<T> {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        self.def.print(printer)
    }
}

impl<T: SyntaxEq> SyntaxEq for Callable<T> {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.def.syntax_eq(&other.def)
    }
}
