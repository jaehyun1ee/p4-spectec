//! Capture-avoiding identifier renaming for OL instructions
//!
//! With `x -> y`, `let y = z { return x }` becomes
//! `let y' = z { return y }`, assuming `y'` is fresh
//! The local binder is renamed so it does not capture the introduced `y`

use super::super::{StructureError, StructureErrorKind, ol::ast as ol};
use crate::lang::{
    common::{
        ds::{map::IdMap, set::IdSet},
        notation::mixop::Mixop,
        source::Span,
    },
    hints::input,
    il::{ast::*, fresh},
    traits::free::Free,
};
use crate::{note_phrase, phrase};

// == Environment

#[derive(Clone, Debug, Default)]
pub(crate) struct Renamer {
    ids: IdMap<Id>,
}

impl Renamer {
    pub(crate) fn empty() -> Self {
        Self::default()
    }

    pub(crate) fn dom(&self) -> IdSet {
        self.ids.domain()
    }

    pub(crate) fn values(&self) -> Vec<Id> {
        self.ids.iter().map(|(_, id)| id.clone()).collect()
    }

    pub(crate) fn singleton(id: Id, id_renamed: Id) -> Self {
        let mut renamer = Self::empty();
        renamer.add(id, id_renamed);
        renamer
    }

    pub(crate) fn add(&mut self, id: Id, id_renamed: Id) {
        self.ids.insert(id, id_renamed);
    }

    pub(crate) fn filter(&self, mut predicate: impl FnMut(&Id, &Id) -> bool) -> Self {
        Self {
            ids: self
                .ids
                .iter()
                .filter(|(id, id_renamed)| predicate(id, id_renamed))
                .map(|(id, id_renamed)| (id.clone(), id_renamed.clone()))
                .collect(),
        }
    }

    // == Capture avoidance

    /// Builds fresh names for binders in `frees` that collide with rename targets
    ///
    /// ```text
    /// Rename x -> y:
    ///   before: let y = w { return (x, y) }
    ///   after:  let y' = w { return (y, y') }
    /// ```
    ///
    /// Returns `y -> y'`, assuming `y'` is fresh; the caller applies this map
    /// to the binder and its uses along with the original renaming
    /// Fresh names avoid the binders, the block's free names, both sides of
    /// this renamer, and names already chosen in this call
    pub(crate) fn freshen_binders(&self, frees: &IdSet, block: &ol::Block) -> Self {
        let ids_collide: IdSet = self
            .values()
            .into_iter()
            .filter(|id| frees.contains(id))
            .collect();
        let mut ids_avoid = frees
            .clone()
            .union(block.free())
            .union(self.dom())
            .union(self.values().into_iter().collect());
        let mut renamer_fresh = Self::empty();
        for id in ids_collide.iter() {
            let id_fresh = fresh::id(&ids_avoid, id);
            renamer_fresh.add(id.clone(), id_fresh.clone());
            ids_avoid.insert(id_fresh);
        }
        renamer_fresh
    }

    // == Variables

    fn rename_vars(&self, vars: Vec<Var>) -> Vec<Var> {
        vars.into_iter()
            .map(|mut var| {
                if let Some(id) = self.ids.get(&var.id) {
                    var.id = id.clone();
                }
                var
            })
            .collect()
    }

    // == Expressions

