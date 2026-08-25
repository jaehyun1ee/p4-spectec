use std::fmt;

use crate::lang::xl::num;

use super::ast::*;

fn join<T>(items: &[T], separator: &str, string_of: impl Fn(&T) -> String) -> String {
    items
        .iter()
        .map(string_of)
        .collect::<Vec<_>>()
        .join(separator)
}

// Numbers

pub fn string_of_num(number: &Num) -> String {
    match number {
        num::Number::Nat(number) => number.to_string(),
        num::Number::Int(number) if number.sign() == num_bigint::Sign::Minus => {
            format!("-{}", -number)
        }
        num::Number::Int(number) => format!("+{number}"),
    }
}

// Texts

pub fn string_of_text(text: &str) -> String {
    text.to_owned()
}

// Identifiers

pub fn string_of_varid(id: &Id) -> String {
    id.node.clone()
}

pub fn string_of_typid(id: &Id) -> String {
    id.node.clone()
}

pub fn string_of_relid(id: &Id) -> String {
    id.node.clone()
}

pub fn string_of_ruleid(id: &Id) -> String {
    if id.node.is_empty() {
        String::new()
    } else {
        format!("/{}", id.node)
    }
}

pub fn string_of_defid(id: &Id) -> String {
    format!("${}", id.node)
}

// Atoms

pub fn string_of_atom(atom: &Atom) -> String {
    atom.node.source_string()
}

// Iterators

pub fn string_of_iter(iter: Iter) -> String {
    match iter {
        Iter::Opt => "?".into(),
        Iter::List => "*".into(),
    }
}

// Types

pub fn string_of_typ(typ: &Typ) -> String {
    match typ {
        Typ::PlainT(plain_typ) => string_of_plaintyp(plain_typ),
        Typ::NotationT(not_typ) => string_of_nottyp(not_typ),
    }
}

pub fn string_of_typs(separator: &str, typs: &[Typ]) -> String {
    join(typs, separator, string_of_typ)
}

pub fn string_of_plaintyp(plain_typ: &PlainTyp) -> String {
    match &plain_typ.node {
        PlainTypKind::BoolT => "bool".into(),
        PlainTypKind::NumT(num::Typ::NatT) => "nat".into(),
        PlainTypKind::NumT(num::Typ::IntT) => "int".into(),
        PlainTypKind::TextT => "text".into(),
        PlainTypKind::VarT(id, targs) => {
            format!("{}{}", string_of_typid(id), string_of_targs(targs))
        }
        PlainTypKind::ParenT(plain_typ) => format!("({})", string_of_plaintyp(plain_typ)),
        PlainTypKind::TupleT(plain_typs) => format!("({})", string_of_plaintyps(", ", plain_typs)),
        PlainTypKind::IterT(plain_typ, iter) => {
            format!("{}{}", string_of_plaintyp(plain_typ), string_of_iter(*iter))
        }
    }
}

pub fn string_of_plaintyps(separator: &str, plain_typs: &[PlainTyp]) -> String {
    join(plain_typs, separator, string_of_plaintyp)
}

pub fn string_of_nottyp(not_typ: &NotTyp) -> String {
    match &not_typ.node {
        NotTypKind::AtomT(atom) => string_of_atom(atom),
        NotTypKind::SeqT(typs) => string_of_typs(" ", typs),
        NotTypKind::InfixT(left, atom, right) => format!(
            "{} {} {}",
            string_of_typ(left),
            string_of_atom(atom),
            string_of_typ(right)
        ),
        NotTypKind::BrackT(left, typ, right) => format!(
            "`{}{}{}",
            string_of_atom(left),
            string_of_typ(typ),
            string_of_atom(right)
        ),
    }
}

pub fn string_of_nottyps(separator: &str, not_typs: &[NotTyp]) -> String {
    join(not_typs, separator, string_of_nottyp)
}

pub fn string_of_deftyp(def_typ: &DefTyp) -> String {
    match &def_typ.node {
        DefTypKind::PlainTD(plain_typ) => string_of_plaintyp(plain_typ),
        DefTypKind::StructTD(fields) => format!("{{{}}}", string_of_typfields(", ", fields)),
        DefTypKind::VariantTD(cases) => format!("\n   | {}", string_of_typcases("\n   | ", cases)),
    }
}

