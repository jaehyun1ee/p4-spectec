use std::{
    cmp::Ordering,
    error::Error,
    fmt,
    hash::{Hash, Hasher},
};

use super::{atom::Atom, source::Spanned};

pub type AtomPhrase = Spanned<Atom>;
pub type Mixop = Mixfix<()>;

#[derive(Clone, Debug)]
pub enum Mixfix<T> {
    Arg(T),
    Atom(AtomPhrase),
    Brack(AtomPhrase, Box<Self>, AtomPhrase),
    Infix(Box<Self>, AtomPhrase, Box<Self>),
    Seq(Vec<Self>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArityMismatch {
    TooFew,
    TooMany,
}

impl fmt::Display for ArityMismatch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooFew => formatter.write_str("Mixfix.fill: too few arguments"),
            Self::TooMany => formatter.write_str("Mixfix.fill: too many arguments"),
        }
    }
}

impl Error for ArityMismatch {}

// Equality and comparison

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

    pub fn cmp_by<U>(
        &self,
        other: &Mixfix<U>,
        mut compare_arg: impl FnMut(&T, &U) -> Ordering,
    ) -> Ordering {
        fn compare<T, U>(
            left: &Mixfix<T>,
            right: &Mixfix<U>,
            compare_arg: &mut impl FnMut(&T, &U) -> Ordering,
        ) -> Ordering {
            match (left, right) {
                (Mixfix::Arg(arg_l), Mixfix::Arg(arg_r)) => compare_arg(arg_l, arg_r),
                (Mixfix::Atom(atom_l), Mixfix::Atom(atom_r)) => atom_l.node.cmp(&atom_r.node),
                (
                    Mixfix::Brack(atom_l_l, body_l, atom_l_r),
                    Mixfix::Brack(atom_r_l, body_r, atom_r_r),
                ) => atom_l_l
                    .node
                    .cmp(&atom_r_l.node)
                    .then_with(|| compare(body_l, body_r, compare_arg))
                    .then_with(|| atom_l_r.node.cmp(&atom_r_r.node)),
                (
                    Mixfix::Infix(left_l, atom_l, right_l),
                    Mixfix::Infix(left_r, atom_r, right_r),
                ) => compare(left_l, left_r, compare_arg)
                    .then_with(|| atom_l.node.cmp(&atom_r.node))
                    .then_with(|| compare(right_l, right_r, compare_arg)),
                (Mixfix::Seq(items_l), Mixfix::Seq(items_r)) => {
                    for (item_l, item_r) in items_l.iter().zip(items_r) {
                        let ordering = compare(item_l, item_r, compare_arg);
                        if ordering != Ordering::Equal {
                            return ordering;
                        }
                    }
                    items_l.len().cmp(&items_r.len())
                }
                _ => left.tag().cmp(&right.tag()),
            }
        }

        compare(self, other, &mut compare_arg)
    }

    pub fn eq_by<U>(&self, other: &Mixfix<U>, mut eq_arg: impl FnMut(&T, &U) -> bool) -> bool {
        fn equal<T, U>(
            left: &Mixfix<T>,
            right: &Mixfix<U>,
            eq_arg: &mut impl FnMut(&T, &U) -> bool,
        ) -> bool {
            match (left, right) {
                (Mixfix::Arg(arg_l), Mixfix::Arg(arg_r)) => eq_arg(arg_l, arg_r),
                (Mixfix::Atom(atom_l), Mixfix::Atom(atom_r)) => atom_l.node == atom_r.node,
                (
                    Mixfix::Brack(atom_l_l, body_l, atom_l_r),
                    Mixfix::Brack(atom_r_l, body_r, atom_r_r),
                ) => {
                    atom_l_l.node == atom_r_l.node
                        && equal(body_l, body_r, eq_arg)
                        && atom_l_r.node == atom_r_r.node
                }
                (
                    Mixfix::Infix(left_l, atom_l, right_l),
                    Mixfix::Infix(left_r, atom_r, right_r),
                ) => {
                    atom_l.node == atom_r.node
                        && equal(left_l, left_r, eq_arg)
                        && equal(right_l, right_r, eq_arg)
                }
                (Mixfix::Seq(items_l), Mixfix::Seq(items_r)) => {
                    items_l.len() == items_r.len()
                        && items_l
                            .iter()
                            .zip(items_r)
                            .all(|(item_l, item_r)| equal(item_l, item_r, eq_arg))
                }
                _ => false,
            }
        }

        equal(self, other, &mut eq_arg)
    }

    pub fn cmp_mixop<U>(&self, other: &Mixfix<U>) -> Ordering {
        self.cmp_by(other, |_, _| Ordering::Equal)
    }

    pub fn eq_mixop<U>(&self, other: &Mixfix<U>) -> bool {
        self.eq_by(other, |_, _| true)
    }
}

