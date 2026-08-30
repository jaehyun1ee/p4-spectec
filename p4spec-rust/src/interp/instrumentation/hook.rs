use crate::{
    lang::{il, sl},
    runtime::value::{Value, ValueRef},
};

use super::Handler;

#[derive(Default)]
pub struct Hook {
    handlers: Vec<Box<dyn Handler>>,
}

impl Hook {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, handler: Box<dyn Handler>) {
        self.handlers.push(handler);
    }

    pub fn is_active(&self) -> bool {
        !self.handlers.is_empty()
    }

    pub fn finish(&mut self) {
        for handler in &mut self.handlers {
            handler.finish();
        }
    }

    pub fn backup(&mut self) {
        for handler in &mut self.handlers {
            handler.backup();
        }
    }

    pub fn restore(&mut self) {
        for handler in &mut self.handlers {
            handler.restore();
        }
    }

    pub fn on_program(&mut self, value: &ValueRef) {
        for handler in &mut self.handlers {
            handler.on_program(value);
        }
    }

    pub fn on_value(&mut self, value: &ValueRef) {
        for handler in &mut self.handlers {
            handler.on_value(value);
        }
    }

    pub fn on_rel_enter(&mut self, id: &il::ast::Id, values: &[ValueRef]) {
        for handler in &mut self.handlers {
            handler.on_rel_enter(id, values);
        }
    }

    pub fn on_rel_exit(&mut self, id: &il::ast::Id) {
        for handler in &mut self.handlers {
            handler.on_rel_exit(id);
        }
    }

    pub fn on_func_enter(&mut self, id: &il::ast::Id, values: &[ValueRef]) {
        for handler in &mut self.handlers {
            handler.on_func_enter(id, values);
        }
    }

    pub fn on_func_exit(&mut self, id: &il::ast::Id) {
        for handler in &mut self.handlers {
            handler.on_func_exit(id);
        }
    }

    pub fn on_prem(&mut self, prem: &il::ast::Prem) {
        for handler in &mut self.handlers {
            handler.on_prem(prem);
        }
    }

    pub fn on_instr(&mut self, instr: &sl::ast::Instr) {
        for handler in &mut self.handlers {
            handler.on_instr(instr);
        }
    }

    pub fn on_instr_dangling(&mut self, condition: bool, iid: sl::ast::Iid, value: &Value) {
        for handler in &mut self.handlers {
            handler.on_instr_dangling(condition, iid, value);
        }
    }
}
