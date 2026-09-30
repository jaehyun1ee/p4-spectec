//! Traversals shared by every notation representation
//!
//! Each function takes a node with the context its representation expands
//! children in, so a tree and a shape can be compared with each other:
//! `eq_by(shapes, shape_kind, &(), mixfix, ..)`.
//! Atoms compare by name only; arguments go to the caller's closure
//! in notation order, left to right.
//! `print_with` separates pieces with spaces and drops empty keyword atoms;
//! `to_tree` rebuilds a `Mixfix`; `arity` counts argument positions.

use std::{cmp::Ordering, fmt};

use crate::lang::{
    common::notation::atom::Atom,
    traits::print::{Print, Printer},
};

use super::{
    node::{AtomPhrase, Expand, Node},
    tree::Mixfix,
};

// = Comparison

/// Whether two nodes have the same structure, atom names, and arguments.
///
/// Atom spans are not compared; `eq_arg` decides each pair of arguments.
pub fn eq_by<A, B, R: Expand<A>, S: Expand<B>>(
    ctx_l: &R::Ctx,
    node_l: &Node<A, R>,
    ctx_r: &S::Ctx,
    node_r: &Node<B, S>,
    mut eq_arg: impl FnMut(&A, &B) -> bool,
) -> bool {
    eq_by_inner(ctx_l, node_l, ctx_r, node_r, &mut eq_arg)
}

/// Structural equality, threading the argument predicate.
fn eq_by_inner<A, B, R: Expand<A>, S: Expand<B>>(
    ctx_l: &R::Ctx,
    node_l: &Node<A, R>,
    ctx_r: &S::Ctx,
    node_r: &Node<B, S>,
    eq_arg: &mut impl FnMut(&A, &B) -> bool,
) -> bool {
    match (node_l, node_r) {
        // Arguments by the caller's predicate
        (Node::Arg(arg_l), Node::Arg(arg_r)) => eq_arg(arg_l, arg_r),
        // Atoms by name
        (Node::Atom(atom_l), Node::Atom(atom_r)) => atom_l.node == atom_r.node,
        // Brackets: opening atom, inner form, closing atom
        (Node::Brack(atom_l_l, child_l, atom_l_r), Node::Brack(atom_r_l, child_r, atom_r_r)) => {
            atom_l_l.node == atom_r_l.node
                && eq_by_inner(
                    ctx_l,
                    R::child(ctx_l, child_l),
                    ctx_r,
                    S::child(ctx_r, child_r),
                    eq_arg,
                )
                && atom_l_r.node == atom_r_r.node
        }
        // Infix: left form, operator, right form
        (Node::Infix(child_l_l, atom_l, child_l_r), Node::Infix(child_r_l, atom_r, child_r_r)) => {
            eq_by_inner(
                ctx_l,
                R::child(ctx_l, child_l_l),
                ctx_r,
                S::child(ctx_r, child_r_l),
                eq_arg,
            ) && atom_l.node == atom_r.node
                && eq_by_inner(
                    ctx_l,
                    R::child(ctx_l, child_l_r),
                    ctx_r,
                    S::child(ctx_r, child_r_r),
                    eq_arg,
                )
        }
        // Sequences elementwise at equal length
        (Node::Seq(children_l), Node::Seq(children_r)) => {
            let nodes_l = R::children(ctx_l, children_l);
            let nodes_r = S::children(ctx_r, children_r);
            nodes_l.len() == nodes_r.len()
                && nodes_l
                    .zip(nodes_r)
                    .all(|(node_l, node_r)| eq_by_inner(ctx_l, node_l, ctx_r, node_r, eq_arg))
        }
        // Different forms
        _ => false,
    }
}

