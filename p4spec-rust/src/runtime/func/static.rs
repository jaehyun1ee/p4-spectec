//! Static function definitions used during elaboration

use crate::lang::il::ast;

/// Static representation of a function
#[derive(Clone, Debug, PartialEq)]
pub enum Func {
    Extern {
        tparams: Vec<ast::TParam>,
        params: Vec<ast::Param>,
        typ_ret: Box<ast::Typ>,
    },
    Builtin {
        tparams: Vec<ast::TParam>,
        params: Vec<ast::Param>,
        typ_ret: Box<ast::Typ>,
    },
    Table {
        params: Vec<ast::Param>,
        typ_ret: Box<ast::Typ>,
        table_rows: Vec<ast::TableRow>,
    },
    Defined {
        tparams: Vec<ast::TParam>,
        params: Vec<ast::Param>,
        typ_ret: Box<ast::Typ>,
        clauses: Vec<ast::Clause>,
        else_clause: Option<Box<ast::ElseClause>>,
    },
}
