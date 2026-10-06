//! Mixfix forms: a mixop and its arguments in notation order
//!
//! `Mixfix<M, T>` pairs a mixop `M` with one argument of type `T`
//! per argument position: `C |- e : t` is the mixop `% |- % : %`
//! with the arguments `[C, e, t]`.
//! Specification syntax shares a tree, `Rc<tree::Mixop>`;
//! case values hold a `flat::Mixop` handle.
//! Fields are private so every mixfix keeps one argument per position:
//! a tree mixfix is built from parts (`arg`, `atom`, `brack`, `infix`,
//! `seq`), by filling a mixop (`fill_with`), or checked (`new`),
//! and `view` shows it as a tree with arguments in place.
//! Equality compares mixops, then arguments; ordering walks the mixop
//! and compares arguments where their positions occur.

use std::{cmp::Ordering, fmt, rc::Rc};

use crate::lang::{
    common::{ds::set::IdSet, source::Span},
    traits::{at::At, cmp::SyntaxCmp, eq::SyntaxEq, free::FreeIds, print::Printer},
};

use super::{AtomPhrase, Piece, arena::MixopArena, error::ArityMismatch, flat, print, tree};

// = Mixfix forms

/// A mixop with one argument per position, in notation order.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Mixfix<M, T> {
    /// The form, with argument positions.
    mixop: M,
    /// Arguments in notation order, one per position.
    args: Vec<T>,
}

impl<M, T> Mixfix<M, T> {
    // - Access

    /// The form, with argument positions.
    pub fn mixop(&self) -> &M {
        &self.mixop
    }

    /// The arguments in notation order.
    pub fn args(&self) -> &[T] {
        &self.args
    }

    /// The arguments in notation order, for rewriting in place.
    pub fn args_mut(&mut self) -> &mut [T] {
        &mut self.args
    }

    /// The number of argument positions.
    pub fn arity(&self) -> usize {
        self.args.len()
    }

    /// The arguments, dropping the mixop.
    pub fn into_args(self) -> Vec<T> {
        self.args
    }

    /// The mixop and the arguments.
    pub fn into_parts(self) -> (M, Vec<T>) {
        (self.mixop, self.args)
    }

    // - Mapping

    /// Maps each argument in order, keeping the mixop.
    pub fn map<U>(&self, map_arg: impl FnMut(&T) -> U) -> Mixfix<M, U>
    where
        M: Clone,
    {
        Mixfix { mixop: self.mixop.clone(), args: self.args.iter().map(map_arg).collect() }
    }

    /// Maps each argument in order, stopping at the first error.
    pub fn try_map<U, E>(&self, map_arg: impl FnMut(&T) -> Result<U, E>) -> Result<Mixfix<M, U>, E>
    where
        M: Clone,
    {
        let args = self.args.iter().map(map_arg).collect::<Result<_, _>>()?;
        Ok(Mixfix { mixop: self.mixop.clone(), args })
    }
}

// = Tree mixops

impl<T> Mixfix<Rc<tree::Mixop>, T> {
    // - Construction

    /// Pairs a mixop with its arguments.
    ///
    /// Fails unless there is exactly one argument per position.
    pub fn new(mixop: Rc<tree::Mixop>, args: Vec<T>) -> Result<Self, ArityMismatch> {
        let arity = mixop.arity();
        match args.len().cmp(&arity) {
            Ordering::Less => Err(ArityMismatch::ArgumentCountTooFew),
            Ordering::Greater => Err(ArityMismatch::ArgumentCountTooMany),
            Ordering::Equal => Ok(Self { mixop, args }),
        }
    }

    /// Fills each position of a mixop, calling `fill` once per position.
    pub fn fill_with(mixop: Rc<tree::Mixop>, fill: impl FnMut(usize) -> T) -> Self {
        let arity = mixop.arity();
        Self { args: (0..arity).map(fill).collect(), mixop }
    }

    // - Viewing

