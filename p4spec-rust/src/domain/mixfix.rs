use std::{cmp::Ordering, fmt, hash::Hash};

use thiserror::Error;

use super::{atom::Atom, source::Phrase};

pub type AtomPhrase = Phrase<Atom>;
pub type Mixop = Mixfix<()>;

#[derive(Clone, Debug)]
pub enum Mixfix<T> {
    Arg(T),
    Atom(AtomPhrase),
    Brack(AtomPhrase, Box<Self>, AtomPhrase),
    Infix(Box<Self>, AtomPhrase, Box<Self>),
    Seq(Vec<Self>),
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum ArityMismatch {
    #[error("too few mixfix arguments")]
    TooFew,

    #[error("too many mixfix arguments")]
    TooMany,
}

// Equality and comparison

impl<T: PartialEq> PartialEq for Mixfix<T> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Arg(left), Self::Arg(right)) => left == right,
            (Self::Atom(left), Self::Atom(right)) => left.it == right.it,
            (Self::Brack(left_l, body_l, right_l), Self::Brack(left_r, body_r, right_r)) => {
                left_l.it == left_r.it && body_l == body_r && right_l.it == right_r.it
            }
            (Self::Infix(left_l, atom_l, right_l), Self::Infix(left_r, atom_r, right_r)) => {
                left_l == left_r && atom_l.it == atom_r.it && right_l == right_r
            }
            (Self::Seq(items_l), Self::Seq(items_r)) => items_l == items_r,
            _ => false,
        }
    }
}

impl<T: Eq> Eq for Mixfix<T> {}

impl<T: Ord> Ord for Mixfix<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Arg(left), Self::Arg(right)) => left.cmp(right),
            (Self::Atom(left), Self::Atom(right)) => left.it.cmp(&right.it),
            (Self::Brack(left_l, body_l, right_l), Self::Brack(left_r, body_r, right_r)) => left_l
                .it
                .cmp(&left_r.it)
                .then_with(|| body_l.cmp(body_r))
                .then_with(|| right_l.it.cmp(&right_r.it)),
            (Self::Infix(left_l, atom_l, right_l), Self::Infix(left_r, atom_r, right_r)) => left_l
                .cmp(left_r)
                .then_with(|| atom_l.it.cmp(&atom_r.it))
                .then_with(|| right_l.cmp(right_r)),
            (Self::Seq(items_l), Self::Seq(items_r)) => items_l.cmp(items_r),
            _ => self.tag().cmp(&other.tag()),
        }
    }
}

impl<T: Ord> PartialOrd for Mixfix<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: Hash> Hash for Mixfix<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.tag().hash(state);
        match self {
            Self::Arg(arg) => arg.hash(state),
            Self::Atom(atom) => atom.it.hash(state),
            Self::Brack(left, body, right) => {
                left.it.hash(state);
                body.hash(state);
                right.it.hash(state);
            }
            Self::Infix(left, atom, right) => {
                left.hash(state);
                atom.it.hash(state);
                right.hash(state);
            }
            Self::Seq(items) => items.hash(state),
        }
    }
}

impl<T> Mixfix<T> {
    fn tag(&self) -> u8 {
        match self {
            Self::Arg(_) => 0,
            Self::Atom(_) => 1,
            Self::Brack(_, _, _) => 2,
            Self::Infix(_, _, _) => 3,
            Self::Seq(_) => 4,
        }
    }

