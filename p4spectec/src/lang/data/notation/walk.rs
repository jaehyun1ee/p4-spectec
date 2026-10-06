//! Concrete tree and flat notation traversals
//!
//! Comparisons visit atoms and argument positions in notation order.
//! Tree operations read owned children; flat operations resolve child IDs
//! through each operand's arena. Atom comparisons ignore source spans.

use super::{AtomPhrase, MixopArena, flat, tree};
use crate::lang::{
    common::notation::atom::Atom,
    traits::print::{Print, Printer},
};
use std::{cmp::Ordering, fmt};

/// Orders two nodes by structure and atom names, lexicographically.
///
/// Brackets compare the opening atom, the inner form, then the closing atom;
/// infix compares the left form, the operator, then the right form;
/// sequences compare the common prefix, then the length;
/// different forms order by variant.
/// Both walks visit equal prefixes, so they reach argument positions in step,
/// and `compare_arg` orders the arguments at each position.
pub fn cmp_trees_by(
    node_l: &tree::Mixop,
    node_r: &tree::Mixop,
    mut compare_arg: impl FnMut(usize) -> Ordering,
) -> Ordering {
    let mut pos = 0;
    cmp_trees_by_inner(node_l, node_r, &mut pos, &mut compare_arg)
}

/// Structural comparison, threading the position and the argument comparator.
fn cmp_trees_by_inner(
    node_l: &tree::Mixop,
    node_r: &tree::Mixop,
    pos: &mut usize,
    compare_arg: &mut impl FnMut(usize) -> Ordering,
) -> Ordering {
    match (node_l, node_r) {
        // Arguments by the caller's comparator
        (tree::Mixop::Arg, tree::Mixop::Arg) => {
            let order = compare_arg(*pos);
            *pos += 1;
            order
        }
        // Atoms by name
        (tree::Mixop::Atom(atom_l), tree::Mixop::Atom(atom_r)) => atom_l.node.cmp(&atom_r.node),
        // Brackets: opening atom, inner form, closing atom
        (
            tree::Mixop::Brack(atom_l_l, child_l, atom_l_r),
            tree::Mixop::Brack(atom_r_l, child_r, atom_r_r),
        ) => atom_l_l
            .node
            .cmp(&atom_r_l.node)
            .then_with(|| cmp_trees_by_inner(child_l, child_r, pos, compare_arg))
            .then_with(|| atom_l_r.node.cmp(&atom_r_r.node)),
        // Infix: left form, operator, right form
        (
            tree::Mixop::Infix(child_l_l, atom_l, child_l_r),
            tree::Mixop::Infix(child_r_l, atom_r, child_r_r),
        ) => cmp_trees_by_inner(child_l_l, child_r_l, pos, compare_arg)
            .then_with(|| atom_l.node.cmp(&atom_r.node))
            .then_with(|| cmp_trees_by_inner(child_l_r, child_r_r, pos, compare_arg)),
        // Sequences: common prefix first, then length
        (tree::Mixop::Seq(elems_l), tree::Mixop::Seq(elems_r)) => {
            let nodes_l = elems_l.iter();
            let nodes_r = elems_r.iter();
            for (node_l, node_r) in nodes_l.zip(nodes_r) {
                let order = cmp_trees_by_inner(node_l, node_r, pos, compare_arg);
                if order != Ordering::Equal {
                    return order;
                }
            }
            elems_l.len().cmp(&elems_r.len())
        }
        // Different forms order by variant
        _ => node_l.tag().cmp(&node_r.tag()),
    }
}

/// Orders two nodes by structure and atom names, lexicographically.
///
/// Brackets compare the opening atom, the inner form, then the closing atom;
/// infix compares the left form, the operator, then the right form;
/// sequences compare the common prefix, then the length;
/// different forms order by variant.
/// Both walks visit equal prefixes, so they reach argument positions in step,
/// and `compare_arg` orders the arguments at each position.
pub fn cmp_flats_by(
    arena_mixop_l: &MixopArena,
    node_l: &flat::MixopKind,
    arena_mixop_r: &MixopArena,
    node_r: &flat::MixopKind,
    mut compare_arg: impl FnMut(usize) -> Ordering,
) -> Ordering {
    let mut pos = 0;
    cmp_flats_by_inner(arena_mixop_l, node_l, arena_mixop_r, node_r, &mut pos, &mut compare_arg)
}