    /// Borrows the mixfix, for viewing its parts.
    pub fn as_ref(&self) -> MixfixRef<'_, T> {
        MixfixRef { mixop: self.mixop.as_ref(), args: &self.args }
    }

    // - Comparison

    /// Whether two mixfixes have the same structure and atom names.
    ///
    /// Atom spans and arguments are not compared.
    pub fn eq_mixop<U>(&self, mixfix_other: &Mixfix<Rc<tree::Mixop>, U>) -> bool {
        self.mixop.as_ref() == mixfix_other.mixop.as_ref()
    }

    /// Orders two mixfixes as the walk of their mixops meets atoms and arguments.
    ///
    /// Atoms compare by name; `compare_arg` orders the arguments
    /// at each position both mixops reach.
    pub fn cmp_by<U>(
        &self,
        mixfix_other: &Mixfix<Rc<tree::Mixop>, U>,
        mut compare_arg: impl FnMut(&T, &U) -> Ordering,
    ) -> Ordering {
        self.mixop.cmp_by(mixfix_other.mixop.as_ref(), |pos| {
            compare_arg(&self.args[pos], &mixfix_other.args[pos])
        })
    }
}

// - Printing

impl<T> Mixfix<Rc<tree::Mixop>, T> {
    /// Writes atoms and arguments, separating non-empty pieces with spaces.
    pub fn print_with(
        &self,
        printer: &mut Printer<'_>,
        mut print_arg: impl FnMut(&T, &mut Printer<'_>) -> fmt::Result,
    ) -> fmt::Result {
        print::tree_with(&self.mixop, printer, |pos, printer| print_arg(&self.args[pos], printer))
    }
}

// - Parts of shared trees

impl<T> Mixfix<Rc<tree::Mixop>, T> {
    /// A lone argument.
    pub fn arg(arg: T) -> Self {
        Self { mixop: Rc::new(tree::Mixop::Arg), args: vec![arg] }
    }

    /// A lone atom.
    pub fn atom(atom: AtomPhrase) -> Self {
        Self { mixop: Rc::new(tree::Mixop::Atom(atom)), args: Vec::new() }
    }

    /// A form between two atoms.
    pub fn brack(atom_l: AtomPhrase, mixfix_inner: Self, atom_r: AtomPhrase) -> Self {
        let mixop_inner = Rc::unwrap_or_clone(mixfix_inner.mixop);
        let mixop = tree::Mixop::Brack(atom_l, Box::new(mixop_inner), atom_r);
        Self { mixop: Rc::new(mixop), args: mixfix_inner.args }
    }

    /// Two forms around an atom.
    pub fn infix(mixfix_l: Self, atom: AtomPhrase, mixfix_r: Self) -> Self {
        let mixop_l = Rc::unwrap_or_clone(mixfix_l.mixop);
        let mixop_r = Rc::unwrap_or_clone(mixfix_r.mixop);
        let mixop = tree::Mixop::Infix(Box::new(mixop_l), atom, Box::new(mixop_r));
        let mut args = mixfix_l.args;
        args.extend(mixfix_r.args);
        Self { mixop: Rc::new(mixop), args }
    }

    /// Forms in sequence.
    pub fn seq(mixfixes: Vec<Self>) -> Self {
        let mut mixops = Vec::with_capacity(mixfixes.len());
        let mut args = Vec::new();
        for mixfix in mixfixes {
            mixops.push(Rc::unwrap_or_clone(mixfix.mixop));
            args.extend(mixfix.args);
        }
        Self { mixop: Rc::new(tree::Mixop::Seq(mixops)), args }
    }
}

// - Syntax operations

impl<T: SyntaxEq> SyntaxEq for Mixfix<Rc<tree::Mixop>, T> {
    fn syntax_eq(&self, mixfix_other: &Self) -> bool {
        self.eq_mixop(mixfix_other)
            && self
                .args
                .iter()
                .zip(&mixfix_other.args)
                .all(|(arg_l, arg_r)| arg_l.syntax_eq(arg_r))
    }
}

impl<T: SyntaxCmp> SyntaxCmp for Mixfix<Rc<tree::Mixop>, T> {
    fn syntax_cmp(&self, mixfix_other: &Self) -> Ordering {
        self.cmp_by(mixfix_other, SyntaxCmp::syntax_cmp)
    }
}

impl<M, T: FreeIds> FreeIds for Mixfix<M, T> {
    fn free_ids_into(&self, free: &mut IdSet) {
        self.args.as_slice().free_ids_into(free);
    }
}

// - Source locations

impl<T: At> At for Mixfix<Rc<tree::Mixop>, T> {
    fn at(&self) -> Span {
        // Cover atoms and arguments, so empty sequences add no default span
        let mut spans = Vec::new();
        self.mixop.visit(|piece| match piece {
            Piece::Atom(atom) => spans.push(atom.at()),
            Piece::Arg(pos) => spans.push(self.args[pos].at()),
        });
        spans.at()
    }
}

// = Borrowed views

/// A borrowed mixfix: a tree mixop and the arguments of its positions.
#[derive(Debug)]
pub struct MixfixRef<'a, T> {
    mixop: &'a tree::Mixop,
    args: &'a [T],
}