pub fn string_of_typfield(field: &TypField) -> String {
    format!(
        "{} {}",
        string_of_atom(&field.atom),
        string_of_plaintyp(&field.typ)
    )
}

pub fn string_of_typfields(separator: &str, fields: &[TypField]) -> String {
    join(fields, separator, string_of_typfield)
}

pub fn string_of_typcase(case: &TypCase) -> String {
    string_of_typ(&case.0)
}

pub fn string_of_typcases(separator: &str, cases: &[TypCase]) -> String {
    join(cases, separator, string_of_typcase)
}

// Operators

pub fn string_of_unop(operator: UnOp) -> &'static str {
    match operator {
        UnOp::NotOp => "~",
        UnOp::PlusOp => "+",
        UnOp::MinusOp => "-",
    }
}

pub fn string_of_binop(operator: BinOp) -> &'static str {
    match operator {
        BinOp::AndOp => "/\\",
        BinOp::OrOp => "\\/",
        BinOp::ImplOp => "=>",
        BinOp::EquivOp => "<=>",
        BinOp::AddOp => "+",
        BinOp::SubOp => "-",
        BinOp::MulOp => "*",
        BinOp::DivOp => "/",
        BinOp::ModOp => "\\",
        BinOp::PowOp => "^",
    }
}

pub fn string_of_cmpop(operator: CmpOp) -> &'static str {
    match operator {
        CmpOp::EqOp => "=",
        CmpOp::NeOp => "=/=",
        CmpOp::LtOp => "<",
        CmpOp::GtOp => ">",
        CmpOp::LeOp => "<=",
        CmpOp::GeOp => ">=",
    }
}

fn escaped(text: &str) -> String {
    text.bytes()
        .map(|byte| match byte {
            b'"' => "\\\"".into(),
            b'\\' => "\\\\".into(),
            8 => "\\b".into(),
            9 => "\\t".into(),
            10 => "\\n".into(),
            13 => "\\r".into(),
            32..=126 => char::from(byte).to_string(),
            _ => format!("\\{byte:03}"),
        })
        .collect()
}

// Expressions

pub fn string_of_exp(exp: &Exp) -> String {
    let mut output = String::new();
    write_exp(&mut output, exp).expect("writing to a String cannot fail");
    output
}

