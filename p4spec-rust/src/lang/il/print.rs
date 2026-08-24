use std::fmt;

use crate::lang::{el, xl::num};

use super::ast::*;

fn join<T>(items: &[T], separator: &str, render: impl Fn(&T) -> String) -> String {
    items.iter().map(render).collect::<Vec<_>>().join(separator)
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

// Numbers

pub fn string_of_num(number: &Num) -> String {
    el::print::string_of_num(number)
}
// Texts

pub fn string_of_text(text: &str) -> String {
    text.into()
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
    id.node.clone()
}
pub fn string_of_rulegroupid(id: &Id) -> String {
    id.node.clone()
}
pub fn string_of_defid(id: &Id) -> String {
    format!("${}", id.node)
}
// Atoms

pub fn string_of_atom(atom: &Atom) -> String {
    atom.node.source_string()
}
pub fn string_of_atoms(atoms: &[Atom]) -> String {
    join(atoms, "", string_of_atom)
}
// Mixfix operators

pub fn string_of_mixop(mixop: &Mixop) -> String {
    mixop.to_string()
}
// Iterators

pub fn string_of_iter(iter: Iter) -> &'static str {
    match iter {
        Iter::Opt => "?",
        Iter::List => "*",
    }
}
// Variables

pub fn string_of_var(variable: &Var) -> String {
    format!(
        "{}{}",
        string_of_varid(&variable.id),
        join(&variable.iters, "", |iter| string_of_iter(*iter).into())
    )
}

// Types

pub fn string_of_typ(typ: &Typ) -> String {
    match &typ.node {
        TypKind::BoolT => "bool".into(),
        TypKind::NumT(num::Typ::NatT) => "nat".into(),
        TypKind::NumT(num::Typ::IntT) => "int".into(),
        TypKind::TextT => "text".into(),
        TypKind::VarT(id, targs) => format!("{}{}", string_of_typid(id), string_of_targs(targs)),
        TypKind::TupleT(typs) => format!("({})", string_of_typs(", ", typs)),
        TypKind::IterT(typ, iter) => format!("{}{}", string_of_typ(typ), string_of_iter(*iter)),
        TypKind::FuncT(tparams, typs, typ) => format!(
            "{}({}) : {}",
            string_of_tparams(tparams),
            string_of_typs(", ", typs),
            string_of_typ(typ)
        ),
    }
}
pub fn string_of_typs(separator: &str, typs: &[Typ]) -> String {
    join(typs, separator, string_of_typ)
}
pub fn string_of_nottyp(nottyp: &NotTyp) -> String {
    nottyp.node.render(string_of_atom, string_of_typ)
}
pub fn string_of_deftyp(deftyp: &DefTyp) -> String {
    match &deftyp.node {
        DefTypKind::PlainT(typ) => string_of_typ(typ),
        DefTypKind::StructT(fields) => format!("{{{}}}", string_of_typfields(", ", fields)),
        DefTypKind::VariantT(cases) => format!("\n   | {}", string_of_typcases("\n   | ", cases)),
    }
}
pub fn string_of_typfield(field: &TypField) -> String {
    format!("{} {}", string_of_atom(&field.0), string_of_typ(&field.1))
}
pub fn string_of_typfields(separator: &str, fields: &[TypField]) -> String {
    join(fields, separator, string_of_typfield)
}
pub fn string_of_typorigin(origin: &TypOrigin) -> String {
    format!(
        "(from {}{})",
        string_of_typid(&origin.node.0),
        string_of_targs(&origin.node.1)
    )
}
pub fn string_of_typcase(case: &TypCase) -> String {
    format!(
        "{} {} {}",
        string_of_nottyp(&case.notation),
        string_of_typorigin(&case.origin),
        string_of_hints(&case.hints)
    )
}
pub fn string_of_typcases(separator: &str, cases: &[TypCase]) -> String {
    join(cases, separator, string_of_typcase)
}

// Values