impl<T> Clone for MixfixRef<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for MixfixRef<'_, T> {}

/// One level of a mixfix tree, with arguments in place.
#[derive(Debug)]
pub enum View<'a, T> {
    /// An argument.
    Arg(&'a T),
    /// A literal atom.
    Atom(&'a AtomPhrase),
    /// A form between two atoms.
    Brack(&'a AtomPhrase, MixfixRef<'a, T>, &'a AtomPhrase),
    /// Two forms around an atom.
    Infix(MixfixRef<'a, T>, &'a AtomPhrase, MixfixRef<'a, T>),
    /// Forms in sequence.
    Seq(Vec<MixfixRef<'a, T>>),
}

impl<'a, T> MixfixRef<'a, T> {
    /// The top level of the tree, splitting arguments among the children.
    pub fn view(&self) -> View<'a, T> {
        // Each child takes as many arguments as it has positions
        let mut args = self.args;
        let mut take = |mixop: &'a tree::Mixop| {
            let (args_child, args_rest) = args.split_at(mixop.arity());
            args = args_rest;
            MixfixRef { mixop, args: args_child }
        };
        match self.mixop {
            tree::Mixop::Arg => View::Arg(&self.args[0]),
            tree::Mixop::Atom(atom) => View::Atom(atom),
            tree::Mixop::Brack(atom_l, mixop_inner, atom_r) => {
                View::Brack(atom_l, take(mixop_inner), atom_r)
            }
            tree::Mixop::Infix(mixop_l, atom, mixop_r) => {
                let mixfix_l = take(mixop_l);
                View::Infix(mixfix_l, atom, take(mixop_r))
            }
            tree::Mixop::Seq(mixops) => View::Seq(mixops.iter().map(take).collect()),
        }
    }
}

// = Interned mixops

impl<T> Mixfix<flat::Mixop, T> {
    // - Construction

    /// Pairs a mixop with its arguments.
    ///
    /// Fails unless there is exactly one argument per position;
    /// `mixop` must belong to `arena_mixop`.
    pub fn new_in(
        arena_mixop: &MixopArena,
        mixop: flat::Mixop,
        args: Vec<T>,
    ) -> Result<Self, ArityMismatch> {
        match args.len().cmp(&arena_mixop.arity(mixop)) {
            Ordering::Less => Err(ArityMismatch::ArgumentCountTooFew),
            Ordering::Greater => Err(ArityMismatch::ArgumentCountTooMany),
            Ordering::Equal => Ok(Self { mixop, args }),
        }
    }

    // - Comparison

    /// Orders two cases as their expanded trees would order.
    ///
    /// Each case reads its mixop in its own `MixopArena`;
    /// `compare_arg` orders the arguments at each position both reach.
    pub fn cmp_in_by<U>(
        &self,
        arena_mixop: &MixopArena,
        mixfix_other: &Mixfix<flat::Mixop, U>,
        arena_mixop_other: &MixopArena,
        mut compare_arg: impl FnMut(&T, &U) -> Ordering,
    ) -> Ordering {
        flat::cmp_by(arena_mixop, self.mixop, arena_mixop_other, mixfix_other.mixop, |pos| {
            compare_arg(&self.args[pos], &mixfix_other.args[pos])
        })
    }

    // - Printing

    /// Writes atoms and arguments as the expanded tree would print.
    pub fn print_in_with(
        &self,
        arena_mixop: &MixopArena,
        printer: &mut Printer<'_>,
        mut print_arg: impl FnMut(&T, &mut Printer<'_>) -> fmt::Result,
    ) -> fmt::Result {
        print::flat_with(arena_mixop, self.mixop, printer, |pos, printer| {
            print_arg(&self.args[pos], printer)
        })
    }
}