fn write_exp(output: &mut dyn fmt::Write, exp: &Exp) -> fmt::Result {
    match &exp.node {
        ExpKind::BoolE(value) => write!(output, "{value}"),
        ExpKind::NumE(NumOp::DecOp, num::Number::Nat(number)) => write!(output, "{number}"),
        ExpKind::NumE(NumOp::HexOp, num::Number::Nat(number)) => {
            write!(
                output,
                "0x{}",
                number.as_bigint().to_str_radix(16).to_uppercase()
            )
        }
        ExpKind::NumE(_, number) => output.write_str(&string_of_num(number)),
        ExpKind::TextE(text) => write!(output, "\"{}\"", escaped(text)),
        ExpKind::VarE(id) => output.write_str(&id.node),
        ExpKind::UnE(operator, exp) => {
            output.write_str(string_of_unop(*operator))?;
            write_exp(output, exp)
        }
        ExpKind::BinE(left, operator, right) => {
            write_exp(output, left)?;
            write!(output, " {} ", string_of_binop(*operator))?;
            write_exp(output, right)
        }
        ExpKind::CmpE(left, operator, right) => {
            write_exp(output, left)?;
            write!(output, " {} ", string_of_cmpop(*operator))?;
            write_exp(output, right)
        }
        ExpKind::ArithE(exp) => {
            output.write_str("$(")?;
            write_exp(output, exp)?;
            output.write_char(')')
        }
        ExpKind::EpsE => output.write_str("eps"),
        ExpKind::ListE(exps) => {
            output.write_char('[')?;
            write_exps(output, ", ", exps)?;
            output.write_char(']')
        }
        ExpKind::ConsE(left, right) => {
            write_exp(output, left)?;
            output.write_str(" :: ")?;
            write_exp(output, right)
        }
        ExpKind::CatE(left, right) => {
            write_exp(output, left)?;
            output.write_str(" ++ ")?;
            write_exp(output, right)
        }
        ExpKind::IdxE(base, index) => {
            write_exp(output, base)?;
            output.write_char('[')?;
            write_exp(output, index)?;
            output.write_char(']')
        }
        ExpKind::SliceE(base, low, high) => {
            write_exp(output, base)?;
            output.write_char('[')?;
            write_exp(output, low)?;
            output.write_str(" : ")?;
            write_exp(output, high)?;
            output.write_char(']')
        }
        ExpKind::LenE(exp) => {
            output.write_char('|')?;
            write_exp(output, exp)?;
            output.write_char('|')
        }
        ExpKind::MemE(left, right) => {
            write_exp(output, left)?;
            output.write_str(" <- ")?;
            write_exp(output, right)
        }
        ExpKind::StrE(fields) => {
            output.write_char('{')?;
            for (index, (atom, exp)) in fields.iter().enumerate() {
                if index != 0 {
                    output.write_str(", ")?;
                }
                write!(output, "{} ", string_of_atom(atom))?;
                write_exp(output, exp)?;
            }
            output.write_char('}')
        }
        ExpKind::DotE(exp, atom) => {
            write_exp(output, exp)?;
            write!(output, ".{}", string_of_atom(atom))
        }
        ExpKind::UpdE(base, path, field) => {
            write_exp(output, base)?;
            output.write_char('[')?;
            write_path(output, path)?;
            output.write_str(" = ")?;
            write_exp(output, field)?;
            output.write_char(']')
        }
        ExpKind::ParenE(exp) => {
            output.write_char('(')?;
            write_exp(output, exp)?;
            output.write_char(')')
        }
        ExpKind::TupleE(exps) => {
            output.write_char('(')?;
            write_exps(output, ", ", exps)?;
            output.write_char(')')
        }
        ExpKind::CallE(id, targs, args) => {
            output.write_str(&string_of_defid(id))?;
            write_targs(output, targs)?;
            write_args(output, args)
        }
        ExpKind::IterE(exp, iter) => {
            write_exp(output, exp)?;
            output.write_str(&string_of_iter(*iter))
        }
        ExpKind::SubE(exp, plain_typ) => {
            write_exp(output, exp)?;
            write!(output, " <: {}", string_of_plaintyp(plain_typ))
        }
        ExpKind::AtomE(atom) => output.write_str(&string_of_atom(atom)),
        ExpKind::SeqE(exps) => write_exps(output, " ", exps),
        ExpKind::InfixE(left, atom, right) => {
            write_exp(output, left)?;
            write!(output, " {} ", string_of_atom(atom))?;
            write_exp(output, right)
        }
        ExpKind::BrackE(left, exp, right) => {
            write!(output, "`{}", string_of_atom(left))?;
            write_exp(output, exp)?;
            output.write_str(&string_of_atom(right))
        }
        ExpKind::HoleE(Hole::Num(number)) => write!(output, "%{number}"),
        ExpKind::HoleE(Hole::Next) => output.write_char('%'),
        ExpKind::HoleE(Hole::Rest) => output.write_str("%%"),
        ExpKind::HoleE(Hole::None) => output.write_str("!%"),
        ExpKind::FuseE(left, right) => {
            write_exp(output, left)?;
            output.write_char('#')?;
            write_exp(output, right)
        }
        ExpKind::UnparenE(exp) => {
            output.write_str("##")?;
            write_exp(output, exp)
        }
        ExpKind::LatexE(text) => write!(output, "latex({})", escaped(text)),
    }
}

pub fn string_of_exps(separator: &str, exps: &[Exp]) -> String {
    let mut output = String::new();
    write_exps(&mut output, separator, exps).expect("writing to a String cannot fail");
    output
}

fn write_exps(output: &mut dyn fmt::Write, separator: &str, exps: &[Exp]) -> fmt::Result {
    for (index, exp) in exps.iter().enumerate() {
        if index != 0 {
            output.write_str(separator)?;
        }
        write_exp(output, exp)?;
    }
    Ok(())
}

// Paths

pub fn string_of_path(path: &Path) -> String {
    let mut output = String::new();
    write_path(&mut output, path).expect("writing to a String cannot fail");
    output
}

