use std::{
    cmp::Ordering,
    fmt,
    hash::{Hash, Hasher},
};

use crate::lang::{
    common::ds::set::IdSet,
    traits::{
        eq::SyntaxEq,
        free::Free,
        print::{Print, Printer},
    },
};

use super::{super::source::Spanned, atom::Atom};

/// An atom paired with its source span
pub type AtomPhrase = Spanned<Atom>;

/// A mixfix expression with arguments of type `T`
#[derive(Clone, Debug)]
pub enum Mixfix<T> {
    /// Argument position
    Arg(T),
    /// Literal atom
    Atom(AtomPhrase),
    /// Bracketed expression
    Brack(AtomPhrase, Box<Self>, AtomPhrase),
    /// Infix expression
    Infix(Box<Self>, AtomPhrase, Box<Self>),
    /// Sequence of expressions
    Seq(Vec<Self>),
}

// == Equality and comparison

impl<T> Mixfix<T> {
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
    pub fn cmp_by<U>(
        &self,
        mixfix_other: &Mixfix<U>,
        mut compare_arg: impl FnMut(&T, &U) -> Ordering,
    ) -> Ordering {
        self.cmp_by_inner(mixfix_other, &mut compare_arg)
    }

    fn cmp_by_inner<U>(
        &self,
        mixfix_other: &Mixfix<U>,
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
    pub fn eq_by<U>(
        &self,
        mixfix_other: &Mixfix<U>,
        mut eq_arg: impl FnMut(&T, &U) -> bool,
    ) -> bool {
        self.eq_by_inner(mixfix_other, &mut eq_arg)
    }

    fn eq_by_inner<U>(
        &self,
        mixfix_other: &Mixfix<U>,
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

impl<T: Free> Free for Mixfix<T> {
    fn free(&self) -> IdSet {
        match self {
            Self::Arg(arg) => arg.free(),
            Self::Atom(_) => IdSet::new(),
            Self::Brack(_, mixfix, _) => mixfix.free(),
            Self::Infix(mixfix_l, _, mixfix_r) => mixfix_l.free().union(mixfix_r.free()),
            Self::Seq(mixfixes) => mixfixes
                .iter()
                .fold(IdSet::new(), |free, mixfix| free.union(mixfix.free())),
        }
    }
}

// == Fold, map, and iter

impl<T> Mixfix<T> {
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
    pub fn map<U>(&self, mut map_arg: impl FnMut(&T) -> U) -> Mixfix<U> {
        self.map_inner(&mut map_arg)
    }

    fn map_inner<U>(&self, map_arg: &mut impl FnMut(&T) -> U) -> Mixfix<U> {
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

    /// Visits arguments from left to right
    pub fn iter(&self, mut visit_arg: impl FnMut(&T)) {
        self.fold((), |(), arg| visit_arg(arg));
    }
}

// == Utilities using fold, map, and iter

impl<T> Mixfix<T> {
    // - Arity

    /// Returns the number of argument positions
    pub fn arity(&self) -> usize {
        self.fold(0, |arity, _| arity + 1)
    }

    // - Atoms and args

    /// Collects atoms in left-to-right tree order
    pub fn atoms(&self) -> Vec<&AtomPhrase> {
        let mut atoms = Vec::new();
        self.collect_atoms(&mut atoms);
        atoms
    }

    fn collect_atoms<'a>(&'a self, atoms: &mut Vec<&'a AtomPhrase>) {
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

    /// Groups consecutive atoms, splitting the groups at argument positions
    pub fn atoms_matrix(&self) -> Vec<Vec<&AtomPhrase>> {
        let mut matrix = vec![Vec::new()];
        self.collect_atoms_matrix(&mut matrix);
        matrix
    }

    fn collect_atoms_matrix<'a>(&'a self, matrix: &mut Vec<Vec<&'a AtomPhrase>>) {
        match self {
            Self::Arg(_) => matrix.push(Vec::new()),
            Self::Atom(atom) => matrix.last_mut().expect("matrix is non-empty").push(atom),
            Self::Brack(atom_l, mixfix, atom_r) => {
                matrix.last_mut().expect("matrix is non-empty").push(atom_l);
                mixfix.collect_atoms_matrix(matrix);
                matrix.last_mut().expect("matrix is non-empty").push(atom_r);
            }
            Self::Infix(mixfix_l, atom, mixfix_r) => {
                mixfix_l.collect_atoms_matrix(matrix);
                matrix.last_mut().expect("matrix is non-empty").push(atom);
                mixfix_r.collect_atoms_matrix(matrix);
            }
            Self::Seq(mixfixes) => {
                for mixfix in mixfixes {
                    mixfix.collect_atoms_matrix(matrix);
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
}

// == Assembly

impl<T: Clone> Mixfix<T> {
    /// Combines arguments and selected atoms, inserting `space` between adjacent pieces
    pub fn assemble(
        &self,
        empty: T,
        space: T,
        mut atom: impl FnMut(&AtomPhrase) -> Option<T>,
        mut concat: impl FnMut(T, T) -> T,
    ) -> T {
        self.assemble_inner(&space, &mut atom, &mut concat)
            .unwrap_or(empty)
    }

    fn assemble_join(
        options: impl IntoIterator<Item = Option<T>>,
        space: &T,
        concat: &mut impl FnMut(T, T) -> T,
    ) -> Option<T> {
        let mut values = options.into_iter().flatten();
        let value = values.next()?;
        Some(values.fold(value, |value_l, value_r| {
            let value_l = concat(value_l, space.clone());
            concat(value_l, value_r)
        }))
    }

    fn assemble_inner(
        &self,
        space: &T,
        atom: &mut impl FnMut(&AtomPhrase) -> Option<T>,
        concat: &mut impl FnMut(T, T) -> T,
    ) -> Option<T> {
        match self {
            Self::Arg(arg) => Some(arg.clone()),
            Self::Atom(atom_phrase) => atom(atom_phrase),
            Self::Brack(atom_l, mixfix, atom_r) => Self::assemble_join(
                [
                    atom(atom_l),
                    mixfix.assemble_inner(space, atom, concat),
                    atom(atom_r),
                ],
                space,
                concat,
            ),
            Self::Infix(mixfix_l, atom_phrase, mixfix_r) => Self::assemble_join(
                [
                    mixfix_l.assemble_inner(space, atom, concat),
                    atom(atom_phrase),
                    mixfix_r.assemble_inner(space, atom, concat),
                ],
                space,
                concat,
            ),
            Self::Seq(mixfixes) => {
                let options = mixfixes
                    .iter()
                    .map(|mixfix| mixfix.assemble_inner(space, atom, concat))
                    .collect::<Vec<_>>();
                Self::assemble_join(options, space, concat)
            }
        }
    }
}

// == Rendering

impl<T> Mixfix<T> {
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

        let print_atom = |atom: &AtomPhrase, printer: &mut Printer<'_>, is_first: &mut bool| {
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

    /// Renders atoms and arguments, separating non-empty pieces with spaces
    pub fn render(
        &self,
        mut render_atom: impl FnMut(&AtomPhrase) -> String,
        mut render_arg: impl FnMut(&T) -> String,
    ) -> String {
        self.map(|arg| render_arg(arg)).assemble(
            String::new(),
            " ".to_owned(),
            |atom| {
                let string = render_atom(atom);
                (!string.is_empty()).then_some(string)
            },
            |string_l, string_r| string_l + &string_r,
        )
    }
}

// == Display

impl<T> Mixfix<T> {
    fn display(&self) -> String {
        format!("`{}`", self.display_inner())
    }

    fn display_inner(&self) -> String {
        match self {
            Self::Arg(_) => "%".into(),
            Self::Atom(atom) => atom.node.to_string(),
            Self::Brack(atom_l, mixfix, atom_r) => format!(
                "{}{}{}",
                atom_l.node.to_string(),
                mixfix.display_inner(),
                atom_r.node.to_string()
            ),
            Self::Infix(mixfix_l, atom, mixfix_r) => format!(
                "{}{}{}",
                mixfix_l.display_inner(),
                atom.node.to_string(),
                mixfix_r.display_inner()
            ),
            Self::Seq(mixfixes) => mixfixes
                .iter()
                .map(Self::display_inner)
                .collect::<Vec<_>>()
                .join(" "),
        }
    }
}

impl<T> fmt::Display for Mixfix<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.write_str(&self.display())
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        super::source::{Span, Spanned},
        atom::Atom,
        mixop::Mixop,
    };
    use super::{AtomPhrase, Mixfix};

    fn atom(value: Atom) -> AtomPhrase {
        Spanned::new(value, Span::default())
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
                    .map(|atom| atom.node.to_string())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        assert_eq!(
            matrix,
            [
                vec!["`(".to_owned()],
                vec![":".to_owned(), "_MID".to_owned()],
                vec!["`)".to_owned()],
            ]
        );
    }
}
