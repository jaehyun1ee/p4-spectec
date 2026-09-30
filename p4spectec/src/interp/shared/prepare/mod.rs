//! Prepare source syntax for slot-based execution
//!
//! `Prepare` walks IL syntax once
//! and replaces identifiers and variables by frame slots (`IdSlot`, `VarSlot`),
//! so evaluation indexes a frame instead of looking names up.
//! It also interns each notation expression's notation into the
//! specification's `ShapeArena` (`MixopShape`),
//! so evaluation builds and matches case values by shape handle;
//! containers, phrases, and PL notation recurse structurally.

pub mod ast;

use crate::lang::{
    common::{Id, notation::mixfix::Mixfix, source::NotePhrase},
    data::{
        shape::ShapeArena,
        var::{IdSlot, Var, VarSlot},
    },
};

use crate::runtime::envs::interp::shared::frame::FrameLayout;

/// What preparing one callable writes to.
///
/// The layout belongs to the callable;
/// the shapes belong to the whole specification and outlive preparation,
/// since a runner's arena keeps them for the prepared notations to refer to.
pub struct PrepareContext<'a> {
    /// Slot layout of the callable being prepared.
    pub layout: &'a mut FrameLayout,
    /// Shapes of the specification's notations.
    pub shapes: &'a mut ShapeArena,
}

/// Slot resolution of one syntax node.
pub trait Prepare: Sized {
    /// The same node with identifiers resolved to slots.
    type Output;

    /// Resolves the node's identifiers and interns its notations in `ctx`.
    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output;
}

impl Prepare for Id {
    type Output = IdSlot;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        ctx.layout.resolve_id(self)
    }
}

impl Prepare for Var {
    type Output = VarSlot;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        ctx.layout.resolve_var(self)
    }
}

// - Containers

impl<T: Prepare> Prepare for Vec<T> {
    type Output = Vec<T::Output>;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        self.into_iter().map(|syntax| syntax.prepare(ctx)).collect()
    }
}

impl<T: Prepare> Prepare for Box<T> {
    type Output = Box<T::Output>;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        Box::new((*self).prepare(ctx))
    }
}

impl<T: Prepare> Prepare for Option<T> {
    type Output = Option<T::Output>;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        self.map(|syntax| syntax.prepare(ctx))
    }
}

// - Phrases

impl<T: Prepare, N, S> Prepare for NotePhrase<T, N, S> {
    type Output = NotePhrase<T::Output, N, S>;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        // Grow the stack for deep syntax trees
        stacker::maybe_grow(64 * 1024, 1024 * 1024, || NotePhrase {
            node: self.node.prepare(ctx),
            note: self.note,
            span: self.span,
        })
    }
}

// - Notation

impl<T: Prepare> Prepare for Mixfix<T> {
    type Output = Mixfix<T::Output>;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        match self {
            Mixfix::Arg(arg_inner) => Mixfix::Arg(arg_inner.prepare(ctx)),
            Mixfix::Atom(atom_inner) => Mixfix::Atom(atom_inner),
            Mixfix::Brack(atom_l, mixfix_inner, atom_r) => {
                Mixfix::Brack(atom_l, mixfix_inner.prepare(ctx), atom_r)
            }
            Mixfix::Infix(mixfix_l, atom_inner, mixfix_r) => {
                Mixfix::Infix(mixfix_l.prepare(ctx), atom_inner, mixfix_r.prepare(ctx))
            }
            Mixfix::Seq(mixfixes) => Mixfix::Seq(mixfixes.prepare(ctx)),
        }
    }
}
