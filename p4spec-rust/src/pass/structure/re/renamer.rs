//! Capture-avoiding identifier renaming for structured instructions
use super::super::{StructureError, StructureErrorKind, ol::ast as ol};
use crate::lang::{
    common::{
        ds::{map::IdMap, set::IdSet},
        source::{NotePhrase, Span},
    },
    hints::input,
    il::{ast::*, fresh},
    traits::free::Free,
};

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
        self.ids.values().cloned().collect()
    }
    pub(crate) fn singleton(id: Id, id_renamed: Id) -> Self {
        Self::of_list(vec![(id, id_renamed)])
    }
    pub(crate) fn add(&mut self, id: Id, id_renamed: Id) {
        self.ids.insert(id, id_renamed);
    }
    pub(crate) fn of_list(pairs: Vec<(Id, Id)>) -> Self {
        Self {
            ids: pairs.into_iter().collect(),
        }
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

    pub(crate) fn rename_exp(&self, exp: Exp) -> Exp {
        let NotePhrase {
            node: exp_kind,
            note,
            span,
        } = exp;
        let exp_kind = self.rename_exp_kind(exp_kind);
        NotePhrase {
            node: exp_kind,
            note,
            span,
        }
    }
    fn rename_exp_kind(&self, exp_kind: ExpKind) -> ExpKind {
        match exp_kind {
            ExpKind::Bool(_) | ExpKind::Num(_) | ExpKind::Text(_) => exp_kind,
            ExpKind::Var(id) => self.rename_var_exp(id),
            ExpKind::Un(op, op_typ, exp) => self.rename_un_exp(op, op_typ, exp),
            ExpKind::Bin(op, op_typ, exp_l, exp_r) => self.rename_bin_exp(op, op_typ, exp_l, exp_r),
            ExpKind::Cmp(op, op_typ, exp_l, exp_r) => self.rename_cmp_exp(op, op_typ, exp_l, exp_r),
            ExpKind::UpCast(typ, exp) => self.rename_up_cast_exp(typ, exp),
            ExpKind::DownCast(typ, exp) => self.rename_down_cast_exp(typ, exp),
            ExpKind::Sub(exp, typ, subcheck) => self.rename_sub_exp(exp, typ, subcheck),
            ExpKind::Match(exp, pattern) => self.rename_match_exp(exp, pattern),
            ExpKind::Tuple(exps) => self.rename_tuple_exp(exps),
            ExpKind::Case(not_exp) => self.rename_case_exp(not_exp),
            ExpKind::Str(exp_fields) => self.rename_str_exp(exp_fields),
            ExpKind::Opt(exp) => self.rename_opt_exp(exp),
            ExpKind::List(exps) => self.rename_list_exp(exps),
            ExpKind::Cons(exp_head, exp_tail) => self.rename_cons_exp(exp_head, exp_tail),
            ExpKind::Cat(exp_l, exp_r) => self.rename_cat_exp(exp_l, exp_r),
            ExpKind::Mem(exp_elem, exp_set) => self.rename_mem_exp(exp_elem, exp_set),
            ExpKind::Len(exp) => self.rename_len_exp(exp),
            ExpKind::Dot(exp, atom) => self.rename_dot_exp(exp, atom),
            ExpKind::Idx(exp_base, exp_idx) => self.rename_idx_exp(exp_base, exp_idx),
            ExpKind::Slice(exp_base, exp_idx, exp_len) => {
                self.rename_slice_exp(exp_base, exp_idx, exp_len)
            }
            ExpKind::Upd(exp_base, path, exp_field) => {
                self.rename_upd_exp(exp_base, *path, exp_field)
            }
            ExpKind::Call(id, targs, args) => self.rename_call_exp(id, targs, args),
            ExpKind::Iter(exp, iter_exp) => self.rename_iter_exp(exp, iter_exp),
        }
    }
    fn rename_var_exp(&self, id: Id) -> ExpKind {
        ExpKind::Var(self.ids.get(&id).cloned().unwrap_or(id))
    }
    fn rename_un_exp(&self, op: UnOp, op_typ: OpTyp, exp: Box<Exp>) -> ExpKind {
        ExpKind::Un(op, op_typ, Box::new(self.rename_exp(*exp)))
    }
    fn rename_bin_exp(
        &self,
        op: BinOp,
        op_typ: OpTyp,
        exp_l: Box<Exp>,
        exp_r: Box<Exp>,
    ) -> ExpKind {
        ExpKind::Bin(
            op,
            op_typ,
            Box::new(self.rename_exp(*exp_l)),
            Box::new(self.rename_exp(*exp_r)),
        )
    }
    fn rename_cmp_exp(
        &self,
        op: CmpOp,
        op_typ: OpTyp,
        exp_l: Box<Exp>,
        exp_r: Box<Exp>,
    ) -> ExpKind {
        ExpKind::Cmp(
            op,
            op_typ,
            Box::new(self.rename_exp(*exp_l)),
            Box::new(self.rename_exp(*exp_r)),
        )
    }
    fn rename_up_cast_exp(&self, typ: Box<Typ>, exp: Box<Exp>) -> ExpKind {
        ExpKind::UpCast(typ, Box::new(self.rename_exp(*exp)))
    }
    fn rename_down_cast_exp(&self, typ: Box<Typ>, exp: Box<Exp>) -> ExpKind {
        ExpKind::DownCast(typ, Box::new(self.rename_exp(*exp)))
    }
    fn rename_sub_exp(&self, exp: Box<Exp>, typ: Box<Typ>, subcheck: Box<Subcheck>) -> ExpKind {
        ExpKind::Sub(Box::new(self.rename_exp(*exp)), typ, subcheck)
    }
    fn rename_match_exp(&self, exp: Box<Exp>, pattern: Pattern) -> ExpKind {
        ExpKind::Match(Box::new(self.rename_exp(*exp)), pattern)
    }
    fn rename_tuple_exp(&self, exps: Vec<Exp>) -> ExpKind {
        ExpKind::Tuple(self.rename_exps(exps))
    }
    fn rename_case_exp(&self, not_exp: Box<NotExp>) -> ExpKind {
        ExpKind::Case(Box::new(not_exp.map(|exp| self.rename_exp(exp.clone()))))
    }
    fn rename_str_exp(&self, exp_fields: Vec<ExpField>) -> ExpKind {
        ExpKind::Str(
            exp_fields
                .into_iter()
                .map(|(atom, exp)| (atom, self.rename_exp(exp)))
                .collect(),
        )
    }
    fn rename_opt_exp(&self, exp: Option<Box<Exp>>) -> ExpKind {
        ExpKind::Opt(exp.map(|exp| Box::new(self.rename_exp(*exp))))
    }
    fn rename_list_exp(&self, exps: Vec<Exp>) -> ExpKind {
        ExpKind::List(self.rename_exps(exps))
    }
    fn rename_cons_exp(&self, exp_head: Box<Exp>, exp_tail: Box<Exp>) -> ExpKind {
        ExpKind::Cons(
            Box::new(self.rename_exp(*exp_head)),
            Box::new(self.rename_exp(*exp_tail)),
        )
    }
    fn rename_cat_exp(&self, exp_l: Box<Exp>, exp_r: Box<Exp>) -> ExpKind {
        ExpKind::Cat(
            Box::new(self.rename_exp(*exp_l)),
            Box::new(self.rename_exp(*exp_r)),
        )
    }
    fn rename_mem_exp(&self, exp_elem: Box<Exp>, exp_set: Box<Exp>) -> ExpKind {
        ExpKind::Mem(
            Box::new(self.rename_exp(*exp_elem)),
            Box::new(self.rename_exp(*exp_set)),
        )
    }
    fn rename_len_exp(&self, exp: Box<Exp>) -> ExpKind {
        ExpKind::Len(Box::new(self.rename_exp(*exp)))
    }
    fn rename_dot_exp(&self, exp: Box<Exp>, atom: Atom) -> ExpKind {
        ExpKind::Dot(Box::new(self.rename_exp(*exp)), atom)
    }
    fn rename_idx_exp(&self, exp_base: Box<Exp>, exp_idx: Box<Exp>) -> ExpKind {
        ExpKind::Idx(
            Box::new(self.rename_exp(*exp_base)),
            Box::new(self.rename_exp(*exp_idx)),
        )
    }
    fn rename_slice_exp(
        &self,
        exp_base: Box<Exp>,
        exp_idx: Box<Exp>,
        exp_len: Box<Exp>,
    ) -> ExpKind {
        ExpKind::Slice(
            Box::new(self.rename_exp(*exp_base)),
            Box::new(self.rename_exp(*exp_idx)),
            Box::new(self.rename_exp(*exp_len)),
        )
    }
    fn rename_upd_exp(&self, exp_base: Box<Exp>, path: Path, exp_field: Box<Exp>) -> ExpKind {
        ExpKind::Upd(
            Box::new(self.rename_exp(*exp_base)),
            Box::new(self.rename_path(path)),
            Box::new(self.rename_exp(*exp_field)),
        )
    }
    fn rename_call_exp(&self, id: Id, targs: Vec<Targ>, args: Vec<Arg>) -> ExpKind {
        ExpKind::Call(id, targs, self.rename_args(args))
    }
    fn rename_iter_exp(&self, exp: Box<Exp>, iter_exp: ExpIter) -> ExpKind {
        ExpKind::Iter(
            Box::new(self.rename_exp(*exp)),
            self.rename_iterexp(iter_exp),
        )
    }

    pub(crate) fn rename_exps(&self, exps: Vec<Exp>) -> Vec<Exp> {
        exps.into_iter().map(|exp| self.rename_exp(exp)).collect()
    }
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
    pub(crate) fn rename_path(&self, path: Path) -> Path {
        let NotePhrase {
            node: path_kind,
            note,
            span,
        } = path;
        let path_kind = match path_kind {
            PathKind::Root => PathKind::Root,
            PathKind::Idx(path, exp) => self.rename_idx_path(*path, exp),
            PathKind::Slice(path, exp_idx, exp_len) => {
                self.rename_slice_path(*path, exp_idx, exp_len)
            }
            PathKind::Dot(path, atom) => self.rename_dot_path(*path, atom),
        };
        NotePhrase {
            node: path_kind,
            note,
            span,
        }
    }
    fn rename_idx_path(&self, path: Path, exp: Box<Exp>) -> PathKind {
        PathKind::Idx(
            Box::new(self.rename_path(path)),
            Box::new(self.rename_exp(*exp)),
        )
    }
    fn rename_slice_path(&self, path: Path, exp_idx: Box<Exp>, exp_len: Box<Exp>) -> PathKind {
        PathKind::Slice(
            Box::new(self.rename_path(path)),
            Box::new(self.rename_exp(*exp_idx)),
            Box::new(self.rename_exp(*exp_len)),
        )
    }
    fn rename_dot_path(&self, path: Path, atom: Atom) -> PathKind {
        PathKind::Dot(Box::new(self.rename_path(path)), atom)
    }
    pub(crate) fn rename_arg(&self, arg: Arg) -> Arg {
        let NotePhrase {
            node: arg_kind,
            note,
            span,
        } = arg;
        let arg_kind = match arg_kind {
            ArgKind::Exp(exp) => self.rename_exp_arg(exp),
            ArgKind::Def(_) => arg_kind,
        };
        NotePhrase {
            node: arg_kind,
            note,
            span,
        }
    }
    fn rename_exp_arg(&self, exp: Box<Exp>) -> ArgKind {
        ArgKind::Exp(Box::new(self.rename_exp(*exp)))
    }
    pub(crate) fn rename_args(&self, args: Vec<Arg>) -> Vec<Arg> {
        args.into_iter().map(|arg| self.rename_arg(arg)).collect()
    }
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
    pub(crate) fn rename_guard(&self, guard: ol::Guard) -> ol::Guard {
        match guard {
            ol::Guard::Bool(_) | ol::Guard::Sub(..) | ol::Guard::Match(_) => guard,
            ol::Guard::Cmp(op, op_typ, exp) => self.rename_cmp_guard(op, op_typ, exp),
            ol::Guard::Mem(exp) => self.rename_mem_guard(exp),
        }
    }
    fn rename_cmp_guard(&self, op: CmpOp, op_typ: OpTyp, exp: Exp) -> ol::Guard {
        ol::Guard::Cmp(op, op_typ, self.rename_exp(exp))
    }
    fn rename_mem_guard(&self, exp: Exp) -> ol::Guard {
        ol::Guard::Mem(self.rename_exp(exp))
    }
    pub(crate) fn rename_instr(&self, instr_ol: ol::Instr) -> Result<ol::Instr, StructureError> {
        let NotePhrase {
            node: instr_kind_ol,
            note,
            span,
        } = instr_ol;
        let instr_kind_ol = self.rename_instr_kind(instr_kind_ol, &span)?;
        Ok(NotePhrase {
            node: instr_kind_ol,
            note,
            span,
        })
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
    fn rename_case_instr(&self, instr_ol: ol::CaseInstr) -> Result<ol::InstrKind, StructureError> {
        let ol::CaseInstr { exp, cases, total } = instr_ol;
        let exp = self.rename_exp(exp);
        let cases = self.rename_cases(cases)?;
        Ok(ol::InstrKind::Case(ol::CaseInstr { exp, cases, total }))
    }
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
        // The original mixfix has exactly the validated argument count
        let mut idx = 0;
        let not_exp = not_exp.map(|_| {
            let exp = exps[idx].clone();
            idx += 1;
            exp
        });
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
    fn rename_return_instr(&self, instr_ol: ol::ReturnInstr) -> ol::InstrKind {
        let ol::ReturnInstr { exp } = instr_ol;
        let exp = self.rename_exp(exp);
        ol::InstrKind::Return(ol::ReturnInstr { exp })
    }
    fn rename_debug_instr(
        &self,
        instr_ol: ol::DebugInstr,
    ) -> Result<ol::InstrKind, StructureError> {
        let ol::DebugInstr { exp, instr } = instr_ol;
        let exp = self.rename_exp(exp);
        let instr = Box::new(self.rename_instr(*instr)?);
        Ok(ol::InstrKind::Debug(ol::DebugInstr { exp, instr }))
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
    pub(crate) fn rename_block(&self, block: ol::Block) -> Result<ol::Block, StructureError> {
        self.rename_instrs(block)
    }
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
