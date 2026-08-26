//! Text rendering for elaboration-language data

use std::fmt;

use crate::lang::xl::num::string_of_num;
use crate::lang::xl::{bool, num};

use super::ast::*;

// - Texts

/// Renders text
pub fn string_of_text(text: &str) -> String {
    text.to_owned()
}

// - Identifiers

/// Renders varid
pub fn string_of_varid(id: &Id) -> String {
    id.node.clone()
}

/// Renders typid
pub fn string_of_typid(id: &Id) -> String {
    id.node.clone()
}

/// Renders relid
pub fn string_of_relid(id: &Id) -> String {
    id.node.clone()
}

/// Renders ruleid
pub fn string_of_ruleid(id: &Id) -> String {
    if id.node.is_empty() {
        String::new()
    } else {
        format!("/{}", id.node)
    }
}

/// Renders defid
pub fn string_of_defid(id: &Id) -> String {
    format!("${}", id.node)
}

// - Atoms

/// Renders atom
pub fn string_of_atom(atom: &Atom) -> String {
    atom.node.to_string()
}

// - Iterators

/// Renders iter
pub fn string_of_iter(iter: Iter) -> String {
    match iter {
        Iter::Opt => "?".into(),
        Iter::List => "*".into(),
    }
}

// - Types

/// Renders typ
pub fn string_of_typ(typ: &Typ) -> String {
    let mut output = String::new();
    write_typ(&mut output, typ).expect("writing to a String cannot fail");
    output
}

fn write_typ(output: &mut dyn fmt::Write, typ: &Typ) -> fmt::Result {
    match typ {
        Typ::Plain(plain_typ) => write_plain_typ(output, plain_typ),
        Typ::Notation(not_typ) => write_not_typ(output, not_typ),
    }
}

/// Renders typs
pub fn string_of_typs(sep: &str, typs: &[Typ]) -> String {
    let mut output = String::new();
    write_typs(&mut output, sep, typs).expect("writing to a String cannot fail");
    output
}

fn write_typs(output: &mut dyn fmt::Write, sep: &str, typs: &[Typ]) -> fmt::Result {
    for (index, typ) in typs.iter().enumerate() {
        if index != 0 {
            output.write_str(sep)?;
        }
        write_typ(output, typ)?;
    }
    Ok(())
}

// - Plain types

/// Renders plain typ
pub fn string_of_plain_typ(plain_typ: &PlainTyp) -> String {
    let mut output = String::new();
    write_plain_typ(&mut output, plain_typ).expect("writing to a String cannot fail");
    output
}

fn write_plain_typ(output: &mut dyn fmt::Write, plain_typ: &PlainTyp) -> fmt::Result {
    match &plain_typ.node {
        PlainTypKind::Bool => output.write_str("bool"),
        PlainTypKind::Num(num::Typ::Nat) => output.write_str("nat"),
        PlainTypKind::Num(num::Typ::Int) => output.write_str("int"),
        PlainTypKind::Text => output.write_str("text"),
        PlainTypKind::Var(id, targs) => {
            output.write_str(&string_of_typid(id))?;
            write_targs(output, targs)
        }
        PlainTypKind::Paren(plain_typ) => {
            output.write_char('(')?;
            write_plain_typ(output, plain_typ)?;
            output.write_char(')')
        }
        PlainTypKind::Tuple(plain_typs) => {
            output.write_char('(')?;
            write_plain_typs(output, ", ", plain_typs)?;
            output.write_char(')')
        }
        PlainTypKind::Iter(plain_typ, iter) => {
            write_plain_typ(output, plain_typ)?;
            output.write_str(&string_of_iter(*iter))
        }
    }
}

/// Renders plain typs
pub fn string_of_plain_typs(sep: &str, plain_typs: &[PlainTyp]) -> String {
    let mut output = String::new();
    write_plain_typs(&mut output, sep, plain_typs).expect("writing to a String cannot fail");
    output
}

fn write_plain_typs(
    output: &mut dyn fmt::Write,
    sep: &str,
    plain_typs: &[PlainTyp],
) -> fmt::Result {
    for (index, plain_typ) in plain_typs.iter().enumerate() {
        if index != 0 {
            output.write_str(sep)?;
        }
        write_plain_typ(output, plain_typ)?;
    }
    Ok(())
}

// - Notation types