/// Structural comparison, threading the position and the argument comparator.
fn cmp_flats_by_inner(
    arena_mixop_l: &MixopArena,
    node_l: &flat::MixopKind,
    arena_mixop_r: &MixopArena,
    node_r: &flat::MixopKind,
    pos: &mut usize,
    compare_arg: &mut impl FnMut(usize) -> Ordering,
) -> Ordering {
    match (node_l, node_r) {
        // Arguments by the caller's comparator
        (flat::MixopKind::Arg, flat::MixopKind::Arg) => {
            let order = compare_arg(*pos);
            *pos += 1;
            order
        }
        // Atoms by name
        (flat::MixopKind::Atom(atom_l), flat::MixopKind::Atom(atom_r)) => {
            atom_l.node.cmp(&atom_r.node)
        }
        // Brackets: opening atom, inner form, closing atom
        (
            flat::MixopKind::Brack(atom_l_l, child_l, atom_l_r),
            flat::MixopKind::Brack(atom_r_l, child_r, atom_r_r),
        ) => atom_l_l
            .node
            .cmp(&atom_r_l.node)
            .then_with(|| {
                cmp_flats_by_inner(
                    arena_mixop_l,
                    arena_mixop_l.kind(*child_l),
                    arena_mixop_r,
                    arena_mixop_r.kind(*child_r),
                    pos,
                    compare_arg,
                )
            })
            .then_with(|| atom_l_r.node.cmp(&atom_r_r.node)),
        // Infix: left form, operator, right form
        (
            flat::MixopKind::Infix(child_l_l, atom_l, child_l_r),
            flat::MixopKind::Infix(child_r_l, atom_r, child_r_r),
        ) => cmp_flats_by_inner(
            arena_mixop_l,
            arena_mixop_l.kind(*child_l_l),
            arena_mixop_r,
            arena_mixop_r.kind(*child_r_l),
            pos,
            compare_arg,
        )
        .then_with(|| atom_l.node.cmp(&atom_r.node))
        .then_with(|| {
            cmp_flats_by_inner(
                arena_mixop_l,
                arena_mixop_l.kind(*child_l_r),
                arena_mixop_r,
                arena_mixop_r.kind(*child_r_r),
                pos,
                compare_arg,
            )
        }),
        // Sequences: common prefix first, then length
        (flat::MixopKind::Seq(elems_l), flat::MixopKind::Seq(elems_r)) => {
            let nodes_l = elems_l.iter().map(|elem| arena_mixop_l.kind(*elem));
            let nodes_r = elems_r.iter().map(|elem| arena_mixop_r.kind(*elem));
            for (node_l, node_r) in nodes_l.zip(nodes_r) {
                let order = cmp_flats_by_inner(
                    arena_mixop_l,
                    node_l,
                    arena_mixop_r,
                    node_r,
                    pos,
                    compare_arg,
                );
                if order != Ordering::Equal {
                    return order;
                }
            }
            elems_l.len().cmp(&elems_r.len())
        }
        // Different forms order by variant
        _ => node_l.tag().cmp(&node_r.tag()),
    }
}

/// A piece of a notation in reading order.
#[derive(Clone, Copy, Debug)]
pub enum Piece<'a> {
    /// A literal atom.
    Atom(&'a AtomPhrase),
    /// The argument position with this number.
    Arg(usize),
}

/// Visits atoms and argument positions in reading order.
pub fn visit_tree<'a>(node: &'a tree::Mixop, mut visit_piece: impl FnMut(Piece<'a>)) {
    let mut pos = 0;
    visit_tree_inner(node, &mut pos, &mut visit_piece);
}