    pub(crate) fn rename_exp(&self, exp: Exp) -> Exp {
        let exp_kind = match exp.node {
            ExpKind::Bool(_) | ExpKind::Num(_) | ExpKind::Text(_) => exp.node,
            ExpKind::Var(id) => ExpKind::Var(self.ids.get(&id).cloned().unwrap_or(id)),
            ExpKind::Un(op, op_typ, exp) => {
                ExpKind::Un(op, op_typ, Box::new(self.rename_exp(*exp)))
            }
            ExpKind::Bin(op, op_typ, exp_l, exp_r) => ExpKind::Bin(
                op,
                op_typ,
                Box::new(self.rename_exp(*exp_l)),
                Box::new(self.rename_exp(*exp_r)),
            ),
            ExpKind::Cmp(op, op_typ, exp_l, exp_r) => ExpKind::Cmp(
                op,
                op_typ,
                Box::new(self.rename_exp(*exp_l)),
                Box::new(self.rename_exp(*exp_r)),
            ),
            ExpKind::UpCast(typ, exp) => ExpKind::UpCast(typ, Box::new(self.rename_exp(*exp))),
            ExpKind::DownCast(typ, exp) => ExpKind::DownCast(typ, Box::new(self.rename_exp(*exp))),
            ExpKind::Sub(exp, typ, subcheck) => {
                ExpKind::Sub(Box::new(self.rename_exp(*exp)), typ, subcheck)
            }
            ExpKind::Match(exp, pattern) => {
                ExpKind::Match(Box::new(self.rename_exp(*exp)), pattern)
            }
            ExpKind::Tuple(exps) => ExpKind::Tuple(self.rename_exps(exps)),
            ExpKind::Case(not_exp) => {
                ExpKind::Case(Box::new(not_exp.map(|exp| self.rename_exp(exp.clone()))))
            }
            ExpKind::Str(exp_fields) => ExpKind::Str(
                exp_fields
                    .into_iter()
                    .map(|(atom, exp)| (atom, self.rename_exp(exp)))
                    .collect(),
            ),
            ExpKind::Opt(exp) => ExpKind::Opt(exp.map(|exp| Box::new(self.rename_exp(*exp)))),
            ExpKind::List(exps) => ExpKind::List(self.rename_exps(exps)),
            ExpKind::Cons(exp_head, exp_tail) => ExpKind::Cons(
                Box::new(self.rename_exp(*exp_head)),
                Box::new(self.rename_exp(*exp_tail)),
            ),
            ExpKind::Cat(exp_l, exp_r) => ExpKind::Cat(
                Box::new(self.rename_exp(*exp_l)),
                Box::new(self.rename_exp(*exp_r)),
            ),
            ExpKind::Mem(exp_elem, exp_set) => ExpKind::Mem(
                Box::new(self.rename_exp(*exp_elem)),
                Box::new(self.rename_exp(*exp_set)),
            ),
            ExpKind::Len(exp) => ExpKind::Len(Box::new(self.rename_exp(*exp))),
            ExpKind::Dot(exp, atom) => ExpKind::Dot(Box::new(self.rename_exp(*exp)), atom),
            ExpKind::Idx(exp_base, exp_idx) => ExpKind::Idx(
                Box::new(self.rename_exp(*exp_base)),
                Box::new(self.rename_exp(*exp_idx)),
            ),
            ExpKind::Slice(exp_base, exp_idx, exp_len) => ExpKind::Slice(
                Box::new(self.rename_exp(*exp_base)),
                Box::new(self.rename_exp(*exp_idx)),
                Box::new(self.rename_exp(*exp_len)),
            ),
            ExpKind::Upd(exp_base, path, exp_field) => ExpKind::Upd(
                Box::new(self.rename_exp(*exp_base)),
                Box::new(self.rename_path(*path)),
                Box::new(self.rename_exp(*exp_field)),
            ),
            ExpKind::Call(id, targs, args) => ExpKind::Call(id, targs, self.rename_args(args)),
            ExpKind::Iter(exp, iter_exp) => ExpKind::Iter(
                Box::new(self.rename_exp(*exp)),
                self.rename_iterexp(iter_exp),
            ),
        };
        note_phrase!(node: exp_kind, note: exp.note, span: exp.span)
    }

    pub(crate) fn rename_exps(&self, exps: Vec<Exp>) -> Vec<Exp> {
        exps.into_iter().map(|exp| self.rename_exp(exp)).collect()
    }

    // == Expression iterators

    pub(crate) fn rename_iterexp(&self, iter_exp: ExpIter) -> ExpIter {
        let (iter, vars) = iter_exp;
        (iter, self.rename_vars(vars))
    }