/// Renders not typ
pub fn string_of_not_typ(not_typ: &NotTyp) -> String {
    let mut output = String::new();
    write_not_typ(&mut output, not_typ).expect("writing to a String cannot fail");
    output
}

fn write_not_typ(output: &mut dyn fmt::Write, not_typ: &NotTyp) -> fmt::Result {
    match &not_typ.node {
        NotTypKind::Atom(atom) => output.write_str(&string_of_atom(atom)),
        NotTypKind::Seq(typs) => write_typs(output, " ", typs),
        NotTypKind::Infix(typ_l, atom, typ_r) => {
            write_typ(output, typ_l)?;
            write!(output, " {} ", string_of_atom(atom))?;
            write_typ(output, typ_r)
        }
        NotTypKind::Brack(atom_l, typ, atom_r) => {
            write!(output, "`{}", string_of_atom(atom_l))?;
            write_typ(output, typ)?;
            output.write_str(&string_of_atom(atom_r))
        }
    }
}

/// Renders not typs
pub fn string_of_not_typs(sep: &str, not_typs: &[NotTyp]) -> String {
    let mut output = String::new();
    write_not_typs(&mut output, sep, not_typs).expect("writing to a String cannot fail");
    output
}

fn write_not_typs(output: &mut dyn fmt::Write, sep: &str, not_typs: &[NotTyp]) -> fmt::Result {
    for (index, not_typ) in not_typs.iter().enumerate() {
        if index != 0 {
            output.write_str(sep)?;
        }
        write_not_typ(output, not_typ)?;
    }
    Ok(())
}

// - Defined types

/// Renders def typ
pub fn string_of_def_typ(def_typ: &DefTyp) -> String {
    let mut output = String::new();
    write_def_typ(&mut output, def_typ).expect("writing to a String cannot fail");
    output
}

fn write_def_typ(output: &mut dyn fmt::Write, def_typ: &DefTyp) -> fmt::Result {
    match &def_typ.node {
        DefTypKind::Plain(plain_typ) => write_plain_typ(output, plain_typ),
        DefTypKind::Struct(typ_fields) => {
            output.write_char('{')?;
            write_typ_fields(output, ", ", typ_fields)?;
            output.write_char('}')
        }
        DefTypKind::Variant(typ_cases) => {
            output.write_str("\n   | ")?;
            write_typ_cases(output, "\n   | ", typ_cases)
        }
    }
}

/// Renders typ field
pub fn string_of_typ_field(typ_field: &TypField) -> String {
    let mut output = String::new();
    write_typ_field(&mut output, typ_field).expect("writing to a String cannot fail");
    output
}

fn write_typ_field(output: &mut dyn fmt::Write, typ_field: &TypField) -> fmt::Result {
    write!(output, "{} ", string_of_atom(&typ_field.0))?;
    write_plain_typ(output, &typ_field.1)
}

/// Renders typ fields
pub fn string_of_typ_fields(sep: &str, typ_fields: &[TypField]) -> String {
    let mut output = String::new();
    write_typ_fields(&mut output, sep, typ_fields).expect("writing to a String cannot fail");
    output
}

fn write_typ_fields(
    output: &mut dyn fmt::Write,
    sep: &str,
    typ_fields: &[TypField],
) -> fmt::Result {
    for (index, typ_field) in typ_fields.iter().enumerate() {
        if index != 0 {
            output.write_str(sep)?;
        }
        write_typ_field(output, typ_field)?;
    }
    Ok(())
}

/// Renders typ case
pub fn string_of_typ_case(typ_case: &TypCase) -> String {
    let mut output = String::new();
    write_typ_case(&mut output, typ_case).expect("writing to a String cannot fail");
    output
}

fn write_typ_case(output: &mut dyn fmt::Write, typ_case: &TypCase) -> fmt::Result {
    write_typ(output, &typ_case.0)
}

/// Renders typ cases
pub fn string_of_typ_cases(sep: &str, typ_cases: &[TypCase]) -> String {
    let mut output = String::new();
    write_typ_cases(&mut output, sep, typ_cases).expect("writing to a String cannot fail");
    output
}

fn write_typ_cases(output: &mut dyn fmt::Write, sep: &str, typ_cases: &[TypCase]) -> fmt::Result {
    for (index, typ_case) in typ_cases.iter().enumerate() {
        if index != 0 {
            output.write_str(sep)?;
        }
        write_typ_case(output, typ_case)?;
    }
    Ok(())
}

