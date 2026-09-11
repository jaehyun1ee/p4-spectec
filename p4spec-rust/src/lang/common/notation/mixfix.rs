use std::{
    cmp::Ordering,
    fmt,
    hash::{Hash, Hasher},
};

use crate::lang::{
    common::ds::set::IdSet,
    traits::{
        cmp::SyntaxCmp,
        eq::SyntaxEq,
        free::Free,
        print::{Print, Printer},
    },
};

use super::{
    super::source::{Phrase, Span},
    atom::Atom,
};

/// An atom paired with its source span
pub type AtomPhrase<S = Span> = Phrase<Atom, S>;

/// A mixfix expression: literal atoms interleaved with argument holes of type
/// `T`. For example `_ + _` is infix with two holes and `[ _ ]` brackets one.
#[derive(Clone, Debug)]
pub enum Mixfix<T, S = Span> {
    /// Argument position
    Arg(T),
    /// Literal atom
    Atom(AtomPhrase<S>),
    /// Bracketed expression
    Brack(AtomPhrase<S>, Box<Self>, AtomPhrase<S>),
    /// Infix expression
    Infix(Box<Self>, AtomPhrase<S>, Box<Self>),
    /// Sequence of expressions
    Seq(Vec<Self>),
}

// == Equality and comparison

impl<T, S> Mixfix<T, S> {
    // - Tagging for comparison

    fn tag(&self) -> u8 {
        match self {
            Self::Arg(_) => 0,
            Self::Atom(_) => 1,
            Self::Brack(_, _, _) => 2,
            Self::Infix(_, _, _) => 3,
            Self::Seq(_) => 4,
        }
    }

    // - Comparison

    /// Compares structure and atoms lexicographically, using `compare_arg` for arguments
    pub fn cmp_by<U, R>(
        &self,
        mixfix_other: &Mixfix<U, R>,
        mut compare_arg: impl FnMut(&T, &U) -> Ordering,
    ) -> Ordering {
        self.cmp_by_inner(mixfix_other, &mut compare_arg)
    }

    fn cmp_by_inner<U, R>(
        &self,
        mixfix_other: &Mixfix<U, R>,
        compare_arg: &mut impl FnMut(&T, &U) -> Ordering,
    ) -> Ordering {
        match (self, mixfix_other) {
            (Self::Arg(arg_l), Mixfix::Arg(arg_r)) => compare_arg(arg_l, arg_r),
            (Self::Atom(atom_l), Mixfix::Atom(atom_r)) => atom_l.node.cmp(&atom_r.node),
            (
                Self::Brack(atom_l_l, mixfix_l, atom_l_r),
                Mixfix::Brack(atom_r_l, mixfix_r, atom_r_r),
            ) => atom_l_l
                .node
                .cmp(&atom_r_l.node)
                .then_with(|| mixfix_l.cmp_by_inner(mixfix_r, compare_arg))
                .then_with(|| atom_l_r.node.cmp(&atom_r_r.node)),
            (
                Self::Infix(mixfix_l_l, atom_l, mixfix_l_r),
                Mixfix::Infix(mixfix_r_l, atom_r, mixfix_r_r),
            ) => mixfix_l_l
                .cmp_by_inner(mixfix_r_l, compare_arg)
                .then_with(|| atom_l.node.cmp(&atom_r.node))
                .then_with(|| mixfix_l_r.cmp_by_inner(mixfix_r_r, compare_arg)),
            (Self::Seq(mixfixes_l), Mixfix::Seq(mixfixes_r)) => {
                for (mixfix_l, mixfix_r) in mixfixes_l.iter().zip(mixfixes_r) {
                    let ord = mixfix_l.cmp_by_inner(mixfix_r, compare_arg);
                    if ord != Ordering::Equal {
                        return ord;
                    }
                }
                mixfixes_l.len().cmp(&mixfixes_r.len())
            }
            _ => self.tag().cmp(&mixfix_other.tag()),
        }
    }

    /// Compares structure and atoms, using `eq_arg` for arguments
    pub fn eq_by<U, R>(
        &self,
        mixfix_other: &Mixfix<U, R>,
        mut eq_arg: impl FnMut(&T, &U) -> bool,
    ) -> bool {
        self.eq_by_inner(mixfix_other, &mut eq_arg)
    }

