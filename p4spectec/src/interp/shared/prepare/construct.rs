//! Prepared constructor programs for immutable list-map bodies
//!
//! Instructions retain child evaluation order and original type allocations.
//! Plans contain no arena handles and are registered only for stored SL syntax.

use std::{collections::HashMap, rc::Rc};

use foldhash::fast::RandomState;
use smallvec::SmallVec;

use crate::lang::{common::source::Span, data::var::SlotIdx};

use super::ast;

/// A read or constructor evaluated after its children.
#[derive(Debug)]
pub(crate) enum ConstructOp {
    Slot(SlotIdx),
    Column(usize),
    Bool(bool),
    Num(ast::Num),
    Text(ast::Text),
    Tuple(usize),
    Case(Rc<ast::Mixop>, usize),
    Struct(Vec<ast::Atom>),
    Opt(bool),
    List(usize),
}

/// One operation and its source position within the original expression.
#[derive(Debug)]
pub(crate) struct ConstructInstr {
    pub(crate) op: ConstructOp,
    pub(crate) typ: Rc<ast::TypKind>,
    pub(crate) span: Span,
    pub(crate) parent: Option<usize>,
    pub(crate) child: usize,
}

/// A closed constructor expression, in postorder.
#[derive(Debug)]
pub struct ConstructPlan {
    pub(crate) instrs: Vec<ConstructInstr>,
}

impl ConstructPlan {
    /// Resolves row reads without checking values or allocating arena entries.
    fn new(exp: &ast::Exp, vars: &[ast::Var]) -> Option<Self> {
        fn collect(
            exp: &ast::Exp,
            vars: &[ast::Var],
            instrs: &mut Vec<ConstructInstr>,
            child: usize,
        ) -> Option<usize> {
            stacker::maybe_grow(64 * 1024, 1024 * 1024, || {
                let mut children = SmallVec::<[usize; 4]>::new();
                let mut visit = |exp| {
                    let idx = collect(exp, vars, instrs, children.len())?;
                    children.push(idx);
                    Some(())
                };
                let op = match &exp.node {
                    ast::ExpKind::Id(id) => vars
                        .iter()
                        .rposition(|var| var.slot == id.slot)
                        .map_or(ConstructOp::Slot(id.slot), ConstructOp::Column),
                    ast::ExpKind::Bool(value) => ConstructOp::Bool(*value),
                    ast::ExpKind::Num(num) => ConstructOp::Num(num.clone()),
                    ast::ExpKind::Text(text) => ConstructOp::Text(text.clone()),
                    ast::ExpKind::Tuple(exps) => {
                        for exp in exps {
                            visit(exp)?;
                        }
                        ConstructOp::Tuple(exps.len())
                    }
                    ast::ExpKind::Case(not_exp) => {
                        for exp in not_exp.args() {
                            visit(exp)?;
                        }
                        ConstructOp::Case(Rc::clone(not_exp.mixop()), not_exp.args().len())
                    }
                    ast::ExpKind::Str(exp_fields) => {
                        for exp_field in exp_fields {
                            visit(&exp_field.exp)?;
                        }
                        ConstructOp::Struct(
                            exp_fields
                                .iter()
                                .map(|exp_field| exp_field.atom.clone())
                                .collect(),
                        )
                    }
                    ast::ExpKind::Opt(exp_opt) => {
                        if let Some(exp) = exp_opt {
                            visit(exp)?;
                        }
                        ConstructOp::Opt(exp_opt.is_some())
                    }
                    ast::ExpKind::List(exps) => {
                        for exp in exps {
                            visit(exp)?;
                        }
                        ConstructOp::List(exps.len())
                    }
                    _ => return None,
                };
                let idx = instrs.len();
                for idx_child in children {
                    instrs[idx_child].parent = Some(idx);
                }
                instrs.push(ConstructInstr {
                    op,
                    typ: Rc::clone(&exp.note),
                    span: exp.span,
                    parent: None,
                    child,
                });
                Some(idx)
            })
        }
        let mut instrs = Vec::new();
        collect(exp, vars, &mut instrs, 0)?;
        Some(Self { instrs })
    }
}