impl<T: PartialEq> PartialEq for Mixfix<T> {
    fn eq(&self, other: &Self) -> bool {
        self.eq_by(other, PartialEq::eq)
    }
}

impl<T: Eq> Eq for Mixfix<T> {}

impl<T: Ord> Ord for Mixfix<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.cmp_by(other, Ord::cmp)
    }
}

impl<T: Ord> PartialOrd for Mixfix<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: Hash> Hash for Mixfix<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.tag().hash(state);
        match self {
            Self::Arg(arg) => arg.hash(state),
            Self::Atom(atom) => atom.node.hash(state),
            Self::Brack(left, body, right) => {
                left.node.hash(state);
                body.hash(state);
                right.node.hash(state);
            }
            Self::Infix(left, atom, right) => {
                left.hash(state);
                atom.node.hash(state);
                right.hash(state);
            }
            Self::Seq(items) => items.hash(state),
        }
    }
}

// Fold, map, and iter

impl<T> Mixfix<T> {
    pub fn fold<A>(&self, initial: A, mut fold_arg: impl FnMut(A, &T) -> A) -> A {
        fn fold<T, A>(mixfix: &Mixfix<T>, initial: A, fold_arg: &mut impl FnMut(A, &T) -> A) -> A {
            match mixfix {
                Mixfix::Arg(arg) => fold_arg(initial, arg),
                Mixfix::Atom(_) => initial,
                Mixfix::Brack(_, body, _) => fold(body, initial, fold_arg),
                Mixfix::Infix(left, _, right) => {
                    let initial = fold(left, initial, fold_arg);
                    fold(right, initial, fold_arg)
                }
                Mixfix::Seq(items) => items
                    .iter()
                    .fold(initial, |acc, item| fold(item, acc, fold_arg)),
            }
        }

        fold(self, initial, &mut fold_arg)
    }

    pub fn map<U>(&self, mut map_arg: impl FnMut(&T) -> U) -> Mixfix<U> {
        fn map<T, U>(mixfix: &Mixfix<T>, map_arg: &mut impl FnMut(&T) -> U) -> Mixfix<U> {
            match mixfix {
                Mixfix::Arg(arg) => Mixfix::Arg(map_arg(arg)),
                Mixfix::Atom(atom) => Mixfix::Atom(atom.clone()),
                Mixfix::Brack(left, body, right) => {
                    Mixfix::Brack(left.clone(), Box::new(map(body, map_arg)), right.clone())
                }
                Mixfix::Infix(left, atom, right) => Mixfix::Infix(
                    Box::new(map(left, map_arg)),
                    atom.clone(),
                    Box::new(map(right, map_arg)),
                ),
                Mixfix::Seq(items) => {
                    Mixfix::Seq(items.iter().map(|item| map(item, map_arg)).collect())
                }
            }
        }

        map(self, &mut map_arg)
    }

    pub fn iter(&self, mut visit_arg: impl FnMut(&T)) {
        self.fold((), |(), arg| visit_arg(arg));
    }

    pub fn iter_atoms(&self, mut visit_atom: impl FnMut(&AtomPhrase)) {
        for atom in self.atoms() {
            visit_atom(atom);
        }
    }

    // Conversion

    pub fn to_mixop(&self) -> Mixop {
        self.map(|_| ())
    }

    // Arity

    pub fn arity(&self) -> usize {
        self.fold(0, |arity, _| arity + 1)
    }

    // Atoms and args

    pub fn atoms(&self) -> Vec<&AtomPhrase> {
        fn collect<'a, T>(mixfix: &'a Mixfix<T>, atoms: &mut Vec<&'a AtomPhrase>) {
            match mixfix {
                Mixfix::Arg(_) => {}
                Mixfix::Atom(atom) => atoms.push(atom),
                Mixfix::Brack(left, body, right) => {
                    atoms.push(left);
                    collect(body, atoms);
                    atoms.push(right);
                }
                Mixfix::Infix(left, atom, right) => {
                    collect(left, atoms);
                    atoms.push(atom);
                    collect(right, atoms);
                }
                Mixfix::Seq(items) => {
                    for item in items {
                        collect(item, atoms);
                    }
                }
            }
        }

