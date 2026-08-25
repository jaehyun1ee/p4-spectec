pub type Hint = crate::lang::el::ast::Exp;

pub fn to_string(hint: &Hint) -> String {
    crate::lang::el::print::string_of_exp(hint)
}
