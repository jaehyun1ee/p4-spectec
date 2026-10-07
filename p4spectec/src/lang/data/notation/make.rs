//! Constructors and argument mapping for mixfix forms
//!
//! `new` and `new_in` check one argument per mixop position.
//! Tree constructors compose forms with their arguments in notation order;
//! `map` and `try_map` replace arguments while preserving the mixop.

use std::{cmp::Ordering, rc::Rc};

use super::{ArityMismatch, AtomPhrase, Mixfix, MixopArena, flat, tree};

// - General

impl<T> Mixfix<Rc<tree::Mixop>, T> {
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
}

impl<T> Mixfix<flat::Mixop, T> {
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
}

// - Parts

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

// - Mapping

impl<M, T> Mixfix<M, T> {
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
