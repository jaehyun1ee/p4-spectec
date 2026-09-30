//! Tree representation of notation: `Mixfix<T>` and `Mixop`
//!
//! `Tree` holds a node's children in place:
//! a box per bracket or infix side, a vector per sequence.
//! `Mixfix<T>` is the notation form with arguments of type `T`:
//! types for a notation type, expressions for a notation expression,
//! values for a case value, `()` for the bare shape (`Mixop`).
//! Comparison, hashing, and equality look at atom names and arguments,
//! never at atom spans; `eq_shape` compares atoms only.

use std::{
    cmp::Ordering,
    fmt,
    hash::{Hash, Hasher},
};

use serde::{Deserialize, Serialize};
use serde_derive_state::{DeserializeState, SerializeState};

use crate::lang::{
    common::{ds::set::IdSet, source::Span},
    traits::{
        at::At,
        cmp::SyntaxCmp,
        eq::SyntaxEq,
        free::FreeIds,
        print::{Print, Printer},
    },
};

use super::{
    node::{AtomPhrase, Expand, Node, Repr},
    walk,
};

// == Types

/// Children held in place: a box per bracket or infix side, a vector per sequence.
#[derive(Clone, Copy, Debug)]
pub struct Tree;

impl<A> Repr<A> for Tree {
    type Child = Box<Node<A, Tree>>;
    type Children = Vec<Node<A, Tree>>;
}

impl<A> Expand<A> for Tree {
    type Ctx = ();

    fn child<'a>((): &'a (), child: &'a Self::Child) -> &'a Node<A, Self>
    where
        A: 'a,
    {
        child
    }

    fn children<'a>(
        (): &'a (),
        children: &'a Self::Children,
    ) -> impl ExactSizeIterator<Item = &'a Node<A, Self>> + 'a
    where
        A: 'a,
    {
        children.iter()
    }
}

/// A mixfix expression: literal atoms interleaved with argument holes of `T`.
///
/// Equality, ordering, and hashing ignore atom spans.
pub type Mixfix<T> = Node<T, Tree>;

// - Cloning and debugging

impl<T: Clone> Clone for Mixfix<T> {
    fn clone(&self) -> Self {
        match self {
            Self::Arg(arg) => Self::Arg(arg.clone()),
            Self::Atom(atom) => Self::Atom(atom.clone()),
            Self::Brack(atom_l, mixfix, atom_r) => {
                Self::Brack(atom_l.clone(), mixfix.clone(), atom_r.clone())
            }
            Self::Infix(mixfix_l, atom, mixfix_r) => {
                Self::Infix(mixfix_l.clone(), atom.clone(), mixfix_r.clone())
            }
            Self::Seq(mixfixes) => Self::Seq(mixfixes.clone()),
        }
    }
}

// The same text a derive prints: variant names and fields, no type name
impl<T: fmt::Debug> fmt::Debug for Mixfix<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Arg(arg) => fmt.debug_tuple("Arg").field(arg).finish(),
            Self::Atom(atom) => fmt.debug_tuple("Atom").field(atom).finish(),
            Self::Brack(atom_l, mixfix, atom_r) => fmt
                .debug_tuple("Brack")
                .field(atom_l)
                .field(mixfix)
                .field(atom_r)
                .finish(),
            Self::Infix(mixfix_l, atom, mixfix_r) => fmt
                .debug_tuple("Infix")
                .field(mixfix_l)
                .field(atom)
                .field(mixfix_r)
                .finish(),
            Self::Seq(mixfixes) => fmt.debug_tuple("Seq").field(mixfixes).finish(),
        }
    }
}

// == Source locations

impl<T: At> At for Mixfix<T> {
    fn at(&self) -> Span {
        // Collect actual occurrences so empty sequences add no default span
        fn collect<T: At>(mixfix: &Mixfix<T>, spans: &mut Vec<Span>) {
            match mixfix {
                Mixfix::Arg(arg) => spans.push(arg.at()),
                Mixfix::Atom(atom) => spans.push(atom.at()),
                Mixfix::Brack(atom_l, mixfix_inner, atom_r) => {
                    spans.push(atom_l.at());
                    collect(mixfix_inner, spans);
                    spans.push(atom_r.at());
                }
                Mixfix::Infix(mixfix_l, atom, mixfix_r) => {
                    collect(mixfix_l, spans);
                    spans.push(atom.at());
                    collect(mixfix_r, spans);
                }
                Mixfix::Seq(mixfixes) => {
                    for mixfix in mixfixes {
                        collect(mixfix, spans);
                    }
                }
            }
        }

        // Cover the complete token set rather than only the argument positions
        let mut spans = Vec::new();
        collect(self, &mut spans);
        spans.at()
    }
}