    fn eq_by_inner<U, R>(
        &self,
        mixfix_other: &Mixfix<U, R>,
        eq_arg: &mut impl FnMut(&T, &U) -> bool,
    ) -> bool {
        match (self, mixfix_other) {
            (Self::Arg(arg_l), Mixfix::Arg(arg_r)) => eq_arg(arg_l, arg_r),
            (Self::Atom(atom_l), Mixfix::Atom(atom_r)) => atom_l.node == atom_r.node,
            (
                Self::Brack(atom_l_l, mixfix_l, atom_l_r),
                Mixfix::Brack(atom_r_l, mixfix_r, atom_r_r),
            ) => {
                atom_l_l.node == atom_r_l.node
                    && mixfix_l.eq_by_inner(mixfix_r, eq_arg)
                    && atom_l_r.node == atom_r_r.node
            }
            (
                Self::Infix(mixfix_l_l, atom_l, mixfix_l_r),
                Mixfix::Infix(mixfix_r_l, atom_r, mixfix_r_r),
            ) => {
                mixfix_l_l.eq_by_inner(mixfix_r_l, eq_arg)
                    && atom_l.node == atom_r.node
                    && mixfix_l_r.eq_by_inner(mixfix_r_r, eq_arg)
            }
            (Self::Seq(mixfixes_l), Mixfix::Seq(mixfixes_r)) => {
                mixfixes_l.len() == mixfixes_r.len()
                    && mixfixes_l
                        .iter()
                        .zip(mixfixes_r)
                        .all(|(mixfix_l, mixfix_r)| mixfix_l.eq_by_inner(mixfix_r, eq_arg))
            }
            _ => false,
        }
    }

    /// Tests whether two mixfixes have the same atoms and argument positions
    pub fn eq_shape<U, R>(&self, mixfix_other: &Mixfix<U, R>) -> bool {
        self.eq_by(mixfix_other, |_, _| true)
    }
}

impl<T: PartialEq, S> PartialEq for Mixfix<T, S> {
    fn eq(&self, mixfix_other: &Self) -> bool {
        self.eq_by(mixfix_other, PartialEq::eq)
    }
}

impl<T: Eq, S> Eq for Mixfix<T, S> {}

impl<T: SyntaxEq, S> SyntaxEq for Mixfix<T, S> {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.eq_by(other, SyntaxEq::syntax_eq)
    }
}

impl<T: SyntaxCmp, S> SyntaxCmp for Mixfix<T, S> {
    fn syntax_cmp(&self, other: &Self) -> Ordering {
        self.cmp_by(other, SyntaxCmp::syntax_cmp)
    }
}

// == Ordering

impl<T: Ord, S> Ord for Mixfix<T, S> {
    fn cmp(&self, mixfix_other: &Self) -> Ordering {
        self.cmp_by(mixfix_other, Ord::cmp)
    }
}

impl<T: Ord, S> PartialOrd for Mixfix<T, S> {
    fn partial_cmp(&self, mixfix_other: &Self) -> Option<Ordering> {
        Some(self.cmp(mixfix_other))
    }
}

// == Hashing

impl<T: Hash, S> Hash for Mixfix<T, S> {
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        self.tag().hash(hasher);
        match self {
            Self::Arg(arg) => arg.hash(hasher),
            Self::Atom(atom) => atom.node.hash(hasher),
            Self::Brack(atom_l, mixfix, atom_r) => {
                atom_l.node.hash(hasher);
                mixfix.hash(hasher);
                atom_r.node.hash(hasher);
            }
            Self::Infix(mixfix_l, atom, mixfix_r) => {
                mixfix_l.hash(hasher);
                atom.node.hash(hasher);
                mixfix_r.hash(hasher);
            }
            Self::Seq(mixfixes) => mixfixes.hash(hasher),
        }
    }
}

// == Free identifiers

impl<T: Free, S> Free for Mixfix<T, S> {
    fn free_into(&self, free: &mut IdSet) {
        match self {
            Self::Arg(arg) => arg.free_into(free),
            Self::Atom(_) => {}
            Self::Brack(_, mixfix, _) => mixfix.free_into(free),
            Self::Infix(mixfix_l, _, mixfix_r) => {
                mixfix_l.free_into(free);
                mixfix_r.free_into(free);
            }
            Self::Seq(mixfixes) => mixfixes.as_slice().free_into(free),
        }
    }
}

// == Fold, map, and iter

impl<T, S> Mixfix<T, S> {
    /// Folds arguments from left to right
    pub fn fold<A>(&self, acc: A, mut fold_arg: impl FnMut(A, &T) -> A) -> A {
        self.fold_inner(acc, &mut fold_arg)
    }

    fn fold_inner<A>(&self, acc: A, fold_arg: &mut impl FnMut(A, &T) -> A) -> A {
        match self {
            Self::Arg(arg) => fold_arg(acc, arg),
            Self::Atom(_) => acc,
            Self::Brack(_, mixfix, _) => mixfix.fold_inner(acc, fold_arg),
            Self::Infix(mixfix_l, _, mixfix_r) => {
                let acc = mixfix_l.fold_inner(acc, fold_arg);
                mixfix_r.fold_inner(acc, fold_arg)
            }
            Self::Seq(mixfixes) => mixfixes
                .iter()
                .fold(acc, |acc, mixfix| mixfix.fold_inner(acc, fold_arg)),
        }
    }