/// Orders two nodes by structure and atom names, lexicographically.
///
/// Brackets compare the opening atom, the inner form, then the closing atom;
/// infix compares the left form, the operator, then the right form;
/// sequences compare the common prefix, then the length;
/// different forms order by variant; `compare_arg` orders arguments.
pub fn cmp_by<A, B, R: Expand<A>, S: Expand<B>>(
    ctx_l: &R::Ctx,
    node_l: &Node<A, R>,
    ctx_r: &S::Ctx,
    node_r: &Node<B, S>,
    mut compare_arg: impl FnMut(&A, &B) -> Ordering,
) -> Ordering {
    cmp_by_inner(ctx_l, node_l, ctx_r, node_r, &mut compare_arg)
}

/// Structural comparison, threading the argument comparator.
fn cmp_by_inner<A, B, R: Expand<A>, S: Expand<B>>(
    ctx_l: &R::Ctx,
    node_l: &Node<A, R>,
    ctx_r: &S::Ctx,
    node_r: &Node<B, S>,
    compare_arg: &mut impl FnMut(&A, &B) -> Ordering,
) -> Ordering {
    match (node_l, node_r) {
        // Arguments by the caller's comparator
        (Node::Arg(arg_l), Node::Arg(arg_r)) => compare_arg(arg_l, arg_r),
        // Atoms by name
        (Node::Atom(atom_l), Node::Atom(atom_r)) => atom_l.node.cmp(&atom_r.node),
        // Brackets: opening atom, inner form, closing atom
        (Node::Brack(atom_l_l, child_l, atom_l_r), Node::Brack(atom_r_l, child_r, atom_r_r)) => {
            atom_l_l
                .node
                .cmp(&atom_r_l.node)
                .then_with(|| {
                    cmp_by_inner(
                        ctx_l,
                        R::child(ctx_l, child_l),
                        ctx_r,
                        S::child(ctx_r, child_r),
                        compare_arg,
                    )
                })
                .then_with(|| atom_l_r.node.cmp(&atom_r_r.node))
        }
        // Infix: left form, operator, right form
        (Node::Infix(child_l_l, atom_l, child_l_r), Node::Infix(child_r_l, atom_r, child_r_r)) => {
            cmp_by_inner(
                ctx_l,
                R::child(ctx_l, child_l_l),
                ctx_r,
                S::child(ctx_r, child_r_l),
                compare_arg,
            )
            .then_with(|| atom_l.node.cmp(&atom_r.node))
            .then_with(|| {
                cmp_by_inner(
                    ctx_l,
                    R::child(ctx_l, child_l_r),
                    ctx_r,
                    S::child(ctx_r, child_r_r),
                    compare_arg,
                )
            })
        }
        // Sequences: common prefix first, then length
        (Node::Seq(children_l), Node::Seq(children_r)) => {
            let nodes_l = R::children(ctx_l, children_l);
            let nodes_r = S::children(ctx_r, children_r);
            let (len_l, len_r) = (nodes_l.len(), nodes_r.len());
            for (node_l, node_r) in nodes_l.zip(nodes_r) {
                let order = cmp_by_inner(ctx_l, node_l, ctx_r, node_r, compare_arg);
                if order != Ordering::Equal {
                    return order;
                }
            }
            len_l.cmp(&len_r)
        }
        // Different forms order by variant
        _ => node_l.tag().cmp(&node_r.tag()),
    }
}

// = Printing