/// Plans keyed by actual immutable expressions retained in the owning Global.
#[derive(Default)]
pub(crate) struct ConstructPlans {
    plans: HashMap<*const ast::Exp, ConstructPlan, RandomState>,
}

impl ConstructPlans {
    /// Discards registrations before the owning syntax can move.
    pub(crate) fn clear(&mut self) {
        self.plans.clear();
    }

    /// Finds a stored expression without dereferencing its address.
    pub(crate) fn get(&self, exp: &ast::Exp) -> Option<&ConstructPlan> {
        self.plans.get(&std::ptr::from_ref(exp))
    }

    /// Registers closed list-map bodies throughout an evaluated expression.
    pub(crate) fn register(&mut self, exp: &ast::Exp) {
        stacker::maybe_grow(64 * 1024, 1024 * 1024, || match &exp.node {
            ast::ExpKind::Iter(exp_inner, exp_iter) => {
                if exp_iter.iter == ast::Iter::List
                    && !matches!(exp_inner.node, ast::ExpKind::Id(_))
                    && let Some(plan) = ConstructPlan::new(exp_inner, &exp_iter.vars)
                {
                    self.plans.insert(std::ptr::from_ref(exp), plan);
                }
                self.register(exp_inner);
            }
            ast::ExpKind::Tuple(exps) | ast::ExpKind::List(exps) => {
                for exp in exps {
                    self.register(exp);
                }
            }
            ast::ExpKind::Case(not_exp) => {
                for exp in not_exp.args() {
                    self.register(exp);
                }
            }
            ast::ExpKind::Str(exp_fields) => {
                for exp_field in exp_fields {
                    self.register(&exp_field.exp);
                }
            }
            ast::ExpKind::Un(_, _, exp)
            | ast::ExpKind::UpCast(_, exp)
            | ast::ExpKind::DownCast(_, exp)
            | ast::ExpKind::Sub(exp, _, _)
            | ast::ExpKind::Match(exp, _)
            | ast::ExpKind::Opt(Some(exp))
            | ast::ExpKind::Len(exp)
            | ast::ExpKind::Dot(exp, _) => self.register(exp),
            ast::ExpKind::Bin(_, _, exp_l, exp_r)
            | ast::ExpKind::Cmp(_, _, exp_l, exp_r)
            | ast::ExpKind::Cons(exp_l, exp_r)
            | ast::ExpKind::Cat(exp_l, exp_r)
            | ast::ExpKind::Mem(exp_l, exp_r)
            | ast::ExpKind::Idx(exp_l, exp_r) => {
                self.register(exp_l);
                self.register(exp_r);
            }
            ast::ExpKind::Slice(exp_base, exp_idx, exp_len) => {
                self.register(exp_base);
                self.register(exp_idx);
                self.register(exp_len);
            }
            ast::ExpKind::Upd(exp_base, path, exp_new) => {
                self.register(exp_base);
                self.register_path(path);
                self.register(exp_new);
            }
            ast::ExpKind::Call(_, _, args) => {
                for arg in args {
                    if let ast::ArgKind::Exp(exp) = &arg.node {
                        self.register(exp);
                    }
                }
            }
            ast::ExpKind::Bool(_)
            | ast::ExpKind::Num(_)
            | ast::ExpKind::Text(_)
            | ast::ExpKind::Id(_)
            | ast::ExpKind::Opt(None) => {}
        });
    }

    /// Visits expression operands inside a stored update path.
    fn register_path(&mut self, path: &ast::Path) {
        stacker::maybe_grow(64 * 1024, 1024 * 1024, || match &path.node {
            ast::PathKind::Root => {}
            ast::PathKind::Idx(path, exp) => {
                self.register_path(path);
                self.register(exp);
            }
            ast::PathKind::Slice(path, exp_idx, exp_len) => {
                self.register_path(path);
                self.register(exp_idx);
                self.register(exp_len);
            }
            ast::PathKind::Dot(path, _) => self.register_path(path),
        });
    }
}
