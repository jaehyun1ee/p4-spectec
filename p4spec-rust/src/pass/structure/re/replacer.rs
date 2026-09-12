//! Capture-avoiding expression replacement for structured instructions
use super::super::{StructureError, StructureErrorKind, ol::ast as ol};
use super::renamer::Renamer;
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
pub(crate) struct Replacer {
    exps: IdMap<Exp>,
}

impl Replacer {
    pub(crate) fn empty() -> Self {
        Self::default()
    }
    pub(crate) fn dom(&self) -> IdSet {
        self.exps.domain()
    }
    pub(crate) fn singleton(id: Id, exp: Exp) -> Self {
        let mut replacer = Self::empty();
        replacer.add(id, exp);
        replacer
    }
    pub(crate) fn add(&mut self, id: Id, exp: Exp) {
        self.exps.insert(id, exp);
    }
    pub(crate) fn filter(&self, mut predicate: impl FnMut(&Id, &Exp) -> bool) -> Self {
        Self {
            exps: self
                .exps
                .iter()
                .filter(|(id, exp)| predicate(id, exp))
                .map(|(id, exp)| (id.clone(), exp.clone()))
                .collect(),
        }
    }
    pub(crate) fn freshen_binders(&self, frees: &IdSet, block: &ol::Block) -> Renamer {
        let ids_codom = self
            .exps
            .values()
            .fold(IdSet::new(), |ids, exp| ids.union(exp.free()));
        let ids_collide: IdSet = frees
            .iter()
            .filter(|id| ids_codom.contains(id))
            .cloned()
            .collect();
        let mut ids_avoid = frees
            .clone()
            .union(block.free())
            .union(self.dom())
            .union(ids_codom);
        let mut renamer_fresh = Renamer::empty();
        for id in ids_collide.iter() {
            let id_fresh = fresh::id(&ids_avoid, id);
            renamer_fresh.add(id.clone(), id_fresh.clone());
            ids_avoid.insert(id_fresh);
        }
        renamer_fresh
    }