// == Equality and comparison

impl<T> Mixfix<T> {
    // - Comparison

    /// Compares structure and atoms lexicographically,
    /// using `compare_arg` for arguments (`walk::cmp_by`).
    pub fn cmp_by<U>(
        &self,
        mixfix_other: &Mixfix<U>,
        compare_arg: impl FnMut(&T, &U) -> Ordering,
    ) -> Ordering {
        walk::cmp_by(&(), self, &(), mixfix_other, compare_arg)
    }

    /// Compares structure and atoms, using `eq_arg` for arguments (`walk::eq_by`).
    pub fn eq_by<U>(&self, mixfix_other: &Mixfix<U>, eq_arg: impl FnMut(&T, &U) -> bool) -> bool {
        walk::eq_by(&(), self, &(), mixfix_other, eq_arg)
    }

    /// Tests whether two mixfixes have the same atoms and argument positions.
    pub fn eq_shape<U>(&self, mixfix_other: &Mixfix<U>) -> bool {
        self.eq_by(mixfix_other, |_, _| true)
    }
}

impl<T: PartialEq> PartialEq for Mixfix<T> {
    fn eq(&self, mixfix_other: &Self) -> bool {
        self.eq_by(mixfix_other, PartialEq::eq)
    }
}

impl<T: Eq> Eq for Mixfix<T> {}

impl<T: SyntaxEq> SyntaxEq for Mixfix<T> {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.eq_by(other, SyntaxEq::syntax_eq)
    }
}

impl<T: SyntaxCmp> SyntaxCmp for Mixfix<T> {
    fn syntax_cmp(&self, other: &Self) -> Ordering {
        self.cmp_by(other, SyntaxCmp::syntax_cmp)
    }
}

// == Ordering

impl<T: Ord> Ord for Mixfix<T> {
    fn cmp(&self, mixfix_other: &Self) -> Ordering {
        self.cmp_by(mixfix_other, Ord::cmp)
    }
}

impl<T: Ord> PartialOrd for Mixfix<T> {
    fn partial_cmp(&self, mixfix_other: &Self) -> Option<Ordering> {
        Some(self.cmp(mixfix_other))
    }
}

// == Hashing

impl<T: Hash> Hash for Mixfix<T> {
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        // Hash the shape first so different variants rarely collide
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

impl<T: FreeIds> FreeIds for Mixfix<T> {
    fn free_ids_into(&self, free: &mut IdSet) {
        match self {
            Self::Arg(arg) => arg.free_ids_into(free),
            Self::Atom(_) => {}
            Self::Brack(_, mixfix, _) => mixfix.free_ids_into(free),
            Self::Infix(mixfix_l, _, mixfix_r) => {
                mixfix_l.free_ids_into(free);
                mixfix_r.free_ids_into(free);
            }
            Self::Seq(mixfixes) => mixfixes.as_slice().free_ids_into(free),
        }
    }
}

// == Fold, map, and iter

impl<T> Mixfix<T> {
    /// Folds arguments from left to right.
    pub fn fold<A>(&self, acc: A, mut fold_arg: impl FnMut(A, &T) -> A) -> A {
        self.fold_inner(acc, &mut fold_arg)
    }

    /// Folds this subtree's arguments left to right.
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

    /// Maps arguments while preserving mixfix structure and atoms.
    pub fn map<U>(&self, mut map_arg: impl FnMut(&T) -> U) -> Mixfix<U> {
        self.map_inner(&mut map_arg)
    }