    pub fn cmp_shape<U>(&self, other: &Mixfix<U>) -> Ordering {
        fn compare_slices<T, U>(left: &[Mixfix<T>], right: &[Mixfix<U>]) -> Ordering {
            for (item_l, item_r) in left.iter().zip(right) {
                let ordering = item_l.cmp_shape(item_r);
                if ordering != Ordering::Equal {
                    return ordering;
                }
            }
            left.len().cmp(&right.len())
        }

        match (self, other) {
            (Self::Arg(_), Mixfix::Arg(_)) => Ordering::Equal,
            (Self::Atom(left), Mixfix::Atom(right)) => left.it.cmp(&right.it),
            (Self::Brack(left_l, body_l, right_l), Mixfix::Brack(left_r, body_r, right_r)) => {
                left_l
                    .it
                    .cmp(&left_r.it)
                    .then_with(|| body_l.cmp_shape(body_r))
                    .then_with(|| right_l.it.cmp(&right_r.it))
            }
            (Self::Infix(left_l, atom_l, right_l), Mixfix::Infix(left_r, atom_r, right_r)) => {
                left_l
                    .cmp_shape(left_r)
                    .then_with(|| atom_l.it.cmp(&atom_r.it))
                    .then_with(|| right_l.cmp_shape(right_r))
            }
            (Self::Seq(items_l), Mixfix::Seq(items_r)) => compare_slices(items_l, items_r),
            _ => self.tag().cmp(&other.tag()),
        }
    }

    pub fn same_shape<U>(&self, other: &Mixfix<U>) -> bool {
        self.cmp_shape(other) == Ordering::Equal
    }
}

// Fold, map, and iter

impl<T> Mixfix<T> {
    pub fn fold<A>(&self, initial: A, mut fold_arg: impl FnMut(A, &T) -> A) -> A {
        self.fold_with(initial, &mut fold_arg)
    }

    fn fold_with<A>(&self, initial: A, fold_arg: &mut impl FnMut(A, &T) -> A) -> A {
        match self {
            Self::Arg(arg) => fold_arg(initial, arg),
            Self::Atom(_) => initial,
            Self::Brack(_, body, _) => body.fold_with(initial, fold_arg),
            Self::Infix(left, _, right) => {
                let after_left = left.fold_with(initial, fold_arg);
                right.fold_with(after_left, fold_arg)
            }
            Self::Seq(items) => items
                .iter()
                .fold(initial, |acc, item| item.fold_with(acc, fold_arg)),
        }
    }

    pub fn map<U>(&self, mut map_arg: impl FnMut(&T) -> U) -> Mixfix<U> {
        self.map_with(&mut map_arg)
    }

    fn map_with<U>(&self, map_arg: &mut impl FnMut(&T) -> U) -> Mixfix<U> {
        match self {
            Self::Arg(arg) => Mixfix::Arg(map_arg(arg)),
            Self::Atom(atom) => Mixfix::Atom(atom.clone()),
            Self::Brack(left, body, right) => Mixfix::Brack(
                left.clone(),
                Box::new(body.map_with(map_arg)),
                right.clone(),
            ),
            Self::Infix(left, atom, right) => Mixfix::Infix(
                Box::new(left.map_with(map_arg)),
                atom.clone(),
                Box::new(right.map_with(map_arg)),
            ),
            Self::Seq(items) => {
                Mixfix::Seq(items.iter().map(|item| item.map_with(map_arg)).collect())
            }
        }
    }
}

impl<T: Clone> Mixfix<T> {
    pub fn map_atoms(&self, mut map_atom: impl FnMut(&AtomPhrase) -> AtomPhrase) -> Self {
        fn go<T: Clone>(
            mixfix: &Mixfix<T>,
            map_atom: &mut impl FnMut(&AtomPhrase) -> AtomPhrase,
        ) -> Mixfix<T> {
            match mixfix {
                Mixfix::Arg(arg) => Mixfix::Arg(arg.clone()),
                Mixfix::Atom(atom) => Mixfix::Atom(map_atom(atom)),
                Mixfix::Brack(left, body, right) => Mixfix::Brack(
                    map_atom(left),
                    Box::new(go(body, map_atom)),
                    map_atom(right),
                ),
                Mixfix::Infix(left, atom, right) => Mixfix::Infix(
                    Box::new(go(left, map_atom)),
                    map_atom(atom),
                    Box::new(go(right, map_atom)),
                ),
                Mixfix::Seq(items) => {
                    Mixfix::Seq(items.iter().map(|item| go(item, map_atom)).collect())
                }
            }
        }

        go(self, &mut map_atom)
    }
}

impl<T> Mixfix<T> {
    pub fn iter(&self, mut visit_arg: impl FnMut(&T)) {
        self.fold((), |(), arg| visit_arg(arg));
    }