    pub(crate) fn rename_iterexps(&self, iter_exps: Vec<ExpIter>) -> Vec<ExpIter> {
        iter_exps
            .into_iter()
            .map(|iter_exp| self.rename_iterexp(iter_exp))
            .collect()
    }

    // == Paths

    pub(crate) fn rename_path(&self, path: Path) -> Path {
        let path_kind = match path.node {
            PathKind::Root => PathKind::Root,
            PathKind::Idx(path, exp) => PathKind::Idx(
                Box::new(self.rename_path(*path)),
                Box::new(self.rename_exp(*exp)),
            ),
            PathKind::Slice(path, exp_idx, exp_len) => PathKind::Slice(
                Box::new(self.rename_path(*path)),
                Box::new(self.rename_exp(*exp_idx)),
                Box::new(self.rename_exp(*exp_len)),
            ),
            PathKind::Dot(path, atom) => PathKind::Dot(Box::new(self.rename_path(*path)), atom),
        };
        note_phrase!(node: path_kind, note: path.note, span: path.span)
    }

    // == Arguments

    pub(crate) fn rename_arg(&self, arg: Arg) -> Arg {
        let arg_kind = match arg.node {
            ArgKind::Exp(exp) => ArgKind::Exp(Box::new(self.rename_exp(*exp))),
            ArgKind::Def(_) => arg.node,
        };
        phrase!(node: arg_kind, span: arg.span)
    }

    pub(crate) fn rename_args(&self, args: Vec<Arg>) -> Vec<Arg> {
        args.into_iter().map(|arg| self.rename_arg(arg)).collect()
    }

    // == Cases

    pub(crate) fn rename_case(&self, case: ol::Case) -> Result<ol::Case, StructureError> {
        let ol::Case { guard, block } = case;
        let guard = self.rename_guard(guard);
        let block = self.rename_block(block)?;
        Ok(ol::Case { guard, block })
    }

    pub(crate) fn rename_cases(
        &self,
        cases: Vec<ol::Case>,
    ) -> Result<Vec<ol::Case>, StructureError> {
        cases
            .into_iter()
            .map(|case| self.rename_case(case))
            .collect()
    }

    // - Guards

    pub(crate) fn rename_guard(&self, guard: ol::Guard) -> ol::Guard {
        match guard {
            ol::Guard::Bool(_) | ol::Guard::Sub(..) | ol::Guard::Match(_) => guard,
            ol::Guard::Cmp(op, op_typ, exp) => ol::Guard::Cmp(op, op_typ, self.rename_exp(exp)),
            ol::Guard::Mem(exp) => ol::Guard::Mem(self.rename_exp(exp)),
        }
    }

    // == Instructions

    pub(crate) fn rename_instr(&self, instr_ol: ol::Instr) -> Result<ol::Instr, StructureError> {
        let instr_kind_ol = self.rename_instr_kind(instr_ol.node, &instr_ol.span)?;
        Ok(phrase!(node: instr_kind_ol, span: instr_ol.span))
    }

    fn rename_instr_kind(
        &self,
        instr_kind_ol: ol::InstrKind,
        span: &Span,
    ) -> Result<ol::InstrKind, StructureError> {
        match instr_kind_ol {
            ol::InstrKind::If(instr_ol) => self.rename_if_instr(instr_ol),
            ol::InstrKind::Hold(instr_ol) => self.rename_hold_instr(instr_ol),
            ol::InstrKind::Case(instr_ol) => self.rename_case_instr(instr_ol),
            ol::InstrKind::Group(instr_ol) => self.rename_group_instr(instr_ol),
            ol::InstrKind::Let(instr_ol) => self.rename_let_instr(instr_ol),
            ol::InstrKind::Rule(instr_ol) => self.rename_rule_instr(instr_ol, span),
            ol::InstrKind::Result(instr_ol) => Ok(self.rename_result_instr(instr_ol)),
            ol::InstrKind::Return(instr_ol) => Ok(self.rename_return_instr(instr_ol)),
            ol::InstrKind::Debug(instr_ol) => self.rename_debug_instr(instr_ol),
        }
    }