/// Visits one node, threading the position.
fn visit_tree_inner<'a>(
    node: &'a tree::Mixop,
    pos: &mut usize,
    visit_piece: &mut impl FnMut(Piece<'a>),
) {
    match node {
        tree::Mixop::Arg => {
            visit_piece(Piece::Arg(*pos));
            *pos += 1;
        }
        tree::Mixop::Atom(atom) => visit_piece(Piece::Atom(atom)),
        tree::Mixop::Brack(atom_l, child, atom_r) => {
            visit_piece(Piece::Atom(atom_l));
            visit_tree_inner(child, pos, visit_piece);
            visit_piece(Piece::Atom(atom_r));
        }
        tree::Mixop::Infix(child_l, atom, child_r) => {
            visit_tree_inner(child_l, pos, visit_piece);
            visit_piece(Piece::Atom(atom));
            visit_tree_inner(child_r, pos, visit_piece);
        }
        tree::Mixop::Seq(elems) => {
            for elem in elems {
                visit_tree_inner(elem, pos, visit_piece);
            }
        }
    }
}

/// Visits atoms and argument positions in reading order.
pub fn visit_flat<'a>(
    arena_mixop: &'a MixopArena,
    node: &'a flat::MixopKind,
    mut visit_piece: impl FnMut(Piece<'a>),
) {
    let mut pos = 0;
    visit_flat_inner(arena_mixop, node, &mut pos, &mut visit_piece);
}

/// Visits one node, threading the position.
fn visit_flat_inner<'a>(
    arena_mixop: &'a MixopArena,
    node: &'a flat::MixopKind,
    pos: &mut usize,
    visit_piece: &mut impl FnMut(Piece<'a>),
) {
    match node {
        flat::MixopKind::Arg => {
            visit_piece(Piece::Arg(*pos));
            *pos += 1;
        }
        flat::MixopKind::Atom(atom) => visit_piece(Piece::Atom(atom)),
        flat::MixopKind::Brack(atom_l, child, atom_r) => {
            visit_piece(Piece::Atom(atom_l));
            visit_flat_inner(arena_mixop, arena_mixop.kind(*child), pos, visit_piece);
            visit_piece(Piece::Atom(atom_r));
        }
        flat::MixopKind::Infix(child_l, atom, child_r) => {
            visit_flat_inner(arena_mixop, arena_mixop.kind(*child_l), pos, visit_piece);
            visit_piece(Piece::Atom(atom));
            visit_flat_inner(arena_mixop, arena_mixop.kind(*child_r), pos, visit_piece);
        }
        flat::MixopKind::Seq(elems) => {
            for elem in elems {
                visit_flat_inner(arena_mixop, arena_mixop.kind(*elem), pos, visit_piece);
            }
        }
    }
}