    /// Maps arguments while preserving mixfix structure and atoms
    pub fn map<U>(&self, mut map_arg: impl FnMut(&T) -> U) -> Mixfix<U, S>
    where
        S: Clone,
    {
        self.map_inner(&mut map_arg)
    }

    fn map_inner<U>(&self, map_arg: &mut impl FnMut(&T) -> U) -> Mixfix<U, S>
    where
        S: Clone,
    {
        match self {
            Self::Arg(arg) => Mixfix::Arg(map_arg(arg)),
            Self::Atom(atom) => Mixfix::Atom(atom.clone()),
            Self::Brack(atom_l, mixfix, atom_r) => Mixfix::Brack(
                atom_l.clone(),
                Box::new(mixfix.map_inner(map_arg)),
                atom_r.clone(),
            ),
            Self::Infix(mixfix_l, atom, mixfix_r) => Mixfix::Infix(
                Box::new(mixfix_l.map_inner(map_arg)),
                atom.clone(),
                Box::new(mixfix_r.map_inner(map_arg)),
            ),
            Self::Seq(mixfixes) => Mixfix::Seq(
                mixfixes
                    .iter()
                    .map(|mixfix| mixfix.map_inner(map_arg))
                    .collect(),
            ),
        }
    }

    /// Converts atom spans while preserving structure, labels, and argument values
    pub fn map_span<R>(self, mut map_span: impl FnMut(S) -> R) -> Mixfix<T, R> {
        self.map_span_inner(&mut map_span)
    }

    fn map_span_inner<R>(self, map_span: &mut impl FnMut(S) -> R) -> Mixfix<T, R> {
        match self {
            Self::Arg(arg) => Mixfix::Arg(arg),
            Self::Atom(atom) => Mixfix::Atom(atom.map_span(map_span)),
            Self::Brack(atom_l, mixfix, atom_r) => Mixfix::Brack(
                atom_l.map_span(&mut *map_span),
                Box::new(mixfix.map_span_inner(map_span)),
                atom_r.map_span(map_span),
            ),
            Self::Infix(mixfix_l, atom, mixfix_r) => Mixfix::Infix(
                Box::new(mixfix_l.map_span_inner(map_span)),
                atom.map_span(&mut *map_span),
                Box::new(mixfix_r.map_span_inner(map_span)),
            ),
            Self::Seq(mixfixes) => Mixfix::Seq(
                mixfixes
                    .into_iter()
                    .map(|mixfix| mixfix.map_span_inner(map_span))
                    .collect(),
            ),
        }
    }

    /// Converts atom spans, stopping at the first conversion error
    pub fn try_map_span<R, E>(
        self,
        mut map_span: impl FnMut(S) -> Result<R, E>,
    ) -> Result<Mixfix<T, R>, E> {
        self.try_map_span_inner(&mut map_span)
    }

    fn try_map_span_inner<R, E>(
        self,
        map_span: &mut impl FnMut(S) -> Result<R, E>,
    ) -> Result<Mixfix<T, R>, E> {
        Ok(match self {
            Self::Arg(arg) => Mixfix::Arg(arg),
            Self::Atom(atom) => {
                Mixfix::Atom(crate::phrase!(node: atom.node, span: map_span(atom.span)?))
            }
            Self::Brack(atom_l, mixfix, atom_r) => Mixfix::Brack(
                crate::phrase!(node: atom_l.node, span: map_span(atom_l.span)?),
                Box::new(mixfix.try_map_span_inner(map_span)?),
                crate::phrase!(node: atom_r.node, span: map_span(atom_r.span)?),
            ),
            Self::Infix(mixfix_l, atom, mixfix_r) => Mixfix::Infix(
                Box::new(mixfix_l.try_map_span_inner(map_span)?),
                crate::phrase!(node: atom.node, span: map_span(atom.span)?),
                Box::new(mixfix_r.try_map_span_inner(map_span)?),
            ),
            Self::Seq(mixfixes) => Mixfix::Seq(
                mixfixes
                    .into_iter()
                    .map(|mixfix| mixfix.try_map_span_inner(map_span))
                    .collect::<Result<_, _>>()?,
            ),
        })
    }

    /// Visits arguments from left to right
    pub fn iter(&self, mut visit_arg: impl FnMut(&T)) {
        self.fold((), |(), arg| visit_arg(arg));
    }
}

// == Utilities using fold, map, and iter

impl<T, S> Mixfix<T, S> {
    // - Arity

    /// Returns the number of argument positions
    pub fn arity(&self) -> usize {
        self.fold(0, |arity, _| arity + 1)
    }

