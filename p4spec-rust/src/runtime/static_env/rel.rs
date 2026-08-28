use crate::lang::{hints::input::InputHint, il::ast};

/// A statically known relation declaration
#[derive(Clone, Debug, PartialEq)]
pub enum Relation {
    Extern {
        notation_type: Box<ast::NotTyp>,
        input_hint: InputHint,
    },
    Defined {
        notation_type: Box<ast::NotTyp>,
        input_hint: InputHint,
        rule_groups: Vec<ast::RuleGroup>,
        else_group: Option<Box<ast::ElseGroup>>,
    },
}