    pub(crate) fn rename_instrs(
        &self,
        instrs_ol: Vec<ol::Instr>,
    ) -> Result<Vec<ol::Instr>, StructureError> {
        instrs_ol
            .into_iter()
            .map(|instr_ol| self.rename_instr(instr_ol))
            .collect()
    }

    // - If instruction

    fn rename_if_instr(&self, instr_ol: ol::IfInstr) -> Result<ol::InstrKind, StructureError> {
        let ol::IfInstr {
            exp,
            iter_exps,
            block,
        } = instr_ol;
        let exp = self.rename_exp(exp);
        let iter_exps = self.rename_iterexps(iter_exps);
        let block = self.rename_block(block)?;
        Ok(ol::InstrKind::If(ol::IfInstr {
            exp,
            iter_exps,
            block,
        }))
    }

    // - Hold instruction

    fn rename_hold_instr(&self, instr_ol: ol::HoldInstr) -> Result<ol::InstrKind, StructureError> {
        let ol::HoldInstr {
            id,
            not_exp,
            iter_exps,
            block_hold,
            block_not_hold,
        } = instr_ol;
        let not_exp = not_exp.map(|exp| self.rename_exp(exp.clone()));
        let iter_exps = self.rename_iterexps(iter_exps);
        let block_hold = self.rename_block(block_hold)?;
        let block_not_hold = self.rename_block(block_not_hold)?;
        Ok(ol::InstrKind::Hold(ol::HoldInstr {
            id,
            not_exp,
            iter_exps,
            block_hold,
            block_not_hold,
        }))
    }

    // - Case instruction

    fn rename_case_instr(&self, instr_ol: ol::CaseInstr) -> Result<ol::InstrKind, StructureError> {
        let ol::CaseInstr { exp, cases, total } = instr_ol;
        let exp = self.rename_exp(exp);
        let cases = self.rename_cases(cases)?;
        Ok(ol::InstrKind::Case(ol::CaseInstr { exp, cases, total }))
    }

    // - Group instruction

    fn rename_group_instr(
        &self,
        instr_ol: ol::GroupInstr,
    ) -> Result<ol::InstrKind, StructureError> {
        let ol::GroupInstr {
            id,
            rel_signature,
            exps,
            block,
        } = instr_ol;
        let exps = self.rename_exps(exps);
        let block = self.rename_block(block)?;
        Ok(ol::InstrKind::Group(ol::GroupInstr {
            id,
            rel_signature,
            exps,
            block,
        }))
    }

    // - Let instruction

    fn rename_let_instr(&self, instr_ol: ol::LetInstr) -> Result<ol::InstrKind, StructureError> {
        let ol::LetInstr {
            exp_l,
            exp_r,
            iter_instrs,
            block,
        } = instr_ol;
        let exp_r = self.rename_exp(exp_r);
        let frees_l = exp_l.free();
        let mut renamer = self.filter(|id, _| !frees_l.contains(id));
        let renamer_fresh = renamer.freshen_binders(&frees_l, &block);
        let exp_l = renamer_fresh.rename_exp(exp_l);
        renamer.ids.extend(
            renamer_fresh
                .ids
                .iter()
                .map(|(id, id_fresh)| (id.clone(), id_fresh.clone())),
        );
        let iter_instrs = renamer.rename_iterinstrs_bound(iter_instrs);
        let block = renamer.rename_block(block)?;
        Ok(ol::InstrKind::Let(ol::LetInstr {
            exp_l,
            exp_r,
            iter_instrs,
            block,
        }))
    }

    // - Rule instruction

