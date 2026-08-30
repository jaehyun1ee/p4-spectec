use crate::{
    lang::{il, sl},
    runtime::value::{Value, ValueRef},
};

pub trait Handler {
    fn finish(&mut self) {}
    fn backup(&mut self) {}
    fn restore(&mut self) {}
    fn on_program(&mut self, _value: &ValueRef) {}
    fn on_value(&mut self, _value: &ValueRef) {}
    fn on_rel_enter(&mut self, _id: &il::ast::Id, _values: &[ValueRef]) {}
    fn on_rel_exit(&mut self, _id: &il::ast::Id) {}
    fn on_func_enter(&mut self, _id: &il::ast::Id, _values: &[ValueRef]) {}
    fn on_func_exit(&mut self, _id: &il::ast::Id) {}
    fn on_prem(&mut self, _prem: &il::ast::Prem) {}
    fn on_instr(&mut self, _instr: &sl::ast::Instr) {}
    fn on_instr_dangling(&mut self, _condition: bool, _iid: sl::ast::Iid, _value: &Value) {}
}