    pub(crate) fn replace_exp(&self, exp: Exp) -> Exp {
        let NotePhrase {
            node: exp_kind,
            note,
            span,
        } = exp;
        let exp_kind = match exp_kind {
            ExpKind::Bool(_) | ExpKind::Num(_) | ExpKind::Text(_) => exp_kind,
            ExpKind::Var(id) => return self.replace_var_exp(id, note, span),
            ExpKind::Un(op, op_typ, exp) => self.replace_un_exp(op, op_typ, exp),
            ExpKind::Bin(op, op_typ, exp_l, exp_r) => {
                self.replace_bin_exp(op, op_typ, exp_l, exp_r)
            }
            ExpKind::Cmp(op, op_typ, exp_l, exp_r) => {
                self.replace_cmp_exp(op, op_typ, exp_l, exp_r)
            }
            ExpKind::UpCast(typ, exp) => self.replace_up_cast_exp(typ, exp),
            ExpKind::DownCast(typ, exp) => self.replace_down_cast_exp(typ, exp),
            ExpKind::Sub(exp, typ, subcheck) => self.replace_sub_exp(exp, typ, subcheck),
            ExpKind::Match(exp, pattern) => self.replace_match_exp(exp, pattern),
            ExpKind::Tuple(exps) => self.replace_tuple_exp(exps),
            ExpKind::Case(not_exp) => self.replace_case_exp(not_exp),
            ExpKind::Str(exp_fields) => self.replace_str_exp(exp_fields),
            ExpKind::Opt(exp) => self.replace_opt_exp(exp),
            ExpKind::List(exps) => self.replace_list_exp(exps),
            ExpKind::Cons(exp_head, exp_tail) => self.replace_cons_exp(exp_head, exp_tail),
            ExpKind::Cat(exp_l, exp_r) => self.replace_cat_exp(exp_l, exp_r),
            ExpKind::Mem(exp_elem, exp_set) => self.replace_mem_exp(exp_elem, exp_set),
            ExpKind::Len(exp) => self.replace_len_exp(exp),
            ExpKind::Dot(exp, atom) => self.replace_dot_exp(exp, atom),
            ExpKind::Idx(exp_base, exp_idx) => self.replace_idx_exp(exp_base, exp_idx),
            ExpKind::Slice(exp_base, exp_idx, exp_len) => {
                self.replace_slice_exp(exp_base, exp_idx, exp_len)
            }
            ExpKind::Upd(exp_base, path, exp_field) => {
                self.replace_upd_exp(exp_base, *path, exp_field)
            }
            ExpKind::Call(id, targs, args) => self.replace_call_exp(id, targs, args),
            ExpKind::Iter(exp, iter_exp) => self.replace_iter_exp(exp, iter_exp),
        };
        NotePhrase {
            node: exp_kind,
            note,
            span,
        }
    }
    fn replace_var_exp(&self, id: Id, note: std::rc::Rc<TypKind>, span: Span) -> Exp {
        self.exps.get(&id).cloned().unwrap_or(NotePhrase {
            node: ExpKind::Var(id),
            note,
            span,
        })
    }
    fn replace_un_exp(&self, op: UnOp, op_typ: OpTyp, exp: Box<Exp>) -> ExpKind {
        ExpKind::Un(op, op_typ, Box::new(self.replace_exp(*exp)))
    }
    fn replace_bin_exp(
        &self,
        op: BinOp,
        op_typ: OpTyp,
        exp_l: Box<Exp>,
        exp_r: Box<Exp>,
    ) -> ExpKind {
        ExpKind::Bin(
            op,
            op_typ,
            Box::new(self.replace_exp(*exp_l)),
            Box::new(self.replace_exp(*exp_r)),
        )
    }
    fn replace_cmp_exp(
        &self,
        op: CmpOp,
        op_typ: OpTyp,
        exp_l: Box<Exp>,
        exp_r: Box<Exp>,
    ) -> ExpKind {
        ExpKind::Cmp(
            op,
            op_typ,
            Box::new(self.replace_exp(*exp_l)),
            Box::new(self.replace_exp(*exp_r)),
        )
    }
    fn replace_up_cast_exp(&self, typ: Box<Typ>, exp: Box<Exp>) -> ExpKind {
        ExpKind::UpCast(typ, Box::new(self.replace_exp(*exp)))
    }
    fn replace_down_cast_exp(&self, typ: Box<Typ>, exp: Box<Exp>) -> ExpKind {
        ExpKind::DownCast(typ, Box::new(self.replace_exp(*exp)))
    }
    fn replace_sub_exp(&self, exp: Box<Exp>, typ: Box<Typ>, subcheck: Box<Subcheck>) -> ExpKind {
        ExpKind::Sub(Box::new(self.replace_exp(*exp)), typ, subcheck)
    }
    fn replace_match_exp(&self, exp: Box<Exp>, pattern: Pattern) -> ExpKind {
        ExpKind::Match(Box::new(self.replace_exp(*exp)), pattern)
    }
    fn replace_tuple_exp(&self, exps: Vec<Exp>) -> ExpKind {
        ExpKind::Tuple(self.replace_exps(exps))
    }
    fn replace_case_exp(&self, not_exp: Box<NotExp>) -> ExpKind {
        ExpKind::Case(Box::new(not_exp.map(|exp| self.replace_exp(exp.clone()))))
    }
    fn replace_str_exp(&self, exp_fields: Vec<ExpField>) -> ExpKind {
        ExpKind::Str(
            exp_fields
                .into_iter()
                .map(|(atom, exp)| (atom, self.replace_exp(exp)))
                .collect(),
        )
    }
    fn replace_opt_exp(&self, exp: Option<Box<Exp>>) -> ExpKind {
        ExpKind::Opt(exp.map(|exp| Box::new(self.replace_exp(*exp))))
    }
    fn replace_list_exp(&self, exps: Vec<Exp>) -> ExpKind {
        ExpKind::List(self.replace_exps(exps))
    }
    fn replace_cons_exp(&self, exp_head: Box<Exp>, exp_tail: Box<Exp>) -> ExpKind {
        ExpKind::Cons(
            Box::new(self.replace_exp(*exp_head)),
            Box::new(self.replace_exp(*exp_tail)),
        )
    }
    fn replace_cat_exp(&self, exp_l: Box<Exp>, exp_r: Box<Exp>) -> ExpKind {
        ExpKind::Cat(
            Box::new(self.replace_exp(*exp_l)),
            Box::new(self.replace_exp(*exp_r)),
        )
    }
    fn replace_mem_exp(&self, exp_elem: Box<Exp>, exp_set: Box<Exp>) -> ExpKind {
        ExpKind::Mem(
            Box::new(self.replace_exp(*exp_elem)),
            Box::new(self.replace_exp(*exp_set)),
        )
    }
    fn replace_len_exp(&self, exp: Box<Exp>) -> ExpKind {
        ExpKind::Len(Box::new(self.replace_exp(*exp)))
    }
    fn replace_dot_exp(&self, exp: Box<Exp>, atom: Atom) -> ExpKind {
        ExpKind::Dot(Box::new(self.replace_exp(*exp)), atom)
    }
    fn replace_idx_exp(&self, exp_base: Box<Exp>, exp_idx: Box<Exp>) -> ExpKind {
        ExpKind::Idx(
            Box::new(self.replace_exp(*exp_base)),
            Box::new(self.replace_exp(*exp_idx)),
        )
    }
    fn replace_slice_exp(
        &self,
        exp_base: Box<Exp>,
        exp_idx: Box<Exp>,
        exp_len: Box<Exp>,
    ) -> ExpKind {
        ExpKind::Slice(
            Box::new(self.replace_exp(*exp_base)),
            Box::new(self.replace_exp(*exp_idx)),
            Box::new(self.replace_exp(*exp_len)),
        )
    }
    fn replace_upd_exp(&self, exp_base: Box<Exp>, path: Path, exp_field: Box<Exp>) -> ExpKind {
        ExpKind::Upd(
            Box::new(self.replace_exp(*exp_base)),
            Box::new(self.replace_path(path)),
            Box::new(self.replace_exp(*exp_field)),
        )
    }
    fn replace_call_exp(&self, id: Id, targs: Vec<Targ>, args: Vec<Arg>) -> ExpKind {
        ExpKind::Call(id, targs, self.replace_args(args))
    }
    fn replace_iter_exp(&self, exp: Box<Exp>, iter_exp: ExpIter) -> ExpKind {
        ExpKind::Iter(
            Box::new(self.replace_exp(*exp)),
            self.replace_iterexp(iter_exp),
        )
    }