    fn rename_rule_instr(
        &self,
        instr_ol: ol::RuleInstr,
        span: &Span,
    ) -> Result<ol::InstrKind, StructureError> {
        let ol::RuleInstr {
            id,
            not_exp,
            input_hint,
            iter_instrs,
            block,
        } = instr_ol;
        let exps = not_exp.args().into_iter().cloned().collect();
        let (exps_input, exps_output) = input::split(&input_hint, exps)
            .map_err(|error| StructureError::new(StructureErrorKind::Input(error), span.clone()))?;
        let exps_input = self.rename_exps(exps_input);
        let frees_output = exps_output.as_slice().free();
        let mut renamer = self.filter(|id, _| !frees_output.contains(id));
        let renamer_fresh = renamer.freshen_binders(&frees_output, &block);
        let exps_output = renamer_fresh.rename_exps(exps_output);
        renamer.ids.extend(
            renamer_fresh
                .ids
                .iter()
                .map(|(id, id_fresh)| (id.clone(), id_fresh.clone())),
        );
        let exps = input::combine(&input_hint, exps_input, exps_output)
            .map_err(|error| StructureError::new(StructureErrorKind::Input(error), span.clone()))?;
        let mixop = not_exp.to_mixop();
        let not_exp =
            Mixop::fill(&mixop, exps).expect("validated arguments preserve the mixfix arity");
        let iter_instrs = renamer.rename_iterinstrs_bound(iter_instrs);
        let block = renamer.rename_block(block)?;
        Ok(ol::InstrKind::Rule(ol::RuleInstr {
            id,
            not_exp,
            input_hint,
            iter_instrs,
            block,
        }))
    }

    // - Result instruction

    fn rename_result_instr(&self, instr_ol: ol::ResultInstr) -> ol::InstrKind {
        let ol::ResultInstr {
            rel_signature,
            exps,
        } = instr_ol;
        let exps = self.rename_exps(exps);
        ol::InstrKind::Result(ol::ResultInstr {
            rel_signature,
            exps,
        })
    }

    // - Return instruction

    fn rename_return_instr(&self, instr_ol: ol::ReturnInstr) -> ol::InstrKind {
        let ol::ReturnInstr { exp } = instr_ol;
        let exp = self.rename_exp(exp);
        ol::InstrKind::Return(ol::ReturnInstr { exp })
    }

    // - Debug instruction

    fn rename_debug_instr(
        &self,
        instr_ol: ol::DebugInstr,
    ) -> Result<ol::InstrKind, StructureError> {
        let ol::DebugInstr { exp, instr } = instr_ol;
        let exp = self.rename_exp(exp);
        let instr = Box::new(self.rename_instr(*instr)?);
        Ok(ol::InstrKind::Debug(ol::DebugInstr { exp, instr }))
    }

    // == Blocks

    pub(crate) fn rename_block(&self, block: ol::Block) -> Result<ol::Block, StructureError> {
        self.rename_instrs(block)
    }

    // == Instruction iterators

    // - Bound variables

    pub(crate) fn rename_iterinstr_bound(&self, iter_instr: ol::InstrIter) -> ol::InstrIter {
        let ol::InstrIter {
            iter,
            vars_bound,
            vars_bind,
        } = iter_instr;
        let vars_bound = self.rename_vars(vars_bound);
        ol::InstrIter {
            iter,
            vars_bound,
            vars_bind,
        }
    }

    pub(crate) fn rename_iterinstrs_bound(
        &self,
        iter_instrs: Vec<ol::InstrIter>,
    ) -> Vec<ol::InstrIter> {
        iter_instrs
            .into_iter()
            .map(|iter_instr| self.rename_iterinstr_bound(iter_instr))
            .collect()
    }

    // - Binding variables

    pub(crate) fn rename_iterinstr_bind(&self, iter_instr: ol::InstrIter) -> ol::InstrIter {
        let ol::InstrIter {
            iter,
            vars_bound,
            vars_bind,
        } = iter_instr;
        let vars_bind = self.rename_vars(vars_bind);
        ol::InstrIter {
            iter,
            vars_bound,
            vars_bind,
        }
    }

    pub(crate) fn rename_iterinstrs_bind(
        &self,
        iter_instrs: Vec<ol::InstrIter>,
    ) -> Vec<ol::InstrIter> {
        iter_instrs
            .into_iter()
            .map(|iter_instr| self.rename_iterinstr_bind(iter_instr))
            .collect()
    }
}