/// Writes atoms and arguments, separating non-empty pieces with spaces.
///
/// Empty keyword atoms print nothing, not even a space.
pub fn print_with<A, R: Expand<A>>(
    ctx: &R::Ctx,
    node: &Node<A, R>,
    printer: &mut Printer<'_>,
    mut print_arg: impl FnMut(&A, &mut Printer<'_>) -> fmt::Result,
) -> fmt::Result {
    let mut is_first = true;
    print_with_inner(ctx, node, printer, &mut print_arg, &mut is_first)
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

/// Writes an atom, or nothing for an empty keyword.
fn print_atom(atom: &AtomPhrase, printer: &mut Printer<'_>, is_first: &mut bool) -> fmt::Result {
    if matches!(&atom.node, Atom::Keyword(keyword) if keyword.is_empty()) {
        Ok(())
    } else {
        print_sep(printer, is_first)?;
        atom.print(printer)
    }
}

/// Prints one node, tracking whether a separator is due.
fn print_with_inner<A, R: Expand<A>>(
    ctx: &R::Ctx,
    node: &Node<A, R>,
    printer: &mut Printer<'_>,
    print_arg: &mut impl FnMut(&A, &mut Printer<'_>) -> fmt::Result,
    is_first: &mut bool,
) -> fmt::Result {
    match node {
        // An argument after its separator
        Node::Arg(arg) => {
            print_sep(printer, is_first)?;
            print_arg(arg, printer)
        }
        // A lone atom
        Node::Atom(atom) => print_atom(atom, printer, is_first),
        // Brackets around the inner form
        Node::Brack(atom_l, child, atom_r) => {
            print_atom(atom_l, printer, is_first)?;
            print_with_inner(ctx, R::child(ctx, child), printer, print_arg, is_first)?;
            print_atom(atom_r, printer, is_first)
        }
        // Left form, operator, right form
        Node::Infix(child_l, atom, child_r) => {
            print_with_inner(ctx, R::child(ctx, child_l), printer, print_arg, is_first)?;
            print_atom(atom, printer, is_first)?;
            print_with_inner(ctx, R::child(ctx, child_r), printer, print_arg, is_first)
        }
        // Forms in order
        Node::Seq(children) => {
            for node in R::children(ctx, children) {
                print_with_inner(ctx, node, printer, print_arg, is_first)?;
            }
            Ok(())
        }
    }
}

// = Expansion

/// Rebuilds a node as a tree, mapping arguments in notation order.
///
/// Atoms are copied with their spans.
pub fn to_tree<A, T, R: Expand<A>>(
    ctx: &R::Ctx,
    node: &Node<A, R>,
    mut map_arg: impl FnMut(&A) -> T,
) -> Mixfix<T> {
    to_tree_inner(ctx, node, &mut map_arg)
}

/// Rebuilds one node, threading the argument map.
fn to_tree_inner<A, T, R: Expand<A>>(
    ctx: &R::Ctx,
    node: &Node<A, R>,
    map_arg: &mut impl FnMut(&A) -> T,
) -> Mixfix<T> {
    match node {
        Node::Arg(arg) => Mixfix::Arg(map_arg(arg)),
        Node::Atom(atom) => Mixfix::Atom(atom.clone()),
        Node::Brack(atom_l, child, atom_r) => Mixfix::Brack(
            atom_l.clone(),
            Box::new(to_tree_inner(ctx, R::child(ctx, child), map_arg)),
            atom_r.clone(),
        ),
        Node::Infix(child_l, atom, child_r) => {
            let mixfix_l = to_tree_inner(ctx, R::child(ctx, child_l), map_arg);
            let mixfix_r = to_tree_inner(ctx, R::child(ctx, child_r), map_arg);
            Mixfix::Infix(Box::new(mixfix_l), atom.clone(), Box::new(mixfix_r))
        }
        Node::Seq(children) => Mixfix::Seq(
            R::children(ctx, children)
                .map(|node| to_tree_inner(ctx, node, map_arg))
                .collect(),
        ),
    }
}

/// The number of argument positions.
pub fn arity<A, R: Expand<A>>(ctx: &R::Ctx, node: &Node<A, R>) -> usize {
    match node {
        Node::Arg(_) => 1,
        Node::Atom(_) => 0,
        Node::Brack(_, child, _) => arity(ctx, R::child(ctx, child)),
        Node::Infix(child_l, _, child_r) => {
            arity(ctx, R::child(ctx, child_l)) + arity(ctx, R::child(ctx, child_r))
        }
        Node::Seq(children) => R::children(ctx, children)
            .map(|node| arity(ctx, node))
            .sum(),
    }
}