fn write_path(output: &mut dyn fmt::Write, path: &Path) -> fmt::Result {
    match &path.node {
        PathKind::RootP => Ok(()),
        PathKind::IdxP(path, exp) => {
            write_path(output, path)?;
            output.write_char('[')?;
            write_exp(output, exp)?;
            output.write_char(']')
        }
        PathKind::SliceP(path, low, high) => {
            write_path(output, path)?;
            output.write_char('[')?;
            write_exp(output, low)?;
            output.write_str(" : ")?;
            write_exp(output, high)?;
            output.write_char(']')
        }
        PathKind::DotP(path, atom) if matches!(path.node, PathKind::RootP) => {
            output.write_str(&string_of_atom(atom))
        }
        PathKind::DotP(path, atom) => {
            write_path(output, path)?;
            write!(output, ".{}", string_of_atom(atom))
        }
    }
}

// Parameters

pub fn string_of_param(param: &Param) -> String {
    match &param.node {
        ParamKind::ExpP(plain_typ) => string_of_plaintyp(plain_typ),
        ParamKind::DefP(id, tparams, params, plain_typ) => format!(
            "{}{}{} : {}",
            string_of_defid(id),
            string_of_tparams(tparams),
            string_of_params(params),
            string_of_plaintyp(plain_typ)
        ),
    }
}

pub fn string_of_params(params: &[Param]) -> String {
    let mut output = String::new();
    write_params(&mut output, params).expect("writing to a String cannot fail");
    output
}

fn write_params(output: &mut dyn fmt::Write, params: &[Param]) -> fmt::Result {
    if params.is_empty() {
        Ok(())
    } else {
        output.write_char('(')?;
        for (index, param) in params.iter().enumerate() {
            if index != 0 {
                output.write_str(", ")?;
            }
            output.write_str(&string_of_param(param))?;
        }
        output.write_char(')')
    }
}

// Type parameters

pub fn string_of_tparam(tparam: &TParam) -> String {
    tparam.node.clone()
}

pub fn string_of_tparams(tparams: &[TParam]) -> String {
    let mut output = String::new();
    write_tparams(&mut output, tparams).expect("writing to a String cannot fail");
    output
}

fn write_tparams(output: &mut dyn fmt::Write, tparams: &[TParam]) -> fmt::Result {
    if tparams.is_empty() {
        Ok(())
    } else {
        output.write_char('<')?;
        for (index, tparam) in tparams.iter().enumerate() {
            if index != 0 {
                output.write_str(", ")?;
            }
            output.write_str(&string_of_tparam(tparam))?;
        }
        output.write_char('>')
    }
}

// Arguments

pub fn string_of_arg(arg: &Arg) -> String {
    let mut output = String::new();
    write_arg(&mut output, arg).expect("writing to a String cannot fail");
    output
}

fn write_arg(output: &mut dyn fmt::Write, arg: &Arg) -> fmt::Result {
    match &arg.node {
        ArgKind::ExpA(exp) => write_exp(output, exp),
        ArgKind::DefA(id) => output.write_str(&string_of_defid(id)),
    }
}

pub fn string_of_args(args: &[Arg]) -> String {
    let mut output = String::new();
    write_args(&mut output, args).expect("writing to a String cannot fail");
    output
}

fn write_args(output: &mut dyn fmt::Write, args: &[Arg]) -> fmt::Result {
    if args.is_empty() {
        Ok(())
    } else {
        output.write_char('(')?;
        for (index, arg) in args.iter().enumerate() {
            if index != 0 {
                output.write_str(", ")?;
            }
            write_arg(output, arg)?;
        }
        output.write_char(')')
    }
}

// Type arguments

pub fn string_of_targ(targ: &Targ) -> String {
    string_of_plaintyp(targ)
}

pub fn string_of_targs(targs: &[Targ]) -> String {
    let mut output = String::new();
    write_targs(&mut output, targs).expect("writing to a String cannot fail");
    output
}

fn write_targs(output: &mut dyn fmt::Write, targs: &[Targ]) -> fmt::Result {
    if targs.is_empty() {
        Ok(())
    } else {
        output.write_char('<')?;
        for (index, targ) in targs.iter().enumerate() {
            if index != 0 {
                output.write_str(", ")?;
            }
            output.write_str(&string_of_targ(targ))?;
        }
        output.write_char('>')
    }
}