    pub(crate) fn replace_exps(&self, exps: Vec<Exp>) -> Vec<Exp> {
        exps.into_iter().map(|exp| self.replace_exp(exp)).collect()
    }
    pub(crate) fn replace_iterexp(&self, iter_exp: ExpIter) -> ExpIter {
        let (iter, vars) = iter_exp;
        (iter, self.filter_vars(vars))
    }
    pub(crate) fn replace_iterexps(&self, iter_exps: Vec<ExpIter>) -> Vec<ExpIter> {
        iter_exps
            .into_iter()
            .map(|iter_exp| self.replace_iterexp(iter_exp))
            .collect()
    }
    fn filter_vars(&self, vars: Vec<Var>) -> Vec<Var> {
        vars.into_iter()
            .filter(|var| !self.exps.contains_key(&var.id))
            .collect()
    }
    pub(crate) fn replace_path(&self, path: Path) -> Path {
        let NotePhrase {
            node: path_kind,
            note,
            span,
        } = path;
        let path_kind = match path_kind {
            PathKind::Root => PathKind::Root,
            PathKind::Idx(path, exp) => self.replace_idx_path(*path, exp),
            PathKind::Slice(path, exp_idx, exp_len) => {
                self.replace_slice_path(*path, exp_idx, exp_len)
            }
            PathKind::Dot(path, atom) => self.replace_dot_path(*path, atom),
        };
        NotePhrase {
            node: path_kind,
            note,
            span,
        }
    }
    fn replace_idx_path(&self, path: Path, exp: Box<Exp>) -> PathKind {
        PathKind::Idx(
            Box::new(self.replace_path(path)),
            Box::new(self.replace_exp(*exp)),
        )
    }
    fn replace_slice_path(&self, path: Path, exp_idx: Box<Exp>, exp_len: Box<Exp>) -> PathKind {
        PathKind::Slice(
            Box::new(self.replace_path(path)),
            Box::new(self.replace_exp(*exp_idx)),
            Box::new(self.replace_exp(*exp_len)),
        )
    }
    fn replace_dot_path(&self, path: Path, atom: Atom) -> PathKind {
        PathKind::Dot(Box::new(self.replace_path(path)), atom)
    }
    pub(crate) fn replace_arg(&self, arg: Arg) -> Arg {
        let NotePhrase {
            node: arg_kind,
            note,
            span,
        } = arg;
        let arg_kind = match arg_kind {
            ArgKind::Exp(exp) => self.replace_exp_arg(exp),
            ArgKind::Def(_) => arg_kind,
        };
        NotePhrase {
            node: arg_kind,
            note,
            span,
        }
    }
    fn replace_exp_arg(&self, exp: Box<Exp>) -> ArgKind {
        ArgKind::Exp(Box::new(self.replace_exp(*exp)))
    }
    pub(crate) fn replace_args(&self, args: Vec<Arg>) -> Vec<Arg> {
        args.into_iter().map(|arg| self.replace_arg(arg)).collect()
    }
    pub(crate) fn replace_case(&self, case: ol::Case) -> Result<ol::Case, StructureError> {
        let ol::Case { guard, block } = case;
        let guard = self.replace_guard(guard);
        let block = self.replace_block(block)?;
        Ok(ol::Case { guard, block })
    }
    pub(crate) fn replace_cases(
        &self,
        cases: Vec<ol::Case>,
    ) -> Result<Vec<ol::Case>, StructureError> {
        cases
            .into_iter()
            .map(|case| self.replace_case(case))
            .collect()
    }
    pub(crate) fn replace_guard(&self, guard: ol::Guard) -> ol::Guard {
        match guard {
            ol::Guard::Bool(_) | ol::Guard::Sub(..) | ol::Guard::Match(_) => guard,
            ol::Guard::Cmp(op, op_typ, exp) => self.replace_cmp_guard(op, op_typ, exp),
            ol::Guard::Mem(exp) => self.replace_mem_guard(exp),
        }
    }
    fn replace_cmp_guard(&self, op: CmpOp, op_typ: OpTyp, exp: Exp) -> ol::Guard {
        ol::Guard::Cmp(op, op_typ, self.replace_exp(exp))
    }
    fn replace_mem_guard(&self, exp: Exp) -> ol::Guard {
        ol::Guard::Mem(self.replace_exp(exp))
    }
    pub(crate) fn replace_instr(&self, instr_ol: ol::Instr) -> Result<ol::Instr, StructureError> {
        let NotePhrase {
            node: instr_kind_ol,
            note,
            span,
        } = instr_ol;
        let instr_kind_ol = self.replace_instr_kind(instr_kind_ol, &span)?;
        Ok(NotePhrase {
            node: instr_kind_ol,
            note,
            span,
        })
    }
    fn replace_instr_kind(
        &self,
        instr_kind_ol: ol::InstrKind,
        span: &Span,
    ) -> Result<ol::InstrKind, StructureError> {
        match instr_kind_ol {
            ol::InstrKind::If(instr_ol) => self.replace_if_instr(instr_ol),
            ol::InstrKind::Hold(instr_ol) => self.replace_hold_instr(instr_ol),
            ol::InstrKind::Case(instr_ol) => self.replace_case_instr(instr_ol),
            ol::InstrKind::Group(instr_ol) => self.replace_group_instr(instr_ol),
            ol::InstrKind::Let(instr_ol) => self.replace_let_instr(instr_ol),
            ol::InstrKind::Rule(instr_ol) => self.replace_rule_instr(instr_ol, span),
            ol::InstrKind::Result(instr_ol) => Ok(self.replace_result_instr(instr_ol)),
            ol::InstrKind::Return(instr_ol) => Ok(self.replace_return_instr(instr_ol)),
            ol::InstrKind::Debug(instr_ol) => self.replace_debug_instr(instr_ol),
        }
    }
    fn replace_if_instr(&self, instr_ol: ol::IfInstr) -> Result<ol::InstrKind, StructureError> {
        let ol::IfInstr {
            exp,
            iter_exps,
            block,
        } = instr_ol;
        let exp = self.replace_exp(exp);
        let iter_exps = self.replace_iterexps(iter_exps);
        let block = self.replace_block(block)?;
        Ok(ol::InstrKind::If(ol::IfInstr {
            exp,
            iter_exps,
            block,
        }))
    }
    fn replace_hold_instr(&self, instr_ol: ol::HoldInstr) -> Result<ol::InstrKind, StructureError> {
        let ol::HoldInstr {
            id,
            not_exp,
            iter_exps,
            block_hold,
            block_not_hold,
        } = instr_ol;
        let not_exp = not_exp.map(|exp| self.replace_exp(exp.clone()));
        let iter_exps = self.replace_iterexps(iter_exps);
        let block_hold = self.replace_block(block_hold)?;
        let block_not_hold = self.replace_block(block_not_hold)?;
        Ok(ol::InstrKind::Hold(ol::HoldInstr {
            id,
            not_exp,
            iter_exps,
            block_hold,
            block_not_hold,
        }))
    }
    fn replace_case_instr(&self, instr_ol: ol::CaseInstr) -> Result<ol::InstrKind, StructureError> {
        let ol::CaseInstr { exp, cases, total } = instr_ol;
        let exp = self.replace_exp(exp);
        let cases = self.replace_cases(cases)?;
        Ok(ol::InstrKind::Case(ol::CaseInstr { exp, cases, total }))
    }
    fn replace_group_instr(
        &self,
        instr_ol: ol::GroupInstr,
    ) -> Result<ol::InstrKind, StructureError> {
        let ol::GroupInstr {
            id,
            rel_signature,
            exps,
            block,
        } = instr_ol;
        let exps = self.replace_exps(exps);
        let block = self.replace_block(block)?;
        Ok(ol::InstrKind::Group(ol::GroupInstr {
            id,
            rel_signature,
            exps,
            block,
        }))
    }
    fn replace_let_instr(&self, instr_ol: ol::LetInstr) -> Result<ol::InstrKind, StructureError> {
        let ol::LetInstr {
            exp_l,
            exp_r,
            iter_instrs,
            block,
        } = instr_ol;
        let frees_l = exp_l.free();
        let replacer = self.filter(|id, _| !frees_l.contains(id));
        let renamer_fresh = replacer.freshen_binders(&frees_l, &block);
        let exp_l = renamer_fresh.rename_exp(exp_l);
        let iter_instrs = renamer_fresh.rename_iterinstrs_bound(iter_instrs);
        let block = renamer_fresh.rename_block(block)?;
        let exp_r = replacer.replace_exp(exp_r);
        let iter_instrs = replacer.replace_iterinstrs_bound(iter_instrs);
        let block = replacer.replace_block(block)?;
        Ok(ol::InstrKind::Let(ol::LetInstr {
            exp_l,
            exp_r,
            iter_instrs,
            block,
        }))
    }
    fn replace_rule_instr(
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
        let exps_input = self.replace_exps(exps_input);
        let frees_output = exps_output.as_slice().free();
        let replacer = self.filter(|id, _| !frees_output.contains(id));
        let renamer_fresh = replacer.freshen_binders(&frees_output, &block);
        let exps_output = renamer_fresh.rename_exps(exps_output);
        let iter_instrs = renamer_fresh.rename_iterinstrs_bound(iter_instrs);
        let block = renamer_fresh.rename_block(block)?;
        let exps = input::combine(&input_hint, exps_input, exps_output)
            .map_err(|error| StructureError::new(StructureErrorKind::Input(error), span.clone()))?;
        // The original mixfix has exactly the validated argument count
        let mut idx = 0;
        let not_exp = not_exp.map(|_| {
            let exp = exps[idx].clone();
            idx += 1;
            exp
        });
        let iter_instrs = replacer.replace_iterinstrs_bound(iter_instrs);
        let block = replacer.replace_block(block)?;
        Ok(ol::InstrKind::Rule(ol::RuleInstr {
            id,
            not_exp,
            input_hint,
            iter_instrs,
            block,
        }))
    }
    fn replace_result_instr(&self, instr_ol: ol::ResultInstr) -> ol::InstrKind {
        let ol::ResultInstr {
            rel_signature,
            exps,
        } = instr_ol;
        let exps = self.replace_exps(exps);
        ol::InstrKind::Result(ol::ResultInstr {
            rel_signature,
            exps,
        })
    }
    fn replace_return_instr(&self, instr_ol: ol::ReturnInstr) -> ol::InstrKind {
        let ol::ReturnInstr { exp } = instr_ol;
        let exp = self.replace_exp(exp);
        ol::InstrKind::Return(ol::ReturnInstr { exp })
    }
    fn replace_debug_instr(
        &self,
        instr_ol: ol::DebugInstr,
    ) -> Result<ol::InstrKind, StructureError> {
        let ol::DebugInstr { exp, instr } = instr_ol;
        let exp = self.replace_exp(exp);
        let instr = Box::new(self.replace_instr(*instr)?);
        Ok(ol::InstrKind::Debug(ol::DebugInstr { exp, instr }))
    }
    pub(crate) fn replace_instrs(
        &self,
        instrs_ol: Vec<ol::Instr>,
    ) -> Result<Vec<ol::Instr>, StructureError> {
        instrs_ol
            .into_iter()
            .map(|instr_ol| self.replace_instr(instr_ol))
            .collect()
    }
    pub(crate) fn replace_block(&self, block: ol::Block) -> Result<ol::Block, StructureError> {
        self.replace_instrs(block)
    }
    pub(crate) fn replace_iterinstr_bound(&self, iter_instr: ol::InstrIter) -> ol::InstrIter {
        let ol::InstrIter {
            iter,
            vars_bound,
            vars_bind,
        } = iter_instr;
        // The source replacement filters the third iterator component
        let vars_bind = self.filter_vars(vars_bind);
        ol::InstrIter {
            iter,
            vars_bound,
            vars_bind,
        }
    }
    pub(crate) fn replace_iterinstrs_bound(
        &self,
        iter_instrs: Vec<ol::InstrIter>,
    ) -> Vec<ol::InstrIter> {
        iter_instrs
            .into_iter()
            .map(|iter_instr| self.replace_iterinstr_bound(iter_instr))
            .collect()
    }
}
