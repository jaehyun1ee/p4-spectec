//! Traversals shared by every notation representation
//!
//! Each function takes a node with the shape arena its representation
//! reads children from (`()` for trees),
//! so nodes of two representations can be compared with each other.
//! Atoms compare by name only.
//! Argument positions are numbered left to right from 0,
//! and callers look up the argument at a position themselves;
//! `cmp_by` and `print_with` hand positions to a closure in notation order.

use std::{cmp::Ordering, fmt};

use crate::lang::{
    common::notation::atom::Atom,
    traits::print::{Print, Printer},
};

use super::{
    node::{AtomPhrase, Node, Repr},
    tree::Tree,
};

// = Comparison

/// Whether two nodes have the same structure and atom names.
///
/// Atom spans are not compared.
pub fn eq<R: Repr, S: Repr>(
    arena_shape_l: &R::ShapeArena,
    node_l: &Node<R>,
    arena_shape_r: &S::ShapeArena,
    node_r: &Node<S>,
) -> bool {
    cmp_by(arena_shape_l, node_l, arena_shape_r, node_r, |_| Ordering::Equal) == Ordering::Equal
}

/// Orders two nodes by structure and atom names, lexicographically.
///
/// Brackets compare the opening atom, the inner form, then the closing atom;
/// infix compares the left form, the operator, then the right form;
/// sequences compare the common prefix, then the length;
/// different forms order by variant.
/// Both walks visit equal prefixes, so they reach argument positions in step,
/// and `compare_arg` orders the arguments at each position.
pub fn cmp_by<R: Repr, S: Repr>(
    arena_shape_l: &R::ShapeArena,
    node_l: &Node<R>,
    arena_shape_r: &S::ShapeArena,
    node_r: &Node<S>,
    mut compare_arg: impl FnMut(usize) -> Ordering,
) -> Ordering {
    let mut pos = 0;
    cmp_by_inner(arena_shape_l, node_l, arena_shape_r, node_r, &mut pos, &mut compare_arg)
}

