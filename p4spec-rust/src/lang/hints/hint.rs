pub type T = crate::lang::el::ast::Exp;
pub fn to_string(hint: &T) -> String {
    crate::lang::el::print::string_of_exp(hint)
}