    pub fn iter_atoms(&self, mut visit_atom: impl FnMut(&AtomPhrase)) {
        for atom in self.atoms() {
            visit_atom(atom);
        }
    }
}

// Conversion

impl<T> Mixfix<T> {
    fn display_string(&self) -> String {
        fn inner<T>(mixfix: &Mixfix<T>) -> String {
            match mixfix {
                Mixfix::Arg(_) => "%".into(),
                Mixfix::Atom(atom) => atom.it.render(),
                Mixfix::Brack(left, body, right) => {
                    format!("{}{}{}", left.it.render(), inner(body), right.it.render())
                }
                Mixfix::Infix(left, atom, right) => {
                    format!("{}{}{}", inner(left), atom.it.render(), inner(right))
                }
                Mixfix::Seq(items) => items.iter().map(inner).collect::<Vec<_>>().join(" "),
            }
        }

        format!("`{}`", inner(self))
    }

    pub fn to_mixop(&self) -> Mixop {
        self.map(|_| ())
    }
}

impl<T> fmt::Display for Mixfix<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.display_string())
    }
}

// Arity

impl<T> Mixfix<T> {
    pub fn arity(&self) -> usize {
        match self {
            Self::Arg(_) => 1,
            Self::Atom(_) => 0,
            Self::Brack(_, body, _) => body.arity(),
            Self::Infix(left, _, right) => left.arity() + right.arity(),
            Self::Seq(items) => items.iter().map(Self::arity).sum(),
        }
    }
}

// Atoms and args

impl<T> Mixfix<T> {
    pub fn atoms(&self) -> Vec<&AtomPhrase> {
        let mut atoms = Vec::new();
        self.collect_atoms(&mut atoms);
        atoms
    }

    fn collect_atoms<'a>(&'a self, atoms: &mut Vec<&'a AtomPhrase>) {
        match self {
            Self::Arg(_) => {}
            Self::Atom(atom) => atoms.push(atom),
            Self::Brack(left, body, right) => {
                atoms.push(left);
                body.collect_atoms(atoms);
                atoms.push(right);
            }
            Self::Infix(left, atom, right) => {
                left.collect_atoms(atoms);
                atoms.push(atom);
                right.collect_atoms(atoms);
            }
            Self::Seq(items) => {
                for item in items {
                    item.collect_atoms(atoms);
                }
            }
        }
    }

    pub fn atoms_matrix(&self) -> Vec<Vec<&AtomPhrase>> {
        enum Part<'a> {
            Atom(&'a AtomPhrase),
            Arg,
        }

        fn collect<'a, T>(mixfix: &'a Mixfix<T>, parts: &mut Vec<Part<'a>>) {
            match mixfix {
                Mixfix::Arg(_) => parts.push(Part::Arg),
                Mixfix::Atom(atom) => parts.push(Part::Atom(atom)),
                Mixfix::Brack(left, body, right) => {
                    parts.push(Part::Atom(left));
                    collect(body, parts);
                    parts.push(Part::Atom(right));
                }
                Mixfix::Infix(left, atom, right) => {
                    collect(left, parts);
                    parts.push(Part::Atom(atom));
                    collect(right, parts);
                }
                Mixfix::Seq(items) => {
                    for item in items {
                        collect(item, parts);
                    }
                }
            }
        }

        let mut parts = Vec::new();
        collect(self, &mut parts);
        let mut matrix = vec![Vec::new()];
        for part in parts {
            match part {
                Part::Atom(atom) => matrix.last_mut().expect("matrix is non-empty").push(atom),
                Part::Arg => matrix.push(Vec::new()),
            }
        }
        matrix
    }

    pub fn args(&self) -> Vec<&T> {
        let mut args = Vec::with_capacity(self.arity());
        self.collect_args(&mut args);
        args
    }

    fn collect_args<'a>(&'a self, args: &mut Vec<&'a T>) {
        match self {
            Self::Arg(arg) => args.push(arg),
            Self::Atom(_) => {}
            Self::Brack(_, body, _) => body.collect_args(args),
            Self::Infix(left, _, right) => {
                left.collect_args(args);
                right.collect_args(args);
            }
            Self::Seq(items) => {
                for item in items {
                    item.collect_args(args);
                }
            }
        }
    }
}