pub fn string_of_value(value: &Value) -> String {
    string_of_value_with(value, false, 0)
}
pub fn string_of_short_value(value: &Value) -> String {
    string_of_value_with(value, true, 0)
}
pub fn string_of_value_with(value: &Value, short: bool, level: usize) -> String {
    match &value.kind {
        ValueKind::BoolV(value) => value.to_string(),
        ValueKind::NumV(value) => string_of_num(value),
        ValueKind::TextV(text) => escaped(text),
        ValueKind::StructV(fields) if fields.is_empty() => "{}".into(),
        ValueKind::StructV(fields) if short => format!("{{ .../{} }}", fields.len()),
        ValueKind::StructV(fields) => format!(
            "{{\n{}\n{}}}",
            join(fields, ";\n", |(atom, value)| format!(
                "{}{} {}",
                indent(level + 1),
                string_of_atom(atom),
                string_of_value_with(value, short, level + 1)
            )),
            indent(level)
        ),
        ValueKind::CaseV(case) if short => string_of_mixop(&case.to_mixop()),
        ValueKind::CaseV(case) => string_of_notval_with(case, level),
        ValueKind::TupleV(values) => format!(
            "({})",
            join(values, ", ", |value| string_of_value_with(
                value,
                short,
                level + 1
            ))
        ),
        ValueKind::OptV(Some(value)) => {
            format!("Some({})", string_of_value_with(value, short, level + 1))
        }
        ValueKind::OptV(None) => "None".into(),
        ValueKind::ListV(values) if values.is_empty() => "[]".into(),
        ValueKind::ListV(values) if short => format!("[ .../{} ]", values.len()),
        ValueKind::ListV(values) => format!(
            "[\n{}\n{}]",
            join(values, ",\n", |value| format!(
                "{}{}",
                indent(level + 1),
                string_of_value_with(value, short, level + 1)
            )),
            indent(level)
        ),
        ValueKind::FuncV(id) => string_of_defid(id),
        ValueKind::ExternV(_) => "extern".into(),
    }
}
pub fn string_of_notval(notval: &ValueCase) -> String {
    string_of_notval_with(notval, 0)
}
pub fn string_of_notval_with(notval: &ValueCase, level: usize) -> String {
    notval.render(string_of_atom, |value| {
        string_of_value_with(value, false, level + 1)
    })
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
// Expressions

pub fn string_of_exp(exp: &Exp) -> String {
    let mut output = String::new();
    write_exp(&mut output, exp).expect("writing to a String cannot fail");
    output
}

fn write_exp(output: &mut dyn fmt::Write, exp: &Exp) -> fmt::Result {
    match &exp.kind {
        ExpKind::BoolE(value) => write!(output, "{value}"),
        ExpKind::NumE(value) => output.write_str(&string_of_num(value)),
        ExpKind::TextE(text) => write!(output, "\"{}\"", escaped(text)),
        ExpKind::VarE(id) => output.write_str(&id.node),
        ExpKind::UnE(op, _, exp) => {
            output.write_str(string_of_unop(*op))?;
            write_exp(output, exp)
        }
        ExpKind::BinE(op, _, left, right) => {
            output.write_char('(')?;
            write_exp(output, left)?;
            write!(output, " {} ", string_of_binop(*op))?;
            write_exp(output, right)?;
            output.write_char(')')
        }
        ExpKind::CmpE(op, _, left, right) => {
            output.write_char('(')?;
            write_exp(output, left)?;
            write!(output, " {} ", string_of_cmpop(*op))?;
            write_exp(output, right)?;
            output.write_char(')')
        }
        ExpKind::UpCastE(typ, exp) | ExpKind::DownCastE(typ, exp) => {
            write_exp(output, exp)?;
            write!(output, " as {}", string_of_typ(typ))
        }
        ExpKind::SubE(exp, typ, _) => {
            write_exp(output, exp)?;
            write!(output, " <: {}", string_of_typ(typ))
        }
        ExpKind::MatchE(exp, pattern) => {
            write_exp(output, exp)?;
            write!(output, " matches {}", string_of_pattern(pattern))
        }
        ExpKind::TupleE(exps) => {
            output.write_char('(')?;
            write_exps(output, ", ", exps)?;
            output.write_char(')')
        }
        ExpKind::CaseE(notexp) => output.write_str(&string_of_notexp(notexp)),
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
        ExpKind::OptE(exp) => {
            output.write_str("?(")?;
            if let Some(exp) = exp {
                write_exp(output, exp)?;
            }
            output.write_char(')')
        }
        ExpKind::ListE(exps) => {
            output.write_char('[')?;
            write_exps(output, ", ", exps)?;
            output.write_char(']')
        }
        ExpKind::ConsE(head, tail) => {
            write_exp(output, head)?;
            output.write_str(" :: ")?;
            write_exp(output, tail)
        }
        ExpKind::CatE(left, right) => {
            write_exp(output, left)?;
            output.write_str(" ++ ")?;
            write_exp(output, right)
        }
        ExpKind::MemE(element, set) => {
            write_exp(output, element)?;
            output.write_str(" <- ")?;
            write_exp(output, set)
        }
        ExpKind::LenE(exp) => {
            output.write_char('|')?;
            write_exp(output, exp)?;
            output.write_char('|')
        }
        ExpKind::DotE(exp, atom) => {
            write_exp(output, exp)?;
            write!(output, ".{}", string_of_atom(atom))
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
        ExpKind::UpdE(base, path, field) => {
            write_exp(output, base)?;
            output.write_char('[')?;
            write_path(output, path)?;
            output.write_str(" = ")?;
            write_exp(output, field)?;
            output.write_char(']')
        }
        ExpKind::CallE(id, targs, args) => {
            output.write_str(&string_of_defid(id))?;
            write_targs(output, targs)?;
            write_args(output, args)
        }
        ExpKind::IterE(exp, iterexp) => {
            write_exp(output, exp)?;
            output.write_str(&string_of_iterexp(iterexp))
        }
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
pub fn string_of_notexp(notexp: &NotExp) -> String {
    notexp.render(string_of_atom, string_of_exp)
}
pub fn string_of_iterexp(iterexp: &IterExp) -> String {
    format!(
        "{}{{{}}}",
        string_of_iter(iterexp.0),
        join(&iterexp.1, ", ", |variable| {
            let mut iterated = variable.clone();
            iterated.iters.push(iterexp.0);
            format!(
                "{} <- {}",
                string_of_var(variable),
                string_of_var(&iterated)
            )
        })
    )
}
pub fn string_of_iterexps(iterexps: &[IterExp]) -> String {
    join(iterexps, "", string_of_iterexp)
}
// Patterns

pub fn string_of_pattern(pattern: &Pattern) -> String {
    match pattern {
        Pattern::CaseP(mixop) => string_of_mixop(mixop),
        Pattern::ListP(ListPattern::Cons) => "_ :: _".into(),
        Pattern::ListP(ListPattern::Fixed(length)) => format!("[ _/{length} ]"),
        Pattern::ListP(ListPattern::Nil) => "[]".into(),
        Pattern::OptP(OptPattern::Some) => "(_)".into(),
        Pattern::OptP(OptPattern::None) => "()".into(),
    }
}
// Paths

pub fn string_of_path(path: &Path) -> String {
    let mut output = String::new();
    write_path(&mut output, path).expect("writing to a String cannot fail");
    output
}
fn write_path(output: &mut dyn fmt::Write, path: &Path) -> fmt::Result {
    match &path.kind {
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
        PathKind::DotP(path, atom) if matches!(path.kind, PathKind::RootP) => {
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
        ParamKind::ExpP(typ) => string_of_typ(typ),
        ParamKind::DefP(id, tparams, params, typ) => format!(
            "{}{}{} : {}",
            string_of_defid(id),
            string_of_tparams(tparams),
            string_of_params(params),
            string_of_typ(typ)
        ),
    }
}
pub fn string_of_params(params: &[Param]) -> String {
    if params.is_empty() {
        String::new()
    } else {
        format!("({})", join(params, ", ", string_of_param))
    }
}
// Type parameters

pub fn string_of_tparam(tparam: &TParam) -> String {
    tparam.node.clone()
}
pub fn string_of_tparams(tparams: &[TParam]) -> String {
    if tparams.is_empty() {
        String::new()
    } else {
        format!("<{}>", join(tparams, ", ", string_of_tparam))
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
// Type arguments

pub fn string_of_targ(targ: &Targ) -> String {
    string_of_typ(targ)
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
    match &prem.node {
        PremKind::RulePr(id, notexp, _) => {
            format!("{}: {}", string_of_relid(id), string_of_notexp(notexp))
        }
        PremKind::IfPr(exp) => format!("if {}", string_of_exp(exp)),
        PremKind::IfHoldPr(id, notexp) => format!(
            "if {}: {} holds",
            string_of_relid(id),
            string_of_notexp(notexp)
        ),
        PremKind::IfNotHoldPr(id, notexp) => format!(
            "if {}: {} does not hold",
            string_of_relid(id),
            string_of_notexp(notexp)
        ),
        PremKind::LetPr(left, right) => {
            format!("let {} = {}", string_of_exp(left), string_of_exp(right))
        }
        PremKind::IterPr(inner, iterprem) if matches!(inner.node, PremKind::IterPr(_, _)) => {
            format!("{}{}", string_of_prem(inner), string_of_iterprem(iterprem))
        }
        PremKind::IterPr(inner, iterprem) => format!(
            "({}){}",
            string_of_prem(inner),
            string_of_iterprem(iterprem)
        ),
        PremKind::DebugPr(exp) => format!("debug {}", string_of_exp(exp)),
    }
}
pub fn string_of_prems(prems: &[Prem]) -> String {
    string_of_prems_with(0, prems)
}
pub fn string_of_prems_with(level: usize, prems: &[Prem]) -> String {
    join(prems, "", |prem| {
        format!("\n{}-- {}", indent(level), string_of_prem(prem))
    })
}
pub fn string_of_iterprem(iterprem: &IterPrem) -> String {
    let render = |variable: &Var, arrow: &str| {
        let mut iterated = variable.clone();
        iterated.iters.push(iterprem.iter);
        format!(
            "{} {} {}",
            string_of_var(variable),
            arrow,
            string_of_var(&iterated)
        )
    };
    format!(
        "{}{{{}}}",
        string_of_iter(iterprem.iter),
        join(
            &iterprem
                .vars_bound
                .iter()
                .map(|var| render(var, "<-"))
                .chain(iterprem.vars_bind.iter().map(|var| render(var, "->")))
                .collect::<Vec<_>>(),
            ", ",
            Clone::clone
        )
    )
}
pub fn string_of_iterprems(iterprems: &[IterPrem]) -> String {
    join(iterprems, "", string_of_iterprem)
}
// Rules

pub fn string_of_rule(rule: &Rule) -> String {
    format!(
        "rule {}: {}{}",
        string_of_ruleid(&rule.node.id),
        string_of_notexp(&rule.node.notation),
        string_of_prems_with(2, &rule.node.premises)
    )
}
pub fn string_of_rules(rules: &[Rule]) -> String {
    join(rules, "", |rule| {
        format!("\n\n{}{}", indent(2), string_of_rule(rule))
    })
}
pub fn string_of_rulegroup(group: &RuleGroup) -> String {
    format!(
        "{}rulegroup {}{}",
        indent(1),
        string_of_rulegroupid(&group.node.0),
        string_of_rules(&group.node.1)
    )
}
pub fn string_of_rulegroups(groups: &[RuleGroup]) -> String {
    join(groups, "\n\n", string_of_rulegroup)
}
pub fn string_of_elsegroup(group: &ElseGroup) -> String {
    format!(
        "{}rulegroup {}{}",
        indent(1),
        string_of_rulegroupid(&group.node.0),
        string_of_rules(std::slice::from_ref(&group.node.1))
    )
}
pub fn string_of_elsegroup_opt(group: &Option<ElseGroup>) -> String {
    group.as_ref().map_or_else(String::new, |group| {
        format!(
            "\n\n{}elsegroup\n\n{}",
            indent(1),
            string_of_elsegroup(group)
        )
    })
}
// Clause

pub fn string_of_clause(index: i64, clause: &Clause) -> String {
    format!(
        "clause {index} : {} = {}{}",
        string_of_args(&clause.node.args),
        string_of_exp(&clause.node.expression),
        string_of_prems_with(1, &clause.node.premises)
    )
}
pub fn string_of_clauses(clauses: &[Clause]) -> String {
    clauses
        .iter()
        .enumerate()
        .map(|(index, clause)| {
            format!(
                "\n\n{}{}",
                indent(1),
                string_of_clause(index as i64, clause)
            )
        })
        .collect()
}
pub fn string_of_elseclause(clause: &ElseClause) -> String {
    string_of_clause(-1, clause)
}
pub fn string_of_elseclause_opt(clause: &Option<ElseClause>) -> String {
    clause.as_ref().map_or_else(String::new, |clause| {
        format!("\n\n{}{}", indent(1), string_of_elseclause(clause))
    })
}
// Table rows

pub fn string_of_tablerow(row: &TableRow) -> String {
    format!(
        "\n{}{} -> {}",
        indent(2),
        string_of_args(&row.node.0),
        string_of_exp(&row.node.1)
    )
}
pub fn string_of_tablerows(rows: &[TableRow]) -> String {
    rows.iter()
        .enumerate()
        .map(|(index, row)| format!("\n{}row {index} :{}", indent(1), string_of_tablerow(row)))
        .collect()
}
// Hints

pub fn string_of_hint(hint: &Hint) -> String {
    format!(
        " hint({} {})",
        hint.hintid.node,
        el::print::string_of_exp(&hint.hintexp)
    )
}
pub fn string_of_hints(hints: &[Hint]) -> String {
    join(hints, "", string_of_hint)
}

fn write_prem(output: &mut dyn fmt::Write, prem: &Prem) -> fmt::Result {
    match &prem.node {
        PremKind::RulePr(id, notation, _) => write!(
            output,
            "{}: {}",
            string_of_relid(id),
            string_of_notexp(notation)
        ),
        PremKind::IfPr(exp) => {
            output.write_str("if ")?;
            write_exp(output, exp)
        }
        PremKind::IfHoldPr(id, notation) => write!(
            output,
            "if {}: {} holds",
            string_of_relid(id),
            string_of_notexp(notation)
        ),
        PremKind::IfNotHoldPr(id, notation) => write!(
            output,
            "if {}: {} does not hold",
            string_of_relid(id),
            string_of_notexp(notation)
        ),
        PremKind::LetPr(left, right) => {
            output.write_str("let ")?;
            write_exp(output, left)?;
            output.write_str(" = ")?;
            write_exp(output, right)
        }
        PremKind::IterPr(inner, iterprem) if matches!(inner.node, PremKind::IterPr(_, _)) => {
            write_prem(output, inner)?;
            output.write_str(&string_of_iterprem(iterprem))
        }
        PremKind::IterPr(inner, iterprem) => {
            output.write_char('(')?;
            write_prem(output, inner)?;
            write!(output, "){}", string_of_iterprem(iterprem))
        }
        PremKind::DebugPr(exp) => {
            output.write_str("debug ")?;
            write_exp(output, exp)
        }
    }
}

fn write_prems_with(output: &mut dyn fmt::Write, level: usize, prems: &[Prem]) -> fmt::Result {
    for prem in prems {
        write!(output, "\n{}-- ", indent(level))?;
        write_prem(output, prem)?;
    }
    Ok(())
}

fn write_rule(output: &mut dyn fmt::Write, rule: &Rule) -> fmt::Result {
    write!(
        output,
        "rule {}: {}",
        string_of_ruleid(&rule.node.id),
        string_of_notexp(&rule.node.notation)
    )?;
    write_prems_with(output, 2, &rule.node.premises)
}

fn write_rulegroup(output: &mut dyn fmt::Write, group: &RuleGroup) -> fmt::Result {
    write!(
        output,
        "  rulegroup {}",
        string_of_rulegroupid(&group.node.0)
    )?;
    for rule in &group.node.1 {
        output.write_str("\n\n    ")?;
        write_rule(output, rule)?;
    }
    Ok(())
}

fn write_elsegroup(output: &mut dyn fmt::Write, group: &ElseGroup) -> fmt::Result {
    write!(
        output,
        "  rulegroup {}\n\n    ",
        string_of_rulegroupid(&group.node.0)
    )?;
    write_rule(output, &group.node.1)
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

fn write_clauses(output: &mut dyn fmt::Write, clauses: &[Clause]) -> fmt::Result {
    for (index, clause) in clauses.iter().enumerate() {
        output.write_str("\n\n  ")?;
        write_clause(output, index as i64, clause)?;
    }
    Ok(())
}

fn write_tablerow(output: &mut dyn fmt::Write, row: &TableRow) -> fmt::Result {
    write!(output, "\n    {} -> ", string_of_args(&row.node.0))?;
    write_exp(output, &row.node.1)
}

fn write_tablerows(output: &mut dyn fmt::Write, rows: &[TableRow]) -> fmt::Result {
    for (index, row) in rows.iter().enumerate() {
        write!(output, "\n  row {index} :")?;
        write_tablerow(output, row)?;
    }
    Ok(())
}
// Definitions

pub fn string_of_def(definition: &Def) -> String {
    let mut output = String::new();
    write_def(&mut output, definition).expect("writing to a String cannot fail");
    output
}

fn write_def(output: &mut dyn fmt::Write, definition: &Def) -> fmt::Result {
    match &definition.node {
        DefKind::ExternTypD(id, _) => write!(output, "extern syntax {}", string_of_typid(id)),
        DefKind::TypD(id, tparams, deftyp, _) => write!(
            output,
            "syntax {}{} = {}",
            string_of_typid(id),
            string_of_tparams(tparams),
            string_of_deftyp(deftyp)
        ),
        DefKind::VarD(id, typ, _) => write!(
            output,
            "var {} : {}",
            string_of_varid(id),
            string_of_typ(typ)
        ),
        DefKind::ExternRelD(id, nottyp, _, _) => write!(
            output,
            "extern relation {}: {}",
            string_of_relid(id),
            string_of_nottyp(nottyp)
        ),
        DefKind::RelD(id, nottyp, _, groups, elsegroup, _) => {
            write!(
                output,
                "relation {}: {}\n\n",
                string_of_relid(id),
                string_of_nottyp(nottyp)
            )?;
            for (index, group) in groups.iter().enumerate() {
                if index != 0 {
                    output.write_str("\n\n")?;
                }
                write_rulegroup(output, group)?;
            }
            if let Some(group) = elsegroup {
                output.write_str("\n\n  elsegroup\n\n")?;
                write_elsegroup(output, group)?;
            }
            Ok(())
        }
        DefKind::ExternDecD(id, tparams, params, typ, _) => write!(
            output,
            "extern def {}{}{} : {}",
            string_of_defid(id),
            string_of_tparams(tparams),
            string_of_params(params),
            string_of_typ(typ)
        ),
        DefKind::BuiltinDecD(id, tparams, params, typ, _) => write!(
            output,
            "builtin def {}{}{} : {}",
            string_of_defid(id),
            string_of_tparams(tparams),
            string_of_params(params),
            string_of_typ(typ)
        ),
        DefKind::TableDecD(id, params, typ, rows, _) => {
            write!(
                output,
                "tbl def {}{} : {} =",
                string_of_defid(id),
                string_of_params(params),
                string_of_typ(typ)
            )?;
            write_tablerows(output, rows)
        }
        DefKind::FuncDecD(id, tparams, params, typ, clauses, elseclause, _) => {
            write!(
                output,
                "def {}{}{} : {} =",
                string_of_defid(id),
                string_of_tparams(tparams),
                string_of_params(params),
                string_of_typ(typ)
            )?;
            write_clauses(output, clauses)?;
            if let Some(clause) = elseclause {
                output.write_str("\n\n  ")?;
                write_clause(output, -1, clause)?;
            }
            Ok(())
        }
    }
}
pub fn string_of_defs(definitions: &[Def]) -> String {
    join(definitions, "\n\n", string_of_def)
}
// Spec

pub fn string_of_spec(spec: &Spec) -> String {
    let mut output = String::new();
    write_spec(&mut output, spec).expect("writing to a String cannot fail");
    output
}
fn write_spec(output: &mut dyn fmt::Write, spec: &Spec) -> fmt::Result {
    for (index, definition) in spec.iter().enumerate() {
        if index != 0 {
            output.write_str("\n\n")?;
        }
        write_def(output, definition)?;
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
        Exp::new(kind, TypKind::BoolT, Region::none())
    }

    #[test]
    fn expression_sink_preserves_escaping_and_recursive_precedence() {
        let expression = exp(ExpKind::BinE(
            BinOp::AddOp,
            OpTyp::NatT,
            Box::new(exp(ExpKind::TextE("a\n\\\"".to_owned()))),
            Box::new(exp(ExpKind::VarE(id("right")))),
        ));
        let mut output = String::new();

        write_exp(&mut output, &expression).unwrap();

        assert_eq!(output, "(\"a\\n\\\\\\\"\" + right)");
    }
}