// - Operators

/// Renders unop
pub fn string_of_unop(operator: UnOp) -> &'static str {
    match operator {
        UnOp::Bool(operator) => bool::string_of_unop(operator),
        UnOp::Num(operator) => num::string_of_unop(operator),
    }
}

/// Renders binop
pub fn string_of_binop(operator: BinOp) -> &'static str {
    match operator {
        BinOp::Bool(operator) => bool::string_of_binop(operator),
        BinOp::Num(operator) => num::string_of_binop(operator),
    }
}

/// Renders cmpop
pub fn string_of_cmpop(operator: CmpOp) -> &'static str {
    match operator {
        CmpOp::Bool(operator) => bool::string_of_cmpop(operator),
        CmpOp::Num(operator) => num::string_of_cmpop(operator),
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

// - Expressions

/// Renders exp
pub fn string_of_exp(exp: &Exp) -> String {
    let mut output = String::new();
    write_exp(&mut output, exp).expect("writing to a String cannot fail");
    output
}

fn write_exp(output: &mut dyn fmt::Write, exp: &Exp) -> fmt::Result {
    match &exp.node {
        ExpKind::Bool(value) => write!(output, "{value}"),
        ExpKind::Num(NumOp::Dec, num::Number::Nat(number)) => write!(output, "{number}"),
        ExpKind::Num(NumOp::Hex, num::Number::Nat(number)) => {
            write!(
                output,
                "0x{}",
                number.as_bigint().to_str_radix(16).to_uppercase()
            )
        }
        ExpKind::Num(_, number) => output.write_str(&string_of_num(number)),
        ExpKind::Text(text) => write!(output, "\"{}\"", escaped(text)),
        ExpKind::Var(id) => output.write_str(&id.node),
        ExpKind::Un(operator, exp) => {
            output.write_str(string_of_unop(*operator))?;
            write_exp(output, exp)
        }
        ExpKind::Bin(exp_l, operator, exp_r) => {
            write_exp(output, exp_l)?;
            write!(output, " {} ", string_of_binop(*operator))?;
            write_exp(output, exp_r)
        }
        ExpKind::Cmp(exp_l, operator, exp_r) => {
            write_exp(output, exp_l)?;
            write!(output, " {} ", string_of_cmpop(*operator))?;
            write_exp(output, exp_r)
        }
        ExpKind::Arith(exp) => {
            output.write_str("$(")?;
            write_exp(output, exp)?;
            output.write_char(')')
        }
        ExpKind::Eps => output.write_str("eps"),
        ExpKind::List(exps) => {
            output.write_char('[')?;
            write_exps(output, ", ", exps)?;
            output.write_char(']')
        }
        ExpKind::Cons(exp_l, exp_r) => {
            write_exp(output, exp_l)?;
            output.write_str(" :: ")?;
            write_exp(output, exp_r)
        }
        ExpKind::Cat(exp_l, exp_r) => {
            write_exp(output, exp_l)?;
            output.write_str(" ++ ")?;
            write_exp(output, exp_r)
        }
        ExpKind::Idx(exp_base, exp_index) => {
            write_exp(output, exp_base)?;
            output.write_char('[')?;
            write_exp(output, exp_index)?;
            output.write_char(']')
        }
        ExpKind::Slice(exp_base, exp_l, exp_r) => {
            write_exp(output, exp_base)?;
            output.write_char('[')?;
            write_exp(output, exp_l)?;
            output.write_str(" : ")?;
            write_exp(output, exp_r)?;
            output.write_char(']')
        }
        ExpKind::Len(exp) => {
            output.write_char('|')?;
            write_exp(output, exp)?;
            output.write_char('|')
        }
        ExpKind::Mem(exp_l, exp_r) => {
            write_exp(output, exp_l)?;
            output.write_str(" <- ")?;
            write_exp(output, exp_r)
        }
        ExpKind::Str(fields) => {
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
        ExpKind::Dot(exp, atom) => {
            write_exp(output, exp)?;
            write!(output, ".{}", string_of_atom(atom))
        }
        ExpKind::Upd(exp_base, path, exp_field) => {
            write_exp(output, exp_base)?;
            output.write_char('[')?;
            write_path(output, path)?;
            output.write_str(" = ")?;
            write_exp(output, exp_field)?;
            output.write_char(']')
        }
        ExpKind::Paren(exp) => {
            output.write_char('(')?;
            write_exp(output, exp)?;
            output.write_char(')')
        }
        ExpKind::Tuple(exps) => {
            output.write_char('(')?;
            write_exps(output, ", ", exps)?;
            output.write_char(')')
        }
        ExpKind::Call(id, targs, args) => {
            output.write_str(&string_of_defid(id))?;
            write_targs(output, targs)?;
            write_args(output, args)
        }
        ExpKind::Iter(exp, iter) => {
            write_exp(output, exp)?;
            output.write_str(&string_of_iter(*iter))
        }
        ExpKind::Sub(exp, plain_typ) => {
            write_exp(output, exp)?;
            output.write_str(" <:")?;
            output.write_char(' ')?;
            write_plain_typ(output, plain_typ)
        }
        ExpKind::Atom(atom) => output.write_str(&string_of_atom(atom)),
        ExpKind::Seq(exps) => write_exps(output, " ", exps),
        ExpKind::Infix(exp_l, atom, exp_r) => {
            write_exp(output, exp_l)?;
            write!(output, " {} ", string_of_atom(atom))?;
            write_exp(output, exp_r)
        }
        ExpKind::Brack(atom_l, exp, atom_r) => {
            write!(output, "`{}", string_of_atom(atom_l))?;
            write_exp(output, exp)?;
            output.write_str(&string_of_atom(atom_r))
        }
        ExpKind::Hole(Hole::Num(number)) => write!(output, "%{number}"),
        ExpKind::Hole(Hole::Next) => output.write_char('%'),
        ExpKind::Hole(Hole::Rest) => output.write_str("%%"),
        ExpKind::Hole(Hole::None) => output.write_str("!%"),
        ExpKind::Fuse(exp_l, exp_r) => {
            write_exp(output, exp_l)?;
            output.write_char('#')?;
            write_exp(output, exp_r)
        }
        ExpKind::Unparen(exp) => {
            output.write_str("##")?;
            write_exp(output, exp)
        }
        ExpKind::Latex(text) => write!(output, "latex({})", escaped(text)),
    }
}

/// Renders exps
pub fn string_of_exps(sep: &str, exps: &[Exp]) -> String {
    let mut output = String::new();
    write_exps(&mut output, sep, exps).expect("writing to a String cannot fail");
    output
}

fn write_exps(output: &mut dyn fmt::Write, sep: &str, exps: &[Exp]) -> fmt::Result {
    for (index, exp) in exps.iter().enumerate() {
        if index != 0 {
            output.write_str(sep)?;
        }
        write_exp(output, exp)?;
    }
    Ok(())
}

// - Paths

/// Renders path
pub fn string_of_path(path: &Path) -> String {
    let mut output = String::new();
    write_path(&mut output, path).expect("writing to a String cannot fail");
    output
}

fn write_path(output: &mut dyn fmt::Write, path: &Path) -> fmt::Result {
    match &path.node {
        PathKind::Root => Ok(()),
        PathKind::Idx(path, exp_index) => {
            write_path(output, path)?;
            output.write_char('[')?;
            write_exp(output, exp_index)?;
            output.write_char(']')
        }
        PathKind::Slice(path, exp_l, exp_r) => {
            write_path(output, path)?;
            output.write_char('[')?;
            write_exp(output, exp_l)?;
            output.write_str(" : ")?;
            write_exp(output, exp_r)?;
            output.write_char(']')
        }
        PathKind::Dot(path, atom) if matches!(path.node, PathKind::Root) => {
            output.write_str(&string_of_atom(atom))
        }
        PathKind::Dot(path, atom) => {
            write_path(output, path)?;
            write!(output, ".{}", string_of_atom(atom))
        }
    }
}

// - Parameters

/// Renders param
pub fn string_of_param(param: &Param) -> String {
    let mut output = String::new();
    write_param(&mut output, param).expect("writing to a String cannot fail");
    output
}

fn write_param(output: &mut dyn fmt::Write, param: &Param) -> fmt::Result {
    match &param.node {
        ParamKind::Exp(plain_typ) => write_plain_typ(output, plain_typ),
        ParamKind::Def(id, tparams, params, plain_typ) => {
            output.write_str(&string_of_defid(id))?;
            write_tparams(output, tparams)?;
            write_params(output, params)?;
            output.write_str(" : ")?;
            write_plain_typ(output, plain_typ)
        }
    }
}

/// Renders params
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
            write_param(output, param)?;
        }
        output.write_char(')')
    }
}

