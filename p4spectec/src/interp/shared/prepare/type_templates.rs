//! Lazy type templates for output bindings in stored SL syntax
//!
//! Registration retains addresses only while the owning Global holds syntax.
//! First use builds the same iterated type as a fresh output annotation;
//! each arena output still reserves its own distinct allocation identity.

use std::{collections::HashMap, rc::Rc, sync::OnceLock};

use foldhash::fast::RandomState;

use crate::lang::data::typ;

use super::ast;

/// Immutable syntax templates, with no arena-relative state.
#[derive(Default)]
pub(crate) struct TypeTemplates {
    templates: HashMap<(*const ast::Var, ast::Iter), OnceLock<Rc<ast::TypKind>>, RandomState>,
}

impl TypeTemplates {
    /// Discards registrations before the owning syntax can move.
    pub(crate) fn clear(&mut self) {
        self.templates.clear();
    }

    /// Registers output variables without constructing any type contents.
    pub(crate) fn register_vars(&mut self, vars: &[ast::Var], iter: ast::Iter) {
        for var in vars {
            self.templates
                .entry((std::ptr::from_ref(var), iter))
                .or_default();
        }
    }

    /// Registers iteration outputs only through shapes accepted by assignment.
    pub(crate) fn register_pattern(&mut self, exp: &ast::Exp) {
        stacker::maybe_grow(64 * 1024, 1024 * 1024, || match &exp.node {
            ast::ExpKind::Iter(exp_inner, exp_iter) => {
                self.register_vars(&exp_iter.vars, exp_iter.iter);
                self.register_pattern(exp_inner);
            }
            ast::ExpKind::Tuple(exps) | ast::ExpKind::List(exps) => {
                for exp in exps {
                    self.register_pattern(exp);
                }
            }
            ast::ExpKind::Case(not_exp) => {
                for exp in not_exp.args() {
                    self.register_pattern(exp);
                }
            }
            ast::ExpKind::Str(exp_fields) => {
                for exp_field in exp_fields {
                    self.register_pattern(&exp_field.exp);
                }
            }
            ast::ExpKind::Opt(Some(exp)) => self.register_pattern(exp),
            ast::ExpKind::Cons(exp_head, exp_tail) => {
                self.register_pattern(exp_head);
                self.register_pattern(exp_tail);
            }
            _ => {}
        });
    }

    /// Builds a registered variable's immutable output template on first use.
    pub(crate) fn get(&self, var: &ast::Var, iter: ast::Iter) -> Option<&Rc<ast::TypKind>> {
        let template = self.templates.get(&(std::ptr::from_ref(var), iter))?;
        Some(template.get_or_init(|| {
            let typ = typ::make::iterate(var.var.typ.clone(), &var.var.iters);
            Rc::new(typ::make::iterate(typ, &[iter]).node)
        }))
    }
}