/// Structural comparison, threading the position and the argument comparator.
fn cmp_by_inner<R: Repr, S: Repr>(
    arena_shape_l: &R::ShapeArena,
    node_l: &Node<R>,
    arena_shape_r: &S::ShapeArena,
    node_r: &Node<S>,
    pos: &mut usize,
    compare_arg: &mut impl FnMut(usize) -> Ordering,
) -> Ordering {
    match (node_l, node_r) {
        // Arguments by the caller's comparator
        (Node::Arg, Node::Arg) => {
            let order = compare_arg(*pos);
            *pos += 1;
            order
        }
        // Atoms by name
        (Node::Atom(atom_l), Node::Atom(atom_r)) => atom_l.node.cmp(&atom_r.node),
        // Brackets: opening atom, inner form, closing atom
        (Node::Brack(atom_l_l, child_l, atom_l_r), Node::Brack(atom_r_l, child_r, atom_r_r)) => {
            atom_l_l
                .node
                .cmp(&atom_r_l.node)
                .then_with(|| {
                    cmp_by_inner(
                        arena_shape_l,
                        Node::<R>::child(arena_shape_l, child_l),
                        arena_shape_r,
                        Node::<S>::child(arena_shape_r, child_r),
                        pos,
                        compare_arg,
                    )
                })
                .then_with(|| atom_l_r.node.cmp(&atom_r_r.node))
        }
        // Infix: left form, operator, right form
        (Node::Infix(child_l_l, atom_l, child_l_r), Node::Infix(child_r_l, atom_r, child_r_r)) => {
            cmp_by_inner(
                arena_shape_l,
                Node::<R>::child(arena_shape_l, child_l_l),
                arena_shape_r,
                Node::<S>::child(arena_shape_r, child_r_l),
                pos,
                compare_arg,
            )
            .then_with(|| atom_l.node.cmp(&atom_r.node))
            .then_with(|| {
                cmp_by_inner(
                    arena_shape_l,
                    Node::<R>::child(arena_shape_l, child_l_r),
                    arena_shape_r,
                    Node::<S>::child(arena_shape_r, child_r_r),
                    pos,
                    compare_arg,
                )
            })
        }
        // Sequences: common prefix first, then length
        (Node::Seq(elems_l), Node::Seq(elems_r)) => {
            let nodes_l = elems_l.iter().map(|elem| R::node(arena_shape_l, elem));
            let nodes_r = elems_r.iter().map(|elem| S::node(arena_shape_r, elem));
            for (node_l, node_r) in nodes_l.zip(nodes_r) {
                let order =
                    cmp_by_inner(arena_shape_l, node_l, arena_shape_r, node_r, pos, compare_arg);
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

// = Visiting

/// A piece of a notation in reading order.
#[derive(Clone, Copy, Debug)]
pub enum Piece<'a> {
    /// A literal atom.
    Atom(&'a AtomPhrase),
    /// The argument position with this number.
    Arg(usize),
}

/// Visits atoms and argument positions in reading order.
pub fn visit<'a, R: Repr>(
    arena_shape: &'a R::ShapeArena,
    node: &'a Node<R>,
    mut visit_piece: impl FnMut(Piece<'a>),
) {
    let mut pos = 0;
    visit_inner(arena_shape, node, &mut pos, &mut visit_piece);
}

/// Visits one node, threading the position.
fn visit_inner<'a, R: Repr>(
    arena_shape: &'a R::ShapeArena,
    node: &'a Node<R>,
    pos: &mut usize,
    visit_piece: &mut impl FnMut(Piece<'a>),
) {
    match node {
        Node::Arg => {
            visit_piece(Piece::Arg(*pos));
            *pos += 1;
        }
        Node::Atom(atom) => visit_piece(Piece::Atom(atom)),
        Node::Brack(atom_l, child, atom_r) => {
            visit_piece(Piece::Atom(atom_l));
            visit_inner(arena_shape, Node::<R>::child(arena_shape, child), pos, visit_piece);
            visit_piece(Piece::Atom(atom_r));
        }
        Node::Infix(child_l, atom, child_r) => {
            visit_inner(arena_shape, Node::<R>::child(arena_shape, child_l), pos, visit_piece);
            visit_piece(Piece::Atom(atom));
            visit_inner(arena_shape, Node::<R>::child(arena_shape, child_r), pos, visit_piece);
        }
        Node::Seq(elems) => {
            for elem in elems {
                visit_inner(arena_shape, R::node(arena_shape, elem), pos, visit_piece);
            }
        }
    }
}

// = Printing

/// Writes atoms and arguments, separating non-empty pieces with spaces.
///
/// Empty keyword atoms print nothing, not even a space;
/// `print_arg` writes the argument at a position.
pub fn print_with<R: Repr>(
    arena_shape: &R::ShapeArena,
    node: &Node<R>,
    printer: &mut Printer<'_>,
    mut print_arg: impl FnMut(usize, &mut Printer<'_>) -> fmt::Result,
) -> fmt::Result {
    let mut is_first = true;
    let mut result = Ok(());
    visit(arena_shape, node, |piece| {
        // Stop writing after the first failure
        if result.is_err() {
            return;
        }
        result = match piece {
            // Empty keyword atoms print nothing, not even a space
            Piece::Atom(atom) if matches!(&atom.node, Atom::Keyword(keyword) if keyword.is_empty()) => {
                Ok(())
            }
            // A space before every piece but the first
            Piece::Atom(atom) => {
                print_sep(printer, &mut is_first).and_then(|()| atom.print(printer))
            }
            Piece::Arg(pos) => {
                print_sep(printer, &mut is_first).and_then(|()| print_arg(pos, printer))
            }
        };
    });
    result
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

// = Expansion

/// Rebuilds a node as a tree, copying atoms with their spans.
pub fn to_tree<R: Repr>(arena_shape: &R::ShapeArena, node: &Node<R>) -> Node<Tree> {
    match node {
        Node::Arg => Node::Arg,
        Node::Atom(atom) => Node::Atom(atom.clone()),
        Node::Brack(atom_l, child, atom_r) => Node::Brack(
            atom_l.clone(),
            Box::new(to_tree(arena_shape, Node::<R>::child(arena_shape, child))),
            atom_r.clone(),
        ),
        Node::Infix(child_l, atom, child_r) => Node::Infix(
            Box::new(to_tree(arena_shape, Node::<R>::child(arena_shape, child_l))),
            atom.clone(),
            Box::new(to_tree(arena_shape, Node::<R>::child(arena_shape, child_r))),
        ),
        Node::Seq(elems) => Node::Seq(
            elems
                .iter()
                .map(|elem| to_tree(arena_shape, R::node(arena_shape, elem)))
                .collect(),
        ),
    }
}

/// The number of argument positions.
pub fn arity<R: Repr>(arena_shape: &R::ShapeArena, node: &Node<R>) -> usize {
    match node {
        Node::Arg => 1,
        Node::Atom(_) => 0,
        Node::Brack(_, child, _) => arity(arena_shape, Node::<R>::child(arena_shape, child)),
        Node::Infix(child_l, _, child_r) => {
            arity(arena_shape, Node::<R>::child(arena_shape, child_l))
                + arity(arena_shape, Node::<R>::child(arena_shape, child_r))
        }
        Node::Seq(elems) => elems
            .iter()
            .map(|elem| arity(arena_shape, R::node(arena_shape, elem)))
            .sum(),
    }
}