/// Writes atoms and arguments, separating non-empty pieces with spaces.
///
/// Empty keyword atoms print nothing, not even a space;
/// `print_arg` writes the argument at a position.
pub fn print_tree_with(
    node: &tree::Mixop,
    printer: &mut Printer<'_>,
    mut print_arg: impl FnMut(usize, &mut Printer<'_>) -> fmt::Result,
) -> fmt::Result {
    let mut is_first = true;
    let mut result = Ok(());
    visit_tree(node, |piece| {
        if result.is_ok() {
            result = print_piece(piece, printer, &mut is_first, &mut print_arg);
        }
    });
    result
}

/// Writes atoms and arguments, separating non-empty pieces with spaces.
///
/// Empty keyword atoms print nothing, not even a space;
/// `print_arg` writes the argument at a position.
pub fn print_flat_with(
    arena_mixop: &MixopArena,
    node: &flat::MixopKind,
    printer: &mut Printer<'_>,
    mut print_arg: impl FnMut(usize, &mut Printer<'_>) -> fmt::Result,
) -> fmt::Result {
    let mut is_first = true;
    let mut result = Ok(());
    visit_flat(arena_mixop, node, |piece| {
        if result.is_ok() {
            result = print_piece(piece, printer, &mut is_first, &mut print_arg);
        }
    });
    result
}

/// Prints one piece using the same spacing rules for both representations.
fn print_piece(
    piece: Piece<'_>,
    printer: &mut Printer<'_>,
    is_first: &mut bool,
    print_arg: &mut impl FnMut(usize, &mut Printer<'_>) -> fmt::Result,
) -> fmt::Result {
    match piece {
        Piece::Atom(atom) if matches!(&atom.node, Atom::Keyword(keyword) if keyword.is_empty()) => {
            Ok(())
        }
        Piece::Atom(atom) => print_sep(printer, is_first).and_then(|()| atom.print(printer)),
        Piece::Arg(pos) => print_sep(printer, is_first).and_then(|()| print_arg(pos, printer)),
    }
}

/// Writes a space before every piece but the first.
fn print_sep(printer: &mut Printer<'_>, is_first: &mut bool) -> fmt::Result {
    if *is_first {
        *is_first = false;
        Ok(())
    } else {
        printer.write(" ")
    }
}

/// Rebuilds a node as a tree, copying atoms with their spans.
pub fn to_tree(arena_mixop: &MixopArena, node: &flat::MixopKind) -> tree::Mixop {
    match node {
        flat::MixopKind::Arg => tree::Mixop::Arg,
        flat::MixopKind::Atom(atom) => tree::Mixop::Atom(atom.clone()),
        flat::MixopKind::Brack(atom_l, child, atom_r) => tree::Mixop::Brack(
            atom_l.clone(),
            Box::new(to_tree(arena_mixop, arena_mixop.kind(*child))),
            atom_r.clone(),
        ),
        flat::MixopKind::Infix(child_l, atom, child_r) => tree::Mixop::Infix(
            Box::new(to_tree(arena_mixop, arena_mixop.kind(*child_l))),
            atom.clone(),
            Box::new(to_tree(arena_mixop, arena_mixop.kind(*child_r))),
        ),
        flat::MixopKind::Seq(elems) => tree::Mixop::Seq(
            elems
                .iter()
                .map(|elem| to_tree(arena_mixop, arena_mixop.kind(*elem)))
                .collect(),
        ),
    }
}

/// The number of argument positions.
pub fn arity_tree(node: &tree::Mixop) -> usize {
    match node {
        tree::Mixop::Arg => 1,
        tree::Mixop::Atom(_) => 0,
        tree::Mixop::Brack(_, child, _) => arity_tree(child),
        tree::Mixop::Infix(child_l, _, child_r) => arity_tree(child_l) + arity_tree(child_r),
        tree::Mixop::Seq(elems) => elems.iter().map(arity_tree).sum(),
    }
}

/// Compares a stored form with an owned tree, ignoring atom spans.
pub fn matches_tree(
    arena_mixop: &MixopArena,
    mixop_flat: &flat::MixopKind,
    mixop_tree: &tree::Mixop,
) -> bool {
    match (mixop_flat, mixop_tree) {
        (flat::MixopKind::Arg, tree::Mixop::Arg) => true,
        (flat::MixopKind::Atom(atom_l), tree::Mixop::Atom(atom_r)) => atom_l.node == atom_r.node,
        (
            flat::MixopKind::Brack(atom_l_l, child_l, atom_l_r),
            tree::Mixop::Brack(atom_r_l, child_r, atom_r_r),
        ) => {
            atom_l_l.node == atom_r_l.node
                && atom_l_r.node == atom_r_r.node
                && matches_tree(arena_mixop, arena_mixop.kind(*child_l), child_r)
        }
        (
            flat::MixopKind::Infix(child_l_l, atom_l, child_l_r),
            tree::Mixop::Infix(child_r_l, atom_r, child_r_r),
        ) => {
            atom_l.node == atom_r.node
                && matches_tree(arena_mixop, arena_mixop.kind(*child_l_l), child_r_l)
                && matches_tree(arena_mixop, arena_mixop.kind(*child_l_r), child_r_r)
        }
        (flat::MixopKind::Seq(children_l), tree::Mixop::Seq(children_r)) => {
            children_l.len() == children_r.len()
                && children_l.iter().zip(children_r).all(|(child_l, child_r)| {
                    matches_tree(arena_mixop, arena_mixop.kind(*child_l), child_r)
                })
        }
        _ => false,
    }
}