// Premises

pub fn string_of_prem(prem: &Prem) -> String {
    let mut output = String::new();
    write_prem(&mut output, prem).expect("writing to a String cannot fail");
    output
}

fn write_prem(output: &mut dyn fmt::Write, prem: &Prem) -> fmt::Result {
    match &prem.node {
        PremKind::VarPr(id, plain_typ) => write!(
            output,
            "{} : {}",
            string_of_varid(id),
            string_of_plaintyp(plain_typ)
        ),
        PremKind::RulePr(id, exp) => {
            write!(output, "{}: ", string_of_relid(id))?;
            write_exp(output, exp)
        }
        PremKind::RuleNotPr(id, exp) => {
            write!(output, "{}:/ ", string_of_relid(id))?;
            write_exp(output, exp)
        }
        PremKind::IfPr(exp) => {
            output.write_str("if ")?;
            write_exp(output, exp)
        }
        PremKind::ElsePr => output.write_str("otherwise"),
        PremKind::IterPr(inner, iter) if matches!(inner.node, PremKind::IterPr(_, _)) => {
            write_prem(output, inner)?;
            output.write_str(&string_of_iter(*iter))
        }
        PremKind::IterPr(inner, iter) => {
            output.write_char('(')?;
            write_prem(output, inner)?;
            write!(output, "){}", string_of_iter(*iter))
        }
        PremKind::DebugPr(exp) => {
            output.write_str("debug ")?;
            write_exp(output, exp)
        }
    }
}

pub fn string_of_prems(prems: &[Prem]) -> String {
    let mut output = String::new();
    write_prems(&mut output, prems).expect("writing to a String cannot fail");
    output
}

fn write_prems(output: &mut dyn fmt::Write, prems: &[Prem]) -> fmt::Result {
    for prem in prems {
        output.write_str("\n -- ")?;
        write_prem(output, prem)?;
    }
    Ok(())
}

// Rules

pub fn string_of_rule(rule: &Rule) -> String {
    let mut output = String::new();
    write_rule(&mut output, rule).expect("writing to a String cannot fail");
    output
}

fn write_rule(output: &mut dyn fmt::Write, rule: &Rule) -> fmt::Result {
    write!(
        output,
        "rule {}{}:\n  ",
        string_of_relid(&rule.node.relation_id),
        string_of_ruleid(&rule.node.rule_id)
    )?;
    write_exp(output, &rule.node.expression)?;
    write_prems(output, &rule.node.premises)
}

pub fn string_of_rules(rules: &[Rule]) -> String {
    let mut output = String::new();
    for (index, rule) in rules.iter().enumerate() {
        if index != 0 {
            output.push('\n');
        }
        write_rule(&mut output, rule).expect("writing to a String cannot fail");
    }
    output
}

// Tables

pub fn string_of_tablerow(table_row: &TableRow) -> String {
    let mut output = String::new();
    write_tablerow(&mut output, table_row).expect("writing to a String cannot fail");
    output
}

fn write_tablerow(output: &mut dyn fmt::Write, table_row: &TableRow) -> fmt::Result {
    write_exp(output, &table_row.node.0)?;
    output.write_str(" => ")?;
    write_exp(output, &table_row.node.1)
}

pub fn string_of_tablerows(table_rows: &[TableRow]) -> String {
    let mut output = String::new();
    for (index, row) in table_rows.iter().enumerate() {
        if index != 0 {
            output.push_str("\n  | ");
        }
        write_tablerow(&mut output, row).expect("writing to a String cannot fail");
    }
    output
}

// Definitions

pub fn string_of_def(definition: &Def) -> String {
    let mut output = String::new();
    write_def(&mut output, definition).expect("writing to a String cannot fail");
    output
}