// - Type parameters

/// Renders tparam
pub fn string_of_tparam(tparam: &TParam) -> String {
    let mut output = String::new();
    write_tparam(&mut output, tparam).expect("writing to a String cannot fail");
    output
}

fn write_tparam(output: &mut dyn fmt::Write, tparam: &TParam) -> fmt::Result {
    output.write_str(&tparam.node)
}

/// Renders tparams
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
            write_tparam(output, tparam)?;
        }
        output.write_char('>')
    }
}

// - Arguments

/// Renders arg
pub fn string_of_arg(arg: &Arg) -> String {
    let mut output = String::new();
    write_arg(&mut output, arg).expect("writing to a String cannot fail");
    output
}

fn write_arg(output: &mut dyn fmt::Write, arg: &Arg) -> fmt::Result {
    match &arg.node {
        ArgKind::Exp(exp) => write_exp(output, exp),
        ArgKind::Def(id) => output.write_str(&string_of_defid(id)),
    }
}

/// Renders args
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

// - Type arguments

/// Renders targ
pub fn string_of_targ(targ: &Targ) -> String {
    let mut output = String::new();
    write_targ(&mut output, targ).expect("writing to a String cannot fail");
    output
}

fn write_targ(output: &mut dyn fmt::Write, targ: &Targ) -> fmt::Result {
    write_plain_typ(output, targ)
}