    // - Atoms and args

    /// Collects atoms in left-to-right tree order
    pub fn atoms(&self) -> Vec<&AtomPhrase<S>> {
        let mut atoms = Vec::new();
        self.collect_atoms(&mut atoms);
        atoms
    }

    fn collect_atoms<'a>(&'a self, atoms: &mut Vec<&'a AtomPhrase<S>>) {
        match self {
            Self::Arg(_) => {}
            Self::Atom(atom) => atoms.push(atom),
            Self::Brack(atom_l, mixfix, atom_r) => {
                atoms.push(atom_l);
                mixfix.collect_atoms(atoms);
                atoms.push(atom_r);
            }
            Self::Infix(mixfix_l, atom, mixfix_r) => {
                mixfix_l.collect_atoms(atoms);
                atoms.push(atom);
                mixfix_r.collect_atoms(atoms);
            }
            Self::Seq(mixfixes) => {
                for mixfix in mixfixes {
                    mixfix.collect_atoms(atoms);
                }
            }
        }
    }

    /// Collects arguments in left-to-right tree order
    pub fn args(&self) -> Vec<&T> {
        let mut args = Vec::with_capacity(self.arity());
        self.collect_args(&mut args);
        args
    }

    fn collect_args<'a>(&'a self, args: &mut Vec<&'a T>) {
        match self {
            Self::Arg(arg) => args.push(arg),
            Self::Atom(_) => {}
            Self::Brack(_, mixfix, _) => mixfix.collect_args(args),
            Self::Infix(mixfix_l, _, mixfix_r) => {
                mixfix_l.collect_args(args);
                mixfix_r.collect_args(args);
            }
            Self::Seq(mixfixes) => {
                for mixfix in mixfixes {
                    mixfix.collect_args(args);
                }
            }
        }
    }

    /// Collects owned arguments in left-to-right tree order
    pub fn into_args(self) -> Vec<T> {
        let mut args = Vec::with_capacity(self.arity());
        self.collect_into_args(&mut args);
        args
    }

    fn collect_into_args(self, args: &mut Vec<T>) {
        match self {
            Self::Arg(arg) => args.push(arg),
            Self::Atom(_) => {}
            Self::Brack(_, mixfix, _) => mixfix.collect_into_args(args),
            Self::Infix(mixfix_l, _, mixfix_r) => {
                mixfix_l.collect_into_args(args);
                mixfix_r.collect_into_args(args);
            }
            Self::Seq(mixfixes) => {
                for mixfix in mixfixes {
                    mixfix.collect_into_args(args);
                }
            }
        }
    }
}

// == Printing

impl<T, S> Mixfix<T, S> {
    /// Writes atoms and arguments, separating non-empty pieces with spaces
    pub fn print_with(
        &self,
        printer: &mut Printer<'_>,
        mut print_arg: impl FnMut(&T, &mut Printer<'_>) -> fmt::Result,
    ) -> fmt::Result {
        let mut is_first = true;
        self.print_with_inner(printer, &mut print_arg, &mut is_first)
    }

    fn print_with_inner(
        &self,
        printer: &mut Printer<'_>,
        print_arg: &mut impl FnMut(&T, &mut Printer<'_>) -> fmt::Result,
        is_first: &mut bool,
    ) -> fmt::Result {
        let print_sep = |printer: &mut Printer<'_>, is_first: &mut bool| {
            if *is_first {
                *is_first = false;
                Ok(())
            } else {
                printer.write(" ")
            }
        };

        let print_atom = |atom: &AtomPhrase<S>, printer: &mut Printer<'_>, is_first: &mut bool| {
            if matches!(&atom.node, Atom::Keyword(keyword) if keyword.is_empty()) {
                Ok(())
            } else {
                print_sep(printer, is_first)?;
                atom.print(printer)
            }
        };

        match self {
            Self::Arg(arg) => {
                print_sep(printer, is_first)?;
                print_arg(arg, printer)
            }
            Self::Atom(atom) => print_atom(atom, printer, is_first),
            Self::Brack(atom_l, mixfix, atom_r) => {
                print_atom(atom_l, printer, is_first)?;
                mixfix.print_with_inner(printer, print_arg, is_first)?;
                print_atom(atom_r, printer, is_first)
            }
            Self::Infix(mixfix_l, atom, mixfix_r) => {
                mixfix_l.print_with_inner(printer, print_arg, is_first)?;
                print_atom(atom, printer, is_first)?;
                mixfix_r.print_with_inner(printer, print_arg, is_first)
            }
            Self::Seq(mixfixes) => {
                for mixfix in mixfixes {
                    mixfix.print_with_inner(printer, print_arg, is_first)?;
                }
                Ok(())
            }
        }
    }
}