fn write_def(output: &mut dyn fmt::Write, definition: &Def) -> fmt::Result {
    match &definition.node {
        DefKind::ExternSynD(id, _) => write!(output, "extern syntax {}", string_of_typid(id)),
        DefKind::SynD(syntaxes) => write!(
            output,
            "syntax {}",
            join(syntaxes, ", ", |(id, tparams)| format!(
                "{}{}",
                string_of_typid(id),
                string_of_tparams(tparams)
            ))
        ),
        DefKind::TypD(id, tparams, def_typ, _) => write!(
            output,
            "syntax {}{} = {}",
            string_of_typid(id),
            string_of_tparams(tparams),
            string_of_deftyp(def_typ)
        ),
        DefKind::VarD(id, plain_typ, _) => write!(
            output,
            "var {} : {}",
            string_of_varid(id),
            string_of_plaintyp(plain_typ)
        ),
        DefKind::ExternRelD(id, not_typ, _) => write!(
            output,
            "extern relation {}: {}",
            string_of_relid(id),
            string_of_nottyp(not_typ)
        ),
        DefKind::RelD(id, not_typ, _) => write!(
            output,
            "relation {}: {}",
            string_of_relid(id),
            string_of_nottyp(not_typ)
        ),
        DefKind::RuleGroupD(relid, groupid, rules) => {
            write!(
                output,
                "rulegroup {}{}:\n  ",
                string_of_relid(relid),
                string_of_ruleid(groupid)
            )?;
            for (index, rule) in rules.iter().enumerate() {
                if index != 0 {
                    output.write_str("\n  ")?;
                }
                write_rule(output, rule)?;
            }
            Ok(())
        }
        DefKind::ExternDecD(id, tparams, params, plain_typ, _) => write!(
            output,
            "extern dec {}{}{} : {}",
            string_of_defid(id),
            string_of_tparams(tparams),
            string_of_params(params),
            string_of_plaintyp(plain_typ)
        ),
        DefKind::BuiltinDecD(id, tparams, params, plain_typ, _) => write!(
            output,
            "builtin dec {}{}{} : {}",
            string_of_defid(id),
            string_of_tparams(tparams),
            string_of_params(params),
            string_of_plaintyp(plain_typ)
        ),
        DefKind::TableDecD(id, params, plain_typ, _) => write!(
            output,
            "tbl dec {}{} : {}",
            string_of_defid(id),
            string_of_params(params),
            string_of_plaintyp(plain_typ)
        ),
        DefKind::FuncDecD(id, tparams, params, plain_typ, _) => write!(
            output,
            "dec {}{}{} : {}",
            string_of_defid(id),
            string_of_tparams(tparams),
            string_of_params(params),
            string_of_plaintyp(plain_typ)
        ),
        DefKind::TableDefD(id, rows) => {
            write!(output, "tbl def {} =\n  ", string_of_defid(id))?;
            for (index, row) in rows.iter().enumerate() {
                if index != 0 {
                    output.write_str("\n  | ")?;
                }
                write_exp(output, &row.node.0)?;
                output.write_str(" => ")?;
                write_exp(output, &row.node.1)?;
            }
            Ok(())
        }
        DefKind::FuncDefD(id, tparams, args, exp, prems) => {
            write!(output, "def {}", string_of_defid(id))?;
            write_tparams(output, tparams)?;
            write_args(output, args)?;
            output.write_str(" = ")?;
            write_exp(output, exp)?;
            write_prems(output, prems)
        }
        DefKind::SepD => output.write_str("\n\n"),
    }
}

// Spec

/// Renders a specification without source or hint metadata
pub fn string_of_spec(spec: &Spec) -> String {
    let mut output = String::new();
    write_spec(&mut output, spec).expect("writing to a String cannot fail");
    output
}

fn write_spec(output: &mut dyn fmt::Write, spec: &Spec) -> fmt::Result {
    for definition in spec {
        write_def(output, definition)?;
        output.write_char('\n')?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::source::{Region, Spanned};

    fn id(name: &str) -> Id {
        Spanned::new(name.to_owned(), Region::none())
    }

    fn exp(kind: ExpKind) -> Exp {
        Spanned::new(kind, Region::none())
    }

    #[test]
    fn expression_sink_preserves_escaping_and_recursive_precedence() {
        let expression = exp(ExpKind::BinE(
            Box::new(exp(ExpKind::TextE("a\n\\\"".to_owned()))),
            BinOp::AddOp,
            Box::new(exp(ExpKind::VarE(id("right")))),
        ));
        let mut output = String::new();

        write_exp(&mut output, &expression).unwrap();

        assert_eq!(output, "\"a\\n\\\\\\\"\" + right");
    }
}
