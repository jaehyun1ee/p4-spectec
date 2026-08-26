//! Text rendering for intermediate-language data

use std::fmt;

use crate::lang::{
    el::{self, print::string_of_binop, print::string_of_cmpop, print::string_of_unop},
    xl::num::{self, string_of_num},
};

use super::ast::*;

fn join<T>(items: &[T], sep: &str, render: impl Fn(&T) -> String) -> String {
    items.iter().map(render).collect::<Vec<_>>().join(sep)
}
fn indent(level: usize) -> String {
    "  ".repeat(level)
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

// - Texts

/// Renders text
pub fn string_of_text(text: &str) -> String {
    text.into()
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
    id.node.clone()
}
/// Renders rulegroupid
pub fn string_of_rulegroupid(id: &Id) -> String {
    id.node.clone()
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
/// Renders atoms
pub fn string_of_atoms(atoms: &[Atom]) -> String {
    let mut output = String::new();
    write_atoms(&mut output, atoms).expect("writing to a String cannot fail");
    output
}

fn write_atoms(output: &mut dyn fmt::Write, atoms: &[Atom]) -> fmt::Result {
    for atom in atoms {
        output.write_str(&string_of_atom(atom))?;
    }
    Ok(())
}
// - Mixfix operators

/// Renders mixop
pub fn string_of_mixop(mixop: &Mixop) -> String {
    let mut output = String::new();
    write_mixop(&mut output, mixop).expect("writing to a String cannot fail");
    output
}

fn write_mixop(output: &mut dyn fmt::Write, mixop: &Mixop) -> fmt::Result {
    let rendered = mixop
        .to_string((0..mixop.arity()).map(|_| "%".to_owned()), string_of_atom)
        .expect("mixop arguments match its arity");
    output.write_str(&rendered)
}
// - Iterators

/// Renders iter
pub fn string_of_iter(iter: Iter) -> &'static str {
    match iter {
        Iter::Opt => "?",
        Iter::List => "*",
    }
}
// - Variables

/// Renders var
pub fn string_of_var(var: &Var) -> String {
    let mut output = String::new();
    write_var(&mut output, var).expect("writing to a String cannot fail");
    output
}

fn write_var(output: &mut dyn fmt::Write, var: &Var) -> fmt::Result {
    output.write_str(&string_of_varid(&var.id))?;
    for iter in &var.iters {
        output.write_str(string_of_iter(*iter))?;
    }
    Ok(())
}

// - Types

/// Renders typ
pub fn string_of_typ(typ: &Typ) -> String {
    let mut output = String::new();
    write_typ(&mut output, typ).expect("writing to a String cannot fail");
    output
}

fn write_typ(output: &mut dyn fmt::Write, typ: &Typ) -> fmt::Result {
    match &typ.node {
        TypKind::Bool => output.write_str("bool"),
        TypKind::Num(num::Typ::Nat) => output.write_str("nat"),
        TypKind::Num(num::Typ::Int) => output.write_str("int"),
        TypKind::Text => output.write_str("text"),
        TypKind::Var(id, targs) => {
            output.write_str(&string_of_typid(id))?;
            write_targs(output, targs)
        }
        TypKind::Tuple(typs) => {
            output.write_char('(')?;
            write_typs(output, ", ", typs)?;
            output.write_char(')')
        }
        TypKind::Iter(typ, iter) => {
            write_typ(output, typ)?;
            output.write_str(string_of_iter(*iter))
        }
        TypKind::Func(tparams, typs, typ) => {
            write_tparams(output, tparams)?;
            output.write_char('(')?;
            write_typs(output, ", ", typs)?;
            output.write_str(") : ")?;
            write_typ(output, typ)
        }
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
/// Renders not typ
pub fn string_of_not_typ(not_typ: &NotTyp) -> String {
    let mut output = String::new();
    write_not_typ(&mut output, not_typ).expect("writing to a String cannot fail");
    output
}
fn write_not_typ(output: &mut dyn fmt::Write, not_typ: &NotTyp) -> fmt::Result {
    output.write_str(&not_typ.node.render(string_of_atom, string_of_typ))
}
/// Renders def typ
pub fn string_of_def_typ(def_typ: &DefTyp) -> String {
    let mut output = String::new();
    write_def_typ(&mut output, def_typ).expect("writing to a String cannot fail");
    output
}
fn write_def_typ(output: &mut dyn fmt::Write, def_typ: &DefTyp) -> fmt::Result {
    match &def_typ.node {
        DefTypKind::Plain(typ) => write_typ(output, typ),
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
    write_typ(output, &typ_field.1)
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
/// Renders typ origin
pub fn string_of_typ_origin(typ_origin: &TypOrigin) -> String {
    let mut output = String::new();
    write_typ_origin(&mut output, typ_origin).expect("writing to a String cannot fail");
    output
}

fn write_typ_origin(output: &mut dyn fmt::Write, typ_origin: &TypOrigin) -> fmt::Result {
    write!(output, "(from {}", string_of_typid(&typ_origin.node.0))?;
    write_targs(output, &typ_origin.node.1)?;
    output.write_char(')')
}

/// Renders typ case
pub fn string_of_typ_case(typ_case: &TypCase) -> String {
    let mut output = String::new();
    write_typ_case(&mut output, typ_case).expect("writing to a String cannot fail");
    output
}

fn write_typ_case(output: &mut dyn fmt::Write, typ_case: &TypCase) -> fmt::Result {
    let (not_typ, typ_origin, hints) = typ_case;
    write_not_typ(output, not_typ)?;
    output.write_char(' ')?;
    write_typ_origin(output, typ_origin)?;
    output.write_char(' ')?;
    output.write_str(&string_of_hints(hints))
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

// - Values

/// Renders value
pub fn string_of_value(value: &Value) -> String {
    let mut output = String::new();
    write_value(&mut output, value).expect("writing to a String cannot fail");
    output
}

fn write_value(output: &mut dyn fmt::Write, value: &Value) -> fmt::Result {
    write_value_with(output, value, false, 0)
}

/// Renders short value
pub fn string_of_short_value(value: &Value) -> String {
    let mut output = String::new();
    write_short_value(&mut output, value).expect("writing to a String cannot fail");
    output
}

fn write_short_value(output: &mut dyn fmt::Write, value: &Value) -> fmt::Result {
    write_value_with(output, value, true, 0)
}

/// Renders value with
pub fn string_of_value_with(value: &Value, short: bool, level: usize) -> String {
    let mut output = String::new();
    write_value_with(&mut output, value, short, level).expect("writing to a String cannot fail");
    output
}

fn write_value_with(
    output: &mut dyn fmt::Write,
    value: &Value,
    short: bool,
    level: usize,
) -> fmt::Result {
    output.write_str(&render_value_with(value, short, level))
}

fn render_value_with(value: &Value, short: bool, level: usize) -> String {
    match &value.node.kind {
        ValueKind::Bool(value) => value.to_string(),
        ValueKind::Num(value) => string_of_num(value),
        ValueKind::Text(text) => escaped(text),
        ValueKind::Struct(fields) if fields.is_empty() => "{}".into(),
        ValueKind::Struct(fields) if short => format!("{{ .../{} }}", fields.len()),
        ValueKind::Struct(fields) => format!(
            "{{\n{}\n{}}}",
            join(fields, ";\n", |(atom, value)| format!(
                "{}{} {}",
                indent(level + 1),
                string_of_atom(atom),
                render_value_with(value, short, level + 1)
            )),
            indent(level)
        ),
        ValueKind::Case(case) if short => string_of_mixop(&case.to_mixop()),
        ValueKind::Case(case) => render_notval_with(case, level),
        ValueKind::Tuple(values) => format!(
            "({})",
            join(values, ", ", |value| render_value_with(
                value,
                short,
                level + 1
            ))
        ),
        ValueKind::Opt(Some(value)) => {
            format!("Some({})", render_value_with(value, short, level + 1))
        }
        ValueKind::Opt(None) => "None".into(),
        ValueKind::List(values) if values.is_empty() => "[]".into(),
        ValueKind::List(values) if short => format!("[ .../{} ]", values.len()),
        ValueKind::List(values) => format!(
            "[\n{}\n{}]",
            join(values, ",\n", |value| format!(
                "{}{}",
                indent(level + 1),
                render_value_with(value, short, level + 1)
            )),
            indent(level)
        ),
        ValueKind::Func(id) => string_of_defid(id),
        ValueKind::Extern(_) => "extern".into(),
    }
}
/// Renders notval
pub fn string_of_notval(not_val: &ValueCase) -> String {
    let mut output = String::new();
    write_notval(&mut output, not_val).expect("writing to a String cannot fail");
    output
}

fn write_notval(output: &mut dyn fmt::Write, not_val: &ValueCase) -> fmt::Result {
    write_notval_with(output, not_val, 0)
}

/// Renders notval with
pub fn string_of_notval_with(not_val: &ValueCase, level: usize) -> String {
    let mut output = String::new();
    write_notval_with(&mut output, not_val, level).expect("writing to a String cannot fail");
    output
}

fn write_notval_with(
    output: &mut dyn fmt::Write,
    not_val: &ValueCase,
    level: usize,
) -> fmt::Result {
    output.write_str(&render_notval_with(not_val, level))
}

fn render_notval_with(not_val: &ValueCase, level: usize) -> String {
    not_val.render(string_of_atom, |value| {
        render_value_with(value, false, level + 1)
    })
}

// - Expressions

/// Renders exp
pub fn string_of_exp(exp: &Exp) -> String {
    let mut output = String::new();
    write_exp(&mut output, exp).expect("writing to a String cannot fail");
    output
}

fn write_exp(output: &mut dyn fmt::Write, exp: &Exp) -> fmt::Result {
    match &exp.node.kind {
        ExpKind::Bool(value) => write!(output, "{value}"),
        ExpKind::Num(value) => output.write_str(&string_of_num(value)),
        ExpKind::Text(text) => write!(output, "\"{}\"", escaped(text)),
        ExpKind::Var(id) => output.write_str(&id.node),
        ExpKind::Un(op, _, exp) => {
            output.write_str(string_of_unop(*op))?;
            write_exp(output, exp)
        }
        ExpKind::Bin(op, _, exp_l, exp_r) => {
            output.write_char('(')?;
            write_exp(output, exp_l)?;
            write!(output, " {} ", string_of_binop(*op))?;
            write_exp(output, exp_r)?;
            output.write_char(')')
        }
        ExpKind::Cmp(op, _, exp_l, exp_r) => {
            output.write_char('(')?;
            write_exp(output, exp_l)?;
            write!(output, " {} ", string_of_cmpop(*op))?;
            write_exp(output, exp_r)?;
            output.write_char(')')
        }
        ExpKind::UpCast(typ, exp) | ExpKind::DownCast(typ, exp) => {
            write_exp(output, exp)?;
            write!(output, " as {}", string_of_typ(typ))
        }
        ExpKind::Sub(exp, typ, _) => {
            write_exp(output, exp)?;
            write!(output, " <: {}", string_of_typ(typ))
        }
        ExpKind::Match(exp, pattern) => {
            write_exp(output, exp)?;
            write!(output, " matches {}", string_of_pattern(pattern))
        }
        ExpKind::Tuple(exps) => {
            output.write_char('(')?;
            write_exps(output, ", ", exps)?;
            output.write_char(')')
        }
        ExpKind::Case(not_exp) => write_notexp(output, not_exp),
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
        ExpKind::Opt(exp) => {
            output.write_str("?(")?;
            if let Some(exp) = exp {
                write_exp(output, exp)?;
            }
            output.write_char(')')
        }
        ExpKind::List(exps) => {
            output.write_char('[')?;
            write_exps(output, ", ", exps)?;
            output.write_char(']')
        }
        ExpKind::Cons(head, tail) => {
            write_exp(output, head)?;
            output.write_str(" :: ")?;
            write_exp(output, tail)
        }
        ExpKind::Cat(exp_l, exp_r) => {
            write_exp(output, exp_l)?;
            output.write_str(" ++ ")?;
            write_exp(output, exp_r)
        }
        ExpKind::Mem(exp_e, exp_s) => {
            write_exp(output, exp_e)?;
            output.write_str(" <- ")?;
            write_exp(output, exp_s)
        }
        ExpKind::Len(exp) => {
            output.write_char('|')?;
            write_exp(output, exp)?;
            output.write_char('|')
        }
        ExpKind::Dot(exp, atom) => {
            write_exp(output, exp)?;
            write!(output, ".{}", string_of_atom(atom))
        }
        ExpKind::Idx(exp_b, exp_i) => {
            write_exp(output, exp_b)?;
            output.write_char('[')?;
            write_exp(output, exp_i)?;
            output.write_char(']')
        }
        ExpKind::Slice(exp_b, exp_i, exp_n) => {
            write_exp(output, exp_b)?;
            output.write_char('[')?;
            write_exp(output, exp_i)?;
            output.write_str(" : ")?;
            write_exp(output, exp_n)?;
            output.write_char(']')
        }
        ExpKind::Upd(exp_b, path, exp_f) => {
            write_exp(output, exp_b)?;
            output.write_char('[')?;
            write_path(output, path)?;
            output.write_str(" = ")?;
            write_exp(output, exp_f)?;
            output.write_char(']')
        }
        ExpKind::Call(id, targs, args) => {
            output.write_str(&string_of_defid(id))?;
            write_targs(output, targs)?;
            write_args(output, args)
        }
        ExpKind::Iter(exp, iter_exp) => {
            write_exp(output, exp)?;
            write_iterexp(output, iter_exp)
        }
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
/// Renders notexp
pub fn string_of_notexp(not_exp: &NotExp) -> String {
    let mut output = String::new();
    write_notexp(&mut output, not_exp).expect("writing to a String cannot fail");
    output
}

fn write_notexp(output: &mut dyn fmt::Write, not_exp: &NotExp) -> fmt::Result {
    output.write_str(&not_exp.render(string_of_atom, string_of_exp))
}

/// Renders iterexp
pub fn string_of_iterexp(iter_exp: &IterExp) -> String {
    let mut output = String::new();
    write_iterexp(&mut output, iter_exp).expect("writing to a String cannot fail");
    output
}

fn write_iterexp(output: &mut dyn fmt::Write, iter_exp: &IterExp) -> fmt::Result {
    write!(output, "{}{{", string_of_iter(iter_exp.0))?;
    for (index, var) in iter_exp.1.iter().enumerate() {
        if index != 0 {
            output.write_str(", ")?;
        }
        let mut var_iter = var.clone();
        var_iter.iters.push(iter_exp.0);
        write!(
            output,
            "{} <- {}",
            string_of_var(var),
            string_of_var(&var_iter)
        )?;
    }
    output.write_char('}')
}

/// Renders iterexps
pub fn string_of_iterexps(iter_exps: &[IterExp]) -> String {
    let mut output = String::new();
    write_iterexps(&mut output, iter_exps).expect("writing to a String cannot fail");
    output
}

fn write_iterexps(output: &mut dyn fmt::Write, iter_exps: &[IterExp]) -> fmt::Result {
    for iter_exp in iter_exps {
        write_iterexp(output, iter_exp)?;
    }
    Ok(())
}
// - Patterns

/// Renders pattern
pub fn string_of_pattern(pattern: &Pattern) -> String {
    let mut output = String::new();
    write_pattern(&mut output, pattern).expect("writing to a String cannot fail");
    output
}

fn write_pattern(output: &mut dyn fmt::Write, pattern: &Pattern) -> fmt::Result {
    let rendered = match pattern {
        Pattern::Case(mixop) => string_of_mixop(mixop),
        Pattern::List(ListPattern::Cons) => "_ :: _".into(),
        Pattern::List(ListPattern::Fixed(length)) => format!("[ _/{length} ]"),
        Pattern::List(ListPattern::Nil) => "[]".into(),
        Pattern::Opt(OptPattern::Some) => "(_)".into(),
        Pattern::Opt(OptPattern::None) => "()".into(),
    };
    output.write_str(&rendered)
}
// - Paths

/// Renders path
pub fn string_of_path(path: &Path) -> String {
    let mut output = String::new();
    write_path(&mut output, path).expect("writing to a String cannot fail");
    output
}
fn write_path(output: &mut dyn fmt::Write, path: &Path) -> fmt::Result {
    match &path.node.kind {
        PathKind::Root => Ok(()),
        PathKind::Idx(path, exp_i) => {
            write_path(output, path)?;
            output.write_char('[')?;
            write_exp(output, exp_i)?;
            output.write_char(']')
        }
        PathKind::Slice(path, exp_i, exp_n) => {
            write_path(output, path)?;
            output.write_char('[')?;
            write_exp(output, exp_i)?;
            output.write_str(" : ")?;
            write_exp(output, exp_n)?;
            output.write_char(']')
        }
        PathKind::Dot(path, atom) if matches!(path.node.kind, PathKind::Root) => {
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
        ParamKind::Exp(typ) => write_typ(output, typ),
        ParamKind::Def(id, tparams, params, typ) => {
            output.write_str(&string_of_defid(id))?;
            write_tparams(output, tparams)?;
            write_params(output, params)?;
            output.write_str(" : ")?;
            write_typ(output, typ)
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
        return Ok(());
    }
    output.write_char('(')?;
    for (index, param) in params.iter().enumerate() {
        if index != 0 {
            output.write_str(", ")?;
        }
        write_param(output, param)?;
    }
    output.write_char(')')
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
        return Ok(());
    }
    output.write_char('<')?;
    for (index, tparam) in tparams.iter().enumerate() {
        if index != 0 {
            output.write_str(", ")?;
        }
        write_tparam(output, tparam)?;
    }
    output.write_char('>')
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
        return Ok(());
    }
    output.write_char('(')?;
    for (index, arg) in args.iter().enumerate() {
        if index != 0 {
            output.write_str(", ")?;
        }
        write_arg(output, arg)?;
    }
    output.write_char(')')
}
// - Type arguments

/// Renders targ
pub fn string_of_targ(targ: &Targ) -> String {
    let mut output = String::new();
    write_targ(&mut output, targ).expect("writing to a String cannot fail");
    output
}
fn write_targ(output: &mut dyn fmt::Write, targ: &Targ) -> fmt::Result {
    write_typ(output, targ)
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
        PremKind::Rule(RulePrem { id, not_exp, .. }) => write!(
            output,
            "{}: {}",
            string_of_relid(id),
            string_of_notexp(not_exp)
        ),
        PremKind::If(IfPrem { exp }) => {
            output.write_str("if ")?;
            write_exp(output, exp)
        }
        PremKind::IfHold(IfHoldPrem { id, not_exp }) => write!(
            output,
            "if {}: {} holds",
            string_of_relid(id),
            string_of_notexp(not_exp)
        ),
        PremKind::IfNotHold(IfNotHoldPrem { id, not_exp }) => write!(
            output,
            "if {}: {} does not hold",
            string_of_relid(id),
            string_of_notexp(not_exp)
        ),
        PremKind::Let(LetPrem { exp_l, exp_r }) => {
            output.write_str("let ")?;
            write_exp(output, exp_l)?;
            output.write_str(" = ")?;
            write_exp(output, exp_r)
        }
        PremKind::Iter(IteratedPrem {
            prem: inner,
            iter_prem,
        }) if matches!(inner.node, PremKind::Iter(_)) => {
            write_prem(output, inner)?;
            write_iterprem(output, iter_prem)
        }
        PremKind::Iter(IteratedPrem {
            prem: inner,
            iter_prem,
        }) => {
            output.write_char('(')?;
            write_prem(output, inner)?;
            output.write_char(')')?;
            write_iterprem(output, iter_prem)
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
    write_prems_with(output, 0, prems)
}

/// Renders prems with
pub fn string_of_prems_with(level: usize, prems: &[Prem]) -> String {
    let mut output = String::new();
    write_prems_with(&mut output, level, prems).expect("writing to a String cannot fail");
    output
}

fn write_prems_with(output: &mut dyn fmt::Write, level: usize, prems: &[Prem]) -> fmt::Result {
    for prem in prems {
        write!(output, "\n{}-- ", indent(level))?;
        write_prem(output, prem)?;
    }
    Ok(())
}

/// Renders iterprem
pub fn string_of_iterprem(iter_prem: &IterPrem) -> String {
    let mut output = String::new();
    write_iterprem(&mut output, iter_prem).expect("writing to a String cannot fail");
    output
}

fn write_iterprem(output: &mut dyn fmt::Write, iter_prem: &IterPrem) -> fmt::Result {
    write!(output, "{}{{", string_of_iter(iter_prem.iter))?;
    let vars = iter_prem
        .vars_bound
        .iter()
        .map(|var| (var, "<-"))
        .chain(iter_prem.vars_bind.iter().map(|var| (var, "->")));
    for (index, (var, arrow)) in vars.enumerate() {
        if index != 0 {
            output.write_str(", ")?;
        }
        let mut var_iter = var.clone();
        var_iter.iters.push(iter_prem.iter);
        write!(
            output,
            "{} {} {}",
            string_of_var(var),
            arrow,
            string_of_var(&var_iter)
        )?;
    }
    output.write_char('}')
}

/// Renders iterprems
pub fn string_of_iterprems(iter_prems: &[IterPrem]) -> String {
    let mut output = String::new();
    write_iterprems(&mut output, iter_prems).expect("writing to a String cannot fail");
    output
}

fn write_iterprems(output: &mut dyn fmt::Write, iter_prems: &[IterPrem]) -> fmt::Result {
    for iter_prem in iter_prems {
        write_iterprem(output, iter_prem)?;
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
        "rule {}: {}",
        string_of_ruleid(&rule.node.id),
        string_of_notexp(&rule.node.not_exp)
    )?;
    write_prems_with(output, 2, &rule.node.prems)
}

/// Renders rules
pub fn string_of_rules(rules: &[Rule]) -> String {
    let mut output = String::new();
    write_rules(&mut output, rules).expect("writing to a String cannot fail");
    output
}

fn write_rules(output: &mut dyn fmt::Write, rules: &[Rule]) -> fmt::Result {
    for rule in rules {
        output.write_str("\n\n  ")?;
        write_rule(output, rule)?;
    }
    Ok(())
}

/// Renders rulegroup
pub fn string_of_rulegroup(rule_group: &RuleGroup) -> String {
    let mut output = String::new();
    write_rulegroup(&mut output, rule_group).expect("writing to a String cannot fail");
    output
}

fn write_rulegroup(output: &mut dyn fmt::Write, rule_group: &RuleGroup) -> fmt::Result {
    write!(
        output,
        "  rulegroup {}",
        string_of_rulegroupid(&rule_group.node.0)
    )?;
    for rule in &rule_group.node.1 {
        output.write_str("\n\n    ")?;
        write_rule(output, rule)?;
    }
    Ok(())
}

/// Renders rulegroups
pub fn string_of_rulegroups(rule_groups: &[RuleGroup]) -> String {
    let mut output = String::new();
    write_rulegroups(&mut output, rule_groups).expect("writing to a String cannot fail");
    output
}

fn write_rulegroups(output: &mut dyn fmt::Write, rule_groups: &[RuleGroup]) -> fmt::Result {
    for (index, rule_group) in rule_groups.iter().enumerate() {
        if index != 0 {
            output.write_str("\n\n")?;
        }
        write_rulegroup(output, rule_group)?;
    }
    Ok(())
}

/// Renders elsegroup
pub fn string_of_elsegroup(else_group: &ElseGroup) -> String {
    let mut output = String::new();
    write_elsegroup(&mut output, else_group).expect("writing to a String cannot fail");
    output
}

fn write_elsegroup(output: &mut dyn fmt::Write, else_group: &ElseGroup) -> fmt::Result {
    write!(
        output,
        "  rulegroup {}\n\n    ",
        string_of_rulegroupid(&else_group.node.0)
    )?;
    write_rule(output, &else_group.node.1)
}

/// Renders elsegroup opt
pub fn string_of_elsegroup_opt(else_group: &Option<ElseGroup>) -> String {
    let mut output = String::new();
    write_elsegroup_opt(&mut output, else_group).expect("writing to a String cannot fail");
    output
}

fn write_elsegroup_opt(output: &mut dyn fmt::Write, else_group: &Option<ElseGroup>) -> fmt::Result {
    if let Some(else_group) = else_group {
        output.write_str("\n\n  elsegroup\n\n")?;
        write_elsegroup(output, else_group)?;
    }
    Ok(())
}

// - Clauses

/// Renders clause
pub fn string_of_clause(index: i64, clause: &Clause) -> String {
    let mut output = String::new();
    write_clause(&mut output, index, clause).expect("writing to a String cannot fail");
    output
}

fn write_clause(output: &mut dyn fmt::Write, index: i64, clause: &Clause) -> fmt::Result {
    write!(
        output,
        "clause {index} : {} = ",
        string_of_args(&clause.node.args)
    )?;
    write_exp(output, &clause.node.expression)?;
    write_prems_with(output, 1, &clause.node.premises)
}

/// Renders clauses
pub fn string_of_clauses(clauses: &[Clause]) -> String {
    let mut output = String::new();
    write_clauses(&mut output, clauses).expect("writing to a String cannot fail");
    output
}

fn write_clauses(output: &mut dyn fmt::Write, clauses: &[Clause]) -> fmt::Result {
    for (index, clause) in clauses.iter().enumerate() {
        output.write_str("\n\n  ")?;
        write_clause(output, index as i64, clause)?;
    }
    Ok(())
}

/// Renders elseclause
pub fn string_of_elseclause(else_clause: &ElseClause) -> String {
    let mut output = String::new();
    write_elseclause(&mut output, else_clause).expect("writing to a String cannot fail");
    output
}

fn write_elseclause(output: &mut dyn fmt::Write, else_clause: &ElseClause) -> fmt::Result {
    write_clause(output, -1, else_clause)
}

/// Renders elseclause opt
pub fn string_of_elseclause_opt(else_clause: &Option<ElseClause>) -> String {
    let mut output = String::new();
    write_elseclause_opt(&mut output, else_clause).expect("writing to a String cannot fail");
    output
}

fn write_elseclause_opt(
    output: &mut dyn fmt::Write,
    else_clause: &Option<ElseClause>,
) -> fmt::Result {
    if let Some(else_clause) = else_clause {
        output.write_str("\n\n  ")?;
        write_elseclause(output, else_clause)?;
    }
    Ok(())
}

// - Table rows

/// Renders tablerow
pub fn string_of_tablerow(table_row: &TableRow) -> String {
    let mut output = String::new();
    write_tablerow(&mut output, table_row).expect("writing to a String cannot fail");
    output
}

fn write_tablerow(output: &mut dyn fmt::Write, table_row: &TableRow) -> fmt::Result {
    write!(output, "\n    {} -> ", string_of_args(&table_row.node.0))?;
    write_exp(output, &table_row.node.1)
}

/// Renders tablerows
pub fn string_of_tablerows(table_rows: &[TableRow]) -> String {
    let mut output = String::new();
    write_tablerows(&mut output, table_rows).expect("writing to a String cannot fail");
    output
}

fn write_tablerows(output: &mut dyn fmt::Write, table_rows: &[TableRow]) -> fmt::Result {
    for (index, table_row) in table_rows.iter().enumerate() {
        write!(output, "\n  row {index} :")?;
        write_tablerow(output, table_row)?;
    }
    Ok(())
}

// - Hints

/// Renders hint
pub fn string_of_hint(hint: &Hint) -> String {
    let mut output = String::new();
    write_hint(&mut output, hint).expect("writing to a String cannot fail");
    output
}

fn write_hint(output: &mut dyn fmt::Write, hint: &Hint) -> fmt::Result {
    write!(
        output,
        " hint({} {})",
        hint.0.node,
        el::print::string_of_exp(&hint.1)
    )
}

/// Renders hints
pub fn string_of_hints(hints: &[Hint]) -> String {
    let mut output = String::new();
    write_hints(&mut output, hints).expect("writing to a String cannot fail");
    output
}

fn write_hints(output: &mut dyn fmt::Write, hints: &[Hint]) -> fmt::Result {
    for hint in hints {
        write_hint(output, hint)?;
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
        DefKind::ExternTyp(ExternTyp { id, .. }) => {
            write!(output, "extern syntax {}", string_of_typid(id))
        }
        DefKind::Typ(TypDef {
            id,
            tparams,
            def_typ,
            ..
        }) => write!(
            output,
            "syntax {}{} = {}",
            string_of_typid(id),
            string_of_tparams(tparams),
            string_of_def_typ(def_typ)
        ),
        DefKind::Var(VarDef { id, typ, .. }) => write!(
            output,
            "var {} : {}",
            string_of_varid(id),
            string_of_typ(typ)
        ),
        DefKind::ExternRel(ExternRel { id, not_typ, .. }) => write!(
            output,
            "extern relation {}: {}",
            string_of_relid(id),
            string_of_not_typ(not_typ)
        ),
        DefKind::Rel(Rel {
            id,
            not_typ,
            rule_groups,
            else_group,
            ..
        }) => {
            write!(
                output,
                "relation {}: {}\n\n",
                string_of_relid(id),
                string_of_not_typ(not_typ)
            )?;
            write_rulegroups(output, rule_groups)?;
            write_elsegroup_opt(output, else_group)
        }
        DefKind::ExternDec(ExternDec {
            id,
            tparams,
            params,
            typ,
            ..
        }) => write!(
            output,
            "extern def {}{}{} : {}",
            string_of_defid(id),
            string_of_tparams(tparams),
            string_of_params(params),
            string_of_typ(typ)
        ),
        DefKind::BuiltinDec(BuiltinDec {
            id,
            tparams,
            params,
            typ,
            ..
        }) => write!(
            output,
            "builtin def {}{}{} : {}",
            string_of_defid(id),
            string_of_tparams(tparams),
            string_of_params(params),
            string_of_typ(typ)
        ),
        DefKind::TableDec(TableDec {
            id,
            params,
            typ,
            rows,
            ..
        }) => {
            write!(
                output,
                "tbl def {}{} : {} =",
                string_of_defid(id),
                string_of_params(params),
                string_of_typ(typ)
            )?;
            write_tablerows(output, rows)
        }
        DefKind::FuncDec(FuncDec {
            id,
            tparams,
            params,
            typ,
            clauses,
            else_clause,
            ..
        }) => {
            write!(
                output,
                "def {}{}{} : {} =",
                string_of_defid(id),
                string_of_tparams(tparams),
                string_of_params(params),
                string_of_typ(typ)
            )?;
            write_clauses(output, clauses)?;
            write_elseclause_opt(output, else_clause)
        }
    }
}

/// Renders defs
pub fn string_of_defs(definitions: &[Def]) -> String {
    let mut output = String::new();
    write_defs(&mut output, definitions).expect("writing to a String cannot fail");
    output
}

fn write_defs(output: &mut dyn fmt::Write, definitions: &[Def]) -> fmt::Result {
    for (index, definition) in definitions.iter().enumerate() {
        if index != 0 {
            output.write_str("\n\n")?;
        }
        write_def(output, definition)?;
    }
    Ok(())
}

// - Specifications

/// Renders a specification without source or hint metadata
pub fn string_of_spec(spec: &Spec) -> String {
    let mut output = String::new();
    write_spec(&mut output, spec).expect("writing to a String cannot fail");
    output
}

fn write_spec(output: &mut dyn fmt::Write, spec: &Spec) -> fmt::Result {
    write_defs(output, spec)
}
