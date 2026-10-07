//! Constructors for tree mixfix forms
//!
//! `new` checks the argument count; `fill` supplies each position.
//! Part constructors compose forms and arguments in notation order.

use std::{cmp::Ordering, rc::Rc};

use super::super::{ArityMismatch, AtomPhrase, tree};

// - General

impl<T> tree::Mixfix<T> {
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

    /// Fills each position of a mixop, calling `fill_arg` once per position.
    pub fn fill(mixop: Rc<tree::Mixop>, fill_arg: impl FnMut(usize) -> T) -> Self {
        let arity = mixop.arity();
        Self { args: (0..arity).map(fill_arg).collect(), mixop }
    }
}

// - Parts

impl<T> tree::Mixfix<T> {
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
