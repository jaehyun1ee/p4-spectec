use crate::lang::{
    il::ast::{self, Iter},
    traits::eq::SyntaxEq,
};

/// A base type paired with the iteration dimensions around it
#[derive(Clone, Debug, PartialEq)]
pub struct Dim {
    typ: ast::Typ,
    iters: Vec<Iter>,
}

impl Dim {
    pub fn new(typ: ast::Typ, iters: Vec<Iter>) -> Self {
        Self { typ, iters }
    }

    /// Borrows the base type below the iteration dimensions
    pub fn typ(&self) -> &ast::Typ {
        &self.typ
    }

    /// Borrows iteration dimensions from innermost to outermost
    pub fn iters(&self) -> &[Iter] {
        &self.iters
    }

    /// Tests type syntax and every dimension for equality
    pub fn equiv(&self, other: &Self) -> bool {
        self.typ.syntax_eq(&other.typ) && self.iters == other.iters
    }

    /// Tests whether this value's dimensions are a prefix of `other`
    pub fn sub(&self, other: &Self) -> bool {
        self.typ.syntax_eq(&other.typ)
            && self.iters.len() <= other.iters.len()
            && self.iters == other.iters[..self.iters.len()]
    }

    /// Appends one outer iteration dimension
    pub fn add_iter(mut self, iter: Iter) -> Self {
        self.iters.push(iter);
        self
    }
}