    /// Maps this subtree's arguments, cloning atoms.
    fn map_inner<U>(&self, map_arg: &mut impl FnMut(&T) -> U) -> Mixfix<U> {
        match self {
            Self::Arg(arg) => Mixfix::Arg(map_arg(arg)),
            Self::Atom(atom) => Mixfix::Atom(atom.clone()),
            Self::Brack(atom_l, mixfix, atom_r) => {
                Mixfix::Brack(atom_l.clone(), Box::new(mixfix.map_inner(map_arg)), atom_r.clone())
            }
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

    /// Maps arguments in order, stopping at the first error and retaining atoms.
    pub fn try_map<U, E>(
        &self,
        mut map_arg: impl FnMut(&T) -> Result<U, E>,
    ) -> Result<Mixfix<U>, E> {
        self.try_map_inner(&mut map_arg)
    }

    /// Maps this subtree with the same callback and early error propagation.
    fn try_map_inner<U, E>(
        &self,
        map_arg: &mut impl FnMut(&T) -> Result<U, E>,
    ) -> Result<Mixfix<U>, E> {
        Ok(match self {
            Self::Arg(arg) => Mixfix::Arg(map_arg(arg)?),
            Self::Atom(atom) => Mixfix::Atom(atom.clone()),
            Self::Brack(atom_l, mixfix, atom_r) => Mixfix::Brack(
                atom_l.clone(),
                Box::new(mixfix.try_map_inner(map_arg)?),
                atom_r.clone(),
            ),
            Self::Infix(mixfix_l, atom, mixfix_r) => Mixfix::Infix(
                Box::new(mixfix_l.try_map_inner(map_arg)?),
                atom.clone(),
                Box::new(mixfix_r.try_map_inner(map_arg)?),
            ),
            Self::Seq(mixfixes) => Mixfix::Seq(
                mixfixes
                    .iter()
                    .map(|mixfix| mixfix.try_map_inner(map_arg))
                    .collect::<Result<_, _>>()?,
            ),
        })
    }

    /// Visits arguments from left to right.
    pub fn iter(&self, mut visit_arg: impl FnMut(&T)) {
        self.fold((), |(), arg| visit_arg(arg));
    }
}

// == Utilities using fold, map, and iter

impl<T> Mixfix<T> {
    // - Arity

    /// Returns the number of argument positions.
    pub fn arity(&self) -> usize {
        self.fold(0, |arity, _| arity + 1)
    }

    // - Atoms and args

    /// Collects arguments in left-to-right tree order.
    pub fn args(&self) -> Vec<&T> {
        let mut args = Vec::with_capacity(self.arity());
        self.collect_args(&mut args);
        args
    }

    /// Appends this subtree's arguments in tree order.
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

    /// Collects owned arguments in left-to-right tree order.
    pub fn into_args(self) -> Vec<T> {
        let mut args = Vec::with_capacity(self.arity());
        self.collect_into_args(&mut args);
        args
    }

    /// Moves this subtree's arguments out in tree order.
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

impl<T> Mixfix<T> {
    /// Writes atoms and arguments, separating non-empty pieces with spaces
    /// (`walk::print_with`).
    pub fn print_with(
        &self,
        printer: &mut Printer<'_>,
        print_arg: impl FnMut(&T, &mut Printer<'_>) -> fmt::Result,
    ) -> fmt::Result {
        walk::print_with(&(), self, printer, print_arg)
    }
}

// == Mixops

/// A mixfix shape with unfilled argument positions.
pub type Mixop = Mixfix<()>;

impl Print for Mixop {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        self.print_with(printer, |(), printer| printer.write("%"))
    }
}

// - Syntax operations

impl SyntaxEq for () {
    fn syntax_eq(&self, _other: &Self) -> bool {
        true
    }
}

impl FreeIds for () {
    fn free_ids(&self) -> IdSet {
        IdSet::new()
    }
}

// - Converting a mixfix to a mixop

impl<T> Mixfix<T> {
    /// Replaces every argument with an unfilled mixop position.
    pub fn to_mixop(&self) -> Mixop {
        self.map(|_| ())
    }

    /// Separates the mixop shape from its arguments.
    pub fn split(&self) -> (Mixop, Vec<&T>) {
        (self.to_mixop(), self.args())
    }
}

// == Serialization

// - Plain encode and decode

// Written out rather than derived on the generic `Node`,
// with the variant names and shapes the former `Mixfix` derive produced
impl<T: Serialize> Serialize for Mixfix<T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        #[serde(rename = "Mixfix")]
        enum MixfixRef<'a, T> {
            Arg(&'a T),
            Atom(&'a AtomPhrase),
            Brack(&'a AtomPhrase, &'a Mixfix<T>, &'a AtomPhrase),
            Infix(&'a Mixfix<T>, &'a AtomPhrase, &'a Mixfix<T>),
            Seq(&'a [Mixfix<T>]),
        }

        let mixfix = match self {
            Self::Arg(arg) => MixfixRef::Arg(arg),
            Self::Atom(atom) => MixfixRef::Atom(atom),
            Self::Brack(atom_l, mixfix, atom_r) => MixfixRef::Brack(atom_l, mixfix, atom_r),
            Self::Infix(mixfix_l, atom, mixfix_r) => MixfixRef::Infix(mixfix_l, atom, mixfix_r),
            Self::Seq(mixfixes) => MixfixRef::Seq(mixfixes),
        };
        mixfix.serialize(serializer)
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Mixfix<T> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Recursive children use Mixfix so only this level needs conversion
        #[derive(Deserialize)]
        #[serde(rename = "Mixfix")]
        enum MixfixOwned<T> {
            Arg(T),
            Atom(AtomPhrase),
            Brack(AtomPhrase, Box<Mixfix<T>>, AtomPhrase),
            Infix(Box<Mixfix<T>>, AtomPhrase, Box<Mixfix<T>>),
            Seq(Vec<Mixfix<T>>),
        }

        Ok(match MixfixOwned::deserialize(deserializer)? {
            MixfixOwned::Arg(arg) => Self::Arg(arg),
            MixfixOwned::Atom(atom) => Self::Atom(atom),
            MixfixOwned::Brack(atom_l, mixfix, atom_r) => Self::Brack(atom_l, mixfix, atom_r),
            MixfixOwned::Infix(mixfix_l, atom, mixfix_r) => Self::Infix(mixfix_l, atom, mixfix_r),
            MixfixOwned::Seq(mixfixes) => Self::Seq(mixfixes),
        })
    }
}

// - Encode

// Recursive mixfix boxes are not separated by phrase nodes,
// so grow the stack here rather than relying on `NotePhrase`
impl<T, State> serde_state::SerializeState<State> for Mixfix<T>
where
    T: serde_state::SerializeState<State>,
{
    fn serialize_state<Serializer>(
        &self,
        serializer: Serializer,
        state: &State,
    ) -> Result<Serializer::Ok, Serializer::Error>
    where
        Serializer: serde::Serializer,
    {
        #[derive(SerializeState)]
        #[serde(rename = "Mixfix")]
        #[serde(serialize_state = "State", ser_parameters = "State")]
        #[serde(bound(serialize = "T: serde_state::SerializeState<State>"))]
        enum MixfixState<'a, T> {
            Arg(#[serde(state)] &'a T),
            Atom(#[serde(state)] &'a AtomPhrase),
            Brack(
                #[serde(state)] &'a AtomPhrase,
                #[serde(state)] &'a Mixfix<T>,
                #[serde(state)] &'a AtomPhrase,
            ),
            Infix(
                #[serde(state)] &'a Mixfix<T>,
                #[serde(state)] &'a AtomPhrase,
                #[serde(state)] &'a Mixfix<T>,
            ),
            Seq(#[serde(state)] &'a [Mixfix<T>]),
        }

        stacker::maybe_grow(64 * 1024, 1024 * 1024, || {
            let mixfix = match self {
                Self::Arg(arg) => MixfixState::Arg(arg),
                Self::Atom(atom) => MixfixState::Atom(atom),
                Self::Brack(atom_l, mixfix, atom_r) => MixfixState::Brack(atom_l, mixfix, atom_r),
                Self::Infix(mixfix_l, atom, mixfix_r) => {
                    MixfixState::Infix(mixfix_l, atom, mixfix_r)
                }
                Self::Seq(mixfixes) => MixfixState::Seq(mixfixes),
            };
            mixfix.serialize_state(serializer, state)
        })
    }
}

// - Decode

impl<'de, T, State> serde_state::DeserializeState<'de, State> for Mixfix<T>
where
    T: serde_state::DeserializeState<'de, State>,
{
    fn deserialize_state<Deserializer>(
        state: &mut State,
        deserializer: Deserializer,
    ) -> Result<Self, Deserializer::Error>
    where
        Deserializer: serde::Deserializer<'de>,
    {
        // Recursive children use Mixfix so only this level needs conversion
        #[derive(DeserializeState)]
        #[serde(rename = "Mixfix")]
        #[serde(deserialize_state = "State", de_parameters = "State")]
        #[serde(bound(deserialize = "T: serde_state::DeserializeState<'de, State>"))]
        enum MixfixState<T> {
            Arg(#[serde(state)] T),
            Atom(#[serde(state)] AtomPhrase),
            Brack(
                #[serde(state)] AtomPhrase,
                #[serde(state)] Box<Mixfix<T>>,
                #[serde(state)] AtomPhrase,
            ),
            Infix(
                #[serde(state)] Box<Mixfix<T>>,
                #[serde(state)] AtomPhrase,
                #[serde(state)] Box<Mixfix<T>>,
            ),
            Seq(#[serde(state)] Vec<Mixfix<T>>),
        }

        Ok(match MixfixState::deserialize_state(state, deserializer)? {
            MixfixState::Arg(arg) => Self::Arg(arg),
            MixfixState::Atom(atom) => Self::Atom(atom),
            MixfixState::Brack(atom_l, mixfix, atom_r) => Self::Brack(atom_l, mixfix, atom_r),
            MixfixState::Infix(mixfix_l, atom, mixfix_r) => Self::Infix(mixfix_l, atom, mixfix_r),
            MixfixState::Seq(mixfixes) => Self::Seq(mixfixes),
        })
    }
}
