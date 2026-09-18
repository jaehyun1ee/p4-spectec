//! Source variables and their prepared slot references

use std::fmt;

use super::typ::Typ;
use crate::lang::{
    common::{Id, Iter, ds::set::IdSet},
    traits::{
        eq::SyntaxEq,
        free::Free,
        print::{Print, Printer},
    },
};

// == Source variables

#[derive(Clone, Debug, PartialEq)]
pub struct Var {
    pub id: Id,
    pub typ: Typ,
    pub iters: Vec<Iter>,
}

impl Print for Var {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        self.id.print(printer)?;
        for iter in &self.iters {
            iter.print(printer)?;
        }
        Ok(())
    }
}

impl SyntaxEq for Var {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id) && self.iters == other.iters
    }

    fn slice_syntax_eq(vars_l: &[Self], vars_r: &[Self]) -> bool {
        let mut vars_l = vars_l.iter().collect::<Vec<_>>();
        let mut vars_r = vars_r.iter().collect::<Vec<_>>();
        let cmp_var = |var_l: &&Self, var_r: &&Self| {
            var_l
                .id
                .node
                .cmp(&var_r.id.node)
                .then_with(|| var_l.iters.cmp(&var_r.iters))
        };
        vars_l.sort_by(cmp_var);
        vars_r.sort_by(cmp_var);
        vars_l.len() == vars_r.len()
            && vars_l
                .into_iter()
                .zip(vars_r)
                .all(|(var_l, var_r)| var_l.syntax_eq(var_r))
    }
}

impl Free for Var {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

// == Slot variables

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlotIdx(pub(crate) usize);

/// Prepared identifier occurrence, always addressing the empty iterator path
#[derive(Clone, Debug)]
pub struct IdSlot {
    pub id: Id,
    pub slot: SlotIdx,
}

impl PartialEq for IdSlot {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl SyntaxEq for IdSlot {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id)
    }
}

impl Print for IdSlot {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        self.id.print(printer)
    }
}

#[derive(Clone, Debug)]
pub struct VarSlot {
    pub slot: SlotIdx,
    pub var: Var,
}

impl PartialEq for VarSlot {
    fn eq(&self, slot_other: &Self) -> bool {
        self.var.id.node == slot_other.var.id.node && self.var.iters == slot_other.var.iters
    }
}

impl SyntaxEq for VarSlot {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.var.syntax_eq(&other.var)
    }

    fn slice_syntax_eq(vars_l: &[Self], vars_r: &[Self]) -> bool {
        let mut vars_l = vars_l.iter().collect::<Vec<_>>();
        let mut vars_r = vars_r.iter().collect::<Vec<_>>();
        let cmp_var = |var_l: &&Self, var_r: &&Self| {
            var_l
                .var
                .id
                .node
                .cmp(&var_r.var.id.node)
                .then_with(|| var_l.var.iters.cmp(&var_r.var.iters))
        };
        vars_l.sort_by(cmp_var);
        vars_r.sort_by(cmp_var);
        vars_l.len() == vars_r.len()
            && vars_l
                .into_iter()
                .zip(vars_r)
                .all(|(var_l, var_r)| var_l.syntax_eq(var_r))
    }
}

impl Print for VarSlot {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        self.var.print(printer)
    }
}