/// Renders targs
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
            write_targ(output, targ)?;
        }
        output.write_char('>')
    }
}

// - Premises

/// Renders prem
pub fn string_of_prem(prem: &Prem) -> String {
    let mut output = String::new();
    write_prem(&mut output, prem).expect("writing to a String cannot fail");
    output
}

fn write_prem(output: &mut dyn fmt::Write, prem: &Prem) -> fmt::Result {
    match &prem.node {
        PremKind::Var(VarPrem { id, plain_typ }) => {
            write!(output, "{} : ", string_of_varid(id))?;
            write_plain_typ(output, plain_typ)
        }
        PremKind::Rule(RulePrem { id, exp }) => {
            write!(output, "{}: ", string_of_relid(id))?;
            write_exp(output, exp)
        }
        PremKind::RuleNot(RuleNotPrem { id, exp }) => {
            write!(output, "{}:/ ", string_of_relid(id))?;
            write_exp(output, exp)
        }
        PremKind::If(IfPrem { exp }) => {
            output.write_str("if ")?;
            write_exp(output, exp)
        }
        PremKind::Else => output.write_str("otherwise"),
        PremKind::Iter(IterPrem { prem: inner, iter })
            if matches!(inner.node, PremKind::Iter(_)) =>
        {
            write_prem(output, inner)?;
            output.write_str(&string_of_iter(*iter))
        }
        PremKind::Iter(IterPrem { prem: inner, iter }) => {
            output.write_char('(')?;
            write_prem(output, inner)?;
            write!(output, "){}", string_of_iter(*iter))
        }
        PremKind::Debug(DebugPrem { exp }) => {
            output.write_str("debug ")?;
            write_exp(output, exp)
        }
    }
}

/// Renders prems
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

// - Rules

/// Renders rule
pub fn string_of_rule(rule: &Rule) -> String {
    let mut output = String::new();
    write_rule(&mut output, rule).expect("writing to a String cannot fail");
    output
}

fn write_rule(output: &mut dyn fmt::Write, rule: &Rule) -> fmt::Result {
    write!(
        output,
        "rule {}{}:\n  ",
        string_of_relid(&rule.node.0),
        string_of_ruleid(&rule.node.1)
    )?;
    write_exp(output, &rule.node.2)?;
    write_prems(output, &rule.node.3)
}

/// Renders rules
pub fn string_of_rules(rules: &[Rule]) -> String {
    let mut output = String::new();
    write_rules(&mut output, rules).expect("writing to a String cannot fail");
    output
}

fn write_rules(output: &mut dyn fmt::Write, rules: &[Rule]) -> fmt::Result {
    for (index, rule) in rules.iter().enumerate() {
        if index != 0 {
            output.write_char('\n')?;
        }
        write_rule(output, rule)?;
    }
    Ok(())
}