        let mut atoms = Vec::new();
        collect(self, &mut atoms);
        atoms
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
        fn collect<'a, T>(mixfix: &'a Mixfix<T>, args: &mut Vec<&'a T>) {
            match mixfix {
                Mixfix::Arg(arg) => args.push(arg),
                Mixfix::Atom(_) => {}
                Mixfix::Brack(_, body, _) => collect(body, args),
                Mixfix::Infix(left, _, right) => {
                    collect(left, args);
                    collect(right, args);
                }
                Mixfix::Seq(items) => {
                    for item in items {
                        collect(item, args);
                    }
                }
            }
        }

        let mut args = Vec::with_capacity(self.arity());
        collect(self, &mut args);
        args
    }

    // Filling and splitting

    pub fn split(&self) -> (Mixop, Vec<&T>) {
        (self.to_mixop(), self.args())
    }

    fn display_string(&self) -> String {
        fn inner<T>(mixfix: &Mixfix<T>) -> String {
            match mixfix {
                Mixfix::Arg(_) => "%".into(),
                Mixfix::Atom(atom) => atom.node.render(),
                Mixfix::Brack(left, body, right) => {
                    format!(
                        "{}{}{}",
                        left.node.render(),
                        inner(body),
                        right.node.render()
                    )
                }
                Mixfix::Infix(left, atom, right) => {
                    format!("{}{}{}", inner(left), atom.node.render(), inner(right))
                }
                Mixfix::Seq(items) => items.iter().map(inner).collect::<Vec<_>>().join(" "),
            }
        }

        format!("`{}`", inner(self))
    }

    // Rendering

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

impl<T: Clone> Mixfix<T> {
    pub fn map_atoms(&self, mut map_atom: impl FnMut(&AtomPhrase) -> AtomPhrase) -> Self {
        fn map<T: Clone>(
            mixfix: &Mixfix<T>,
            map_atom: &mut impl FnMut(&AtomPhrase) -> AtomPhrase,
        ) -> Mixfix<T> {
            match mixfix {
                Mixfix::Arg(arg) => Mixfix::Arg(arg.clone()),
                Mixfix::Atom(atom) => Mixfix::Atom(map_atom(atom)),
                Mixfix::Brack(left, body, right) => Mixfix::Brack(
                    map_atom(left),
                    Box::new(map(body, map_atom)),
                    map_atom(right),
                ),
                Mixfix::Infix(left, atom, right) => Mixfix::Infix(
                    Box::new(map(left, map_atom)),
                    map_atom(atom),
                    Box::new(map(right, map_atom)),
                ),
                Mixfix::Seq(items) => {
                    Mixfix::Seq(items.iter().map(|item| map(item, map_atom)).collect())
                }
            }
        }

        map(self, &mut map_atom)
    }

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
                let acc = concat(acc, space.clone());
                concat(acc, piece)
            }))
        }

        fn assemble<T: Clone>(
            mixfix: &Mixfix<T>,
            space: &T,
            atom: &mut impl FnMut(&AtomPhrase) -> Option<T>,
            concat: &mut impl FnMut(T, T) -> T,
        ) -> Option<T> {
            match mixfix {
                Mixfix::Arg(arg) => Some(arg.clone()),
                Mixfix::Atom(atom_value) => atom(atom_value),
                Mixfix::Brack(left, body, right) => join(
                    [atom(left), assemble(body, space, atom, concat), atom(right)],
                    space,
                    concat,
                ),
                Mixfix::Infix(left, atom_value, right) => join(
                    [
                        assemble(left, space, atom, concat),
                        atom(atom_value),
                        assemble(right, space, atom, concat),
                    ],
                    space,
                    concat,
                ),
                Mixfix::Seq(items) => {
                    let pieces = items
                        .iter()
                        .map(|item| assemble(item, space, atom, concat))
                        .collect::<Vec<_>>();
                    join(pieces, space, concat)
                }
            }
        }

        assemble(self, &space, &mut atom, &mut concat).unwrap_or(empty)
    }
}

impl<T> fmt::Display for Mixfix<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.display_string())
    }
}

#[cfg(test)]
mod tests {
    use super::{AtomPhrase, Mixfix, Mixop};
    use crate::domain::{
        atom::Atom,
        source::{Region, Spanned},
    };

    fn atom(value: Atom) -> AtomPhrase {
        Spanned::new(value, Region::none())
    }

    fn nested_mixop() -> Mixop {
        Mixfix::Brack(
            atom(Atom::LParen),
            Box::new(Mixfix::Infix(
                Box::new(Mixfix::Arg(())),
                atom(Atom::Colon),
                Box::new(Mixfix::Seq(vec![
                    Mixfix::Atom(atom(Atom::Tag("MID".into()))),
                    Mixfix::Arg(()),
                ])),
            )),
            atom(Atom::RParen),
        )
    }

    #[test]
    fn atoms_matrix_preserves_nested_groups_between_arguments() {
        let matrix = nested_mixop()
            .atoms_matrix()
            .into_iter()
            .map(|group| {
                group
                    .into_iter()
                    .map(|atom| atom.node.render())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        assert_eq!(
            matrix,
            [
                vec!["(".to_owned()],
                vec![":".to_owned(), "_MID".to_owned()],
                vec![")".to_owned()],
            ]
        );
    }
}
