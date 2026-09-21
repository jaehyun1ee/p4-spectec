//! Prose rendering hints indexed by their definition

use std::collections::BTreeMap;

use crate::lang::{common::notation::mixop::Mixop, pl::annot::Hints, sl::ast::Id};

#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
enum HintKey {
    Case(String, Mixop),
    Func(String),
    Rel(String),
}

#[derive(Debug, Default)]
pub struct HEnv(BTreeMap<HintKey, Hints>);

impl HEnv {
    pub fn insert_case(&mut self, id_typ: &Id, mixop: &Mixop, hints: Hints) {
        let key = HintKey::Case(id_typ.node.clone(), mixop.clone());
        self.0.insert(key, hints);
    }

    pub fn insert_func(&mut self, id_func: &Id, hints: Hints) {
        let key = HintKey::Func(id_func.node.clone());
        self.0.insert(key, hints);
    }

    pub fn insert_rel(&mut self, id_rel: &Id, hints: Hints) {
        let key = HintKey::Rel(id_rel.node.clone());
        self.0.insert(key, hints);
    }

    pub fn get_case(&self, id_typ: &Id, mixop: &Mixop) -> Option<&Hints> {
        let key = HintKey::Case(id_typ.node.clone(), mixop.clone());
        self.0.get(&key)
    }

    pub fn get_func(&self, id_func: &Id) -> Option<&Hints> {
        let key = HintKey::Func(id_func.node.clone());
        self.0.get(&key)
    }

    pub fn get_rel(&self, id_rel: &Id) -> Option<&Hints> {
        let key = HintKey::Rel(id_rel.node.clone());
        self.0.get(&key)
    }
}