// - Tables

/// Renders tablerow
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

/// Renders tablerows
pub fn string_of_tablerows(table_rows: &[TableRow]) -> String {
    let mut output = String::new();
    write_tablerows(&mut output, table_rows).expect("writing to a String cannot fail");
    output
}

fn write_tablerows(output: &mut dyn fmt::Write, table_rows: &[TableRow]) -> fmt::Result {
    for (index, row) in table_rows.iter().enumerate() {
        if index != 0 {
            output.write_str("\n  | ")?;
        }
        write_tablerow(output, row)?;
    }
    Ok(())
}

// - Definitions

/// Renders def
pub fn string_of_def(definition: &Def) -> String {
    let mut output = String::new();
    write_def(&mut output, definition).expect("writing to a String cannot fail");
    output
}

fn write_def(output: &mut dyn fmt::Write, definition: &Def) -> fmt::Result {
    match &definition.node {
        DefKind::ExternSyntax(ExternSyntaxDef { id, .. }) => {
            write!(output, "extern syntax {}", string_of_typid(id))
        }
        DefKind::Syntax(SyntaxDef { entries }) => {
            output.write_str("syntax ")?;
            for (index, SyntaxDefEntry { id, tparams }) in entries.iter().enumerate() {
                if index != 0 {
                    output.write_str(", ")?;
                }
                output.write_str(&string_of_typid(id))?;
                write_tparams(output, tparams)?;
            }
            Ok(())
        }
        DefKind::Typ(TypDef {
            id,
            tparams,
            def_typ,
            ..
        }) => {
            write!(output, "syntax {}", string_of_typid(id))?;
            write_tparams(output, tparams)?;
            output.write_str(" = ")?;
            write_def_typ(output, def_typ)
        }
        DefKind::Var(VarDef { id, plain_typ, .. }) => {
            write!(output, "var {} : ", string_of_varid(id))?;
            write_plain_typ(output, plain_typ)
        }
        DefKind::ExternRel(ExternRelDef { id, not_typ, .. }) => {
            write!(output, "extern relation {}: ", string_of_relid(id))?;
            write_not_typ(output, not_typ)
        }
        DefKind::Rel(RelDef { id, not_typ, .. }) => {
            write!(output, "relation {}: ", string_of_relid(id))?;
            write_not_typ(output, not_typ)
        }
        DefKind::RuleGroup(RuleGroupDef {
            relid,
            groupid,
            rules,
        }) => {
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
        DefKind::ExternDec(ExternDecDef {
            id,
            tparams,
            params,
            plain_typ,
            ..
        }) => {
            write!(output, "extern dec {}", string_of_defid(id))?;
            write_tparams(output, tparams)?;
            write_params(output, params)?;
            output.write_str(" : ")?;
            write_plain_typ(output, plain_typ)
        }
        DefKind::BuiltinDec(BuiltinDecDef {
            id,
            tparams,
            params,
            plain_typ,
            ..
        }) => {
            write!(output, "builtin dec {}", string_of_defid(id))?;
            write_tparams(output, tparams)?;
            write_params(output, params)?;
            output.write_str(" : ")?;
            write_plain_typ(output, plain_typ)
        }
        DefKind::TableDec(TableDecDef {
            id,
            params,
            plain_typ,
            ..
        }) => {
            write!(output, "tbl dec {}", string_of_defid(id))?;
            write_params(output, params)?;
            output.write_str(" : ")?;
            write_plain_typ(output, plain_typ)
        }
        DefKind::FuncDec(FuncDecDef {
            id,
            tparams,
            params,
            plain_typ,
            ..
        }) => {
            write!(output, "dec {}", string_of_defid(id))?;
            write_tparams(output, tparams)?;
            write_params(output, params)?;
            output.write_str(" : ")?;
            write_plain_typ(output, plain_typ)
        }
        DefKind::TableDef(TableDef { id, rows }) => {
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
        DefKind::FuncDef(FuncDef {
            id,
            tparams,
            args,
            exp,
            prems,
        }) => {
            write!(output, "def {}", string_of_defid(id))?;
            write_tparams(output, tparams)?;
            write_args(output, args)?;
            output.write_str(" = ")?;
            write_exp(output, exp)?;
            write_prems(output, prems)
        }
        DefKind::Sep => output.write_str("\n\n"),
    }
}

// - Spec

/// Renders spec
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
