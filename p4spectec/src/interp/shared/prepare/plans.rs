//! Execution metadata for immutable syntax retained by a Global
//!
//! Constructor programs contain no arena handles.
//! Condition text is printed once on its first failed evaluation;
//! only actual stored SL expressions are registered.

use std::{collections::HashMap, sync::OnceLock};

use foldhash::fast::RandomState;

use crate::lang::traits::print::Print;

use super::{ast, construct::ConstructPlans, type_templates::TypeTemplates};

/// Groups syntax metadata whose addresses share the owning Global's lifetime.
#[derive(Default)]
pub(crate) struct EvalPlans {
    pub(crate) constructs: ConstructPlans,
    pub(crate) types: TypeTemplates,
    texts: HashMap<*const ast::Exp, OnceLock<String>, RandomState>,
}

impl EvalPlans {
    /// Discards registrations before the owning syntax can move.
    pub(crate) fn clear(&mut self) {
        self.constructs.clear();
        self.types.clear();
        self.texts.clear();
    }

    /// Registers a stored condition without printing or evaluating it.
    pub(crate) fn register_condition(&mut self, exp: &ast::Exp) {
        self.texts.entry(std::ptr::from_ref(exp)).or_default();
    }

    /// Prints a registered condition on first use and borrows its stable text.
    pub(crate) fn condition_text(&self, exp: &ast::Exp) -> Option<&str> {
        self.texts
            .get(&std::ptr::from_ref(exp))
            .map(|text| text.get_or_init(|| Print::to_string(exp)).as_str())
    }
}
