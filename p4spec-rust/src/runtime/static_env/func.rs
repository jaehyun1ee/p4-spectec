use crate::lang::il::ast;

/// A statically known function declaration
#[derive(Clone, Debug, PartialEq)]
pub enum Function {
    Extern {
        type_parameters: Vec<ast::TParam>,
        parameters: Vec<ast::Param>,
        result_type: Box<ast::Typ>,
    },
    Builtin {
        type_parameters: Vec<ast::TParam>,
        parameters: Vec<ast::Param>,
        result_type: Box<ast::Typ>,
    },
    Table {
        parameters: Vec<ast::Param>,
        result_type: Box<ast::Typ>,
        rows: Vec<ast::TableRow>,
    },
    Defined {
        type_parameters: Vec<ast::TParam>,
        parameters: Vec<ast::Param>,
        result_type: Box<ast::Typ>,
        clauses: Vec<ast::Clause>,
        else_clause: Option<Box<ast::ElseClause>>,
    },
}
