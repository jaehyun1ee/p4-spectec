use std::cmp::Ordering;

use super::mixfix::{ArityMismatch, AtomPhrase, Mixop};

pub type T = Mixop;

pub fn compare(left: &T, right: &T) -> Ordering {
    left.cmp(right)
}

pub fn eq(left: &T, right: &T) -> bool {
    left == right
}

pub fn arity(mixop: &T) -> usize {
    mixop.arity()
}

pub fn atoms(mixop: &T) -> Vec<&AtomPhrase> {
    mixop.atoms()
}

pub fn atoms_matrix(mixop: &T) -> Vec<Vec<&AtomPhrase>> {
    mixop.atoms_matrix()
}

pub fn string_of_mixop(mixop: &T) -> String {
    mixop.to_string()
}

pub fn assemble(
    mixop: &T,
    args: impl IntoIterator<Item = String>,
    string_of_atom: impl FnMut(&AtomPhrase) -> String,
) -> Result<String, ArityMismatch> {
    Ok(T::fill(mixop, args)?.render(string_of_atom, Clone::clone))
}