// Filling and splitting

impl Mixop {
    pub fn fill<T>(
        mixop: &Self,
        args: impl IntoIterator<Item = T>,
    ) -> Result<Mixfix<T>, ArityMismatch> {
        fn fill_next<T>(
            mixop: &Mixop,
            args: &mut impl Iterator<Item = T>,
        ) -> Result<Mixfix<T>, ArityMismatch> {
            match mixop {
                Mixfix::Arg(()) => args.next().map(Mixfix::Arg).ok_or(ArityMismatch::TooFew),
                Mixfix::Atom(atom) => Ok(Mixfix::Atom(atom.clone())),
                Mixfix::Brack(left, body, right) => Ok(Mixfix::Brack(
                    left.clone(),
                    Box::new(fill_next(body, args)?),
                    right.clone(),
                )),
                Mixfix::Infix(left, atom, right) => Ok(Mixfix::Infix(
                    Box::new(fill_next(left, args)?),
                    atom.clone(),
                    Box::new(fill_next(right, args)?),
                )),
                Mixfix::Seq(items) => Ok(Mixfix::Seq(
                    items
                        .iter()
                        .map(|item| fill_next(item, args))
                        .collect::<Result<_, _>>()?,
                )),
            }
        }

        let mut args = args.into_iter();
        let mixfix = fill_next(mixop, &mut args)?;
        if args.next().is_some() {
            Err(ArityMismatch::TooMany)
        } else {
            Ok(mixfix)
        }
    }
}

impl<T> Mixfix<T> {
    pub fn split(&self) -> (Mixop, Vec<&T>) {
        (self.to_mixop(), self.args())
    }
}

// Rendering

impl<T: Clone> Mixfix<T> {
    pub fn assemble(
        &self,
        empty: T,
        space: T,
        mut atom: impl FnMut(&AtomPhrase) -> Option<T>,
        mut concat: impl FnMut(T, T) -> T,
    ) -> T {
        fn join<T: Clone>(
            pieces: impl IntoIterator<Item = Option<T>>,
            space: &T,
            concat: &mut impl FnMut(T, T) -> T,
        ) -> Option<T> {
            let mut pieces = pieces.into_iter().flatten();
            let first = pieces.next()?;
            Some(pieces.fold(first, |acc, piece| {
                let with_space = concat(acc, space.clone());
                concat(with_space, piece)
            }))
        }

        fn go<T: Clone>(
            mixfix: &Mixfix<T>,
            space: &T,
            atom: &mut impl FnMut(&AtomPhrase) -> Option<T>,
            concat: &mut impl FnMut(T, T) -> T,
        ) -> Option<T> {
            match mixfix {
                Mixfix::Arg(arg) => Some(arg.clone()),
                Mixfix::Atom(atom_value) => atom(atom_value),
                Mixfix::Brack(left, body, right) => join(
                    [atom(left), go(body, space, atom, concat), atom(right)],
                    space,
                    concat,
                ),
                Mixfix::Infix(left, atom_value, right) => join(
                    [
                        go(left, space, atom, concat),
                        atom(atom_value),
                        go(right, space, atom, concat),
                    ],
                    space,
                    concat,
                ),
                Mixfix::Seq(items) => {
                    let pieces = items
                        .iter()
                        .map(|item| go(item, space, atom, concat))
                        .collect::<Vec<_>>();
                    join(pieces, space, concat)
                }
            }
        }

        go(self, &space, &mut atom, &mut concat).unwrap_or(empty)
    }
}

impl<T> Mixfix<T> {
    pub fn render(
        &self,
        mut string_of_atom: impl FnMut(&AtomPhrase) -> String,
        mut string_of_arg: impl FnMut(&T) -> String,
    ) -> String {
        self.map(|arg| string_of_arg(arg)).assemble(
            String::new(),
            " ".to_owned(),
            |atom| {
                let rendered = string_of_atom(atom);
                (!rendered.is_empty()).then_some(rendered)
            },
            |left, right| left + &right,
        )
    }
}
