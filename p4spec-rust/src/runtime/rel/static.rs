//! Static relation definitions used during elaboration

use crate::lang::{hints::input::InputHint, il::ast};

/// Static representation of a relation
#[derive(Clone, Debug, PartialEq)]
pub enum Rel {
    Extern {
        not_typ: Box<ast::NotTyp>,
        input_hint: InputHint,
    },
    Defined {
        not_typ: Box<ast::NotTyp>,
        input_hint: InputHint,
        rule_groups: Vec<ast::RuleGroup>,
        else_group: Option<Box<ast::ElseGroup>>,
    },
}
