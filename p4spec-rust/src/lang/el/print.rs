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
        string_of_atom(&field.0),
        string_of_plaintyp(&field.1)
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
    match &exp.node {
        ExpKind::BoolE(value) => value.to_string(),
        ExpKind::NumE(NumOp::DecOp, num::Number::Nat(number)) => number.to_string(),
        ExpKind::NumE(NumOp::HexOp, num::Number::Nat(number)) => {
            format!("0x{}", number.as_bigint().to_str_radix(16).to_uppercase())
        }
        ExpKind::NumE(_, number) => string_of_num(number),
        ExpKind::TextE(text) => format!("\"{}\"", escaped(text)),
        ExpKind::VarE(id) => string_of_varid(id),
        ExpKind::UnE(operator, exp) => {
            format!("{}{}", string_of_unop(*operator), string_of_exp(exp))
        }
        ExpKind::BinE(left, operator, right) => format!(
            "{} {} {}",
            string_of_exp(left),
            string_of_binop(*operator),
            string_of_exp(right)
        ),
        ExpKind::CmpE(left, operator, right) => format!(
            "{} {} {}",
            string_of_exp(left),
            string_of_cmpop(*operator),
            string_of_exp(right)
        ),
        ExpKind::ArithE(exp) => format!("$({})", string_of_exp(exp)),
        ExpKind::EpsE => "eps".into(),
        ExpKind::ListE(exps) => format!("[{}]", string_of_exps(", ", exps)),
        ExpKind::ConsE(left, right) => {
            format!("{} :: {}", string_of_exp(left), string_of_exp(right))
        }
        ExpKind::CatE(left, right) => {
            format!("{} ++ {}", string_of_exp(left), string_of_exp(right))
        }
        ExpKind::IdxE(base, index) => format!("{}[{}]", string_of_exp(base), string_of_exp(index)),
        ExpKind::SliceE(base, low, high) => format!(
            "{}[{} : {}]",
            string_of_exp(base),
            string_of_exp(low),
            string_of_exp(high)
        ),
        ExpKind::LenE(exp) => format!("|{}|", string_of_exp(exp)),
        ExpKind::MemE(left, right) => {
            format!("{} <- {}", string_of_exp(left), string_of_exp(right))
        }
        ExpKind::StrE(fields) => format!(
            "{{{}}}",
            join(fields, ", ", |(atom, exp)| format!(
                "{} {}",
                string_of_atom(atom),
                string_of_exp(exp)
            ))
        ),
        ExpKind::DotE(exp, atom) => format!("{}.{}", string_of_exp(exp), string_of_atom(atom)),
        ExpKind::UpdE(base, path, field) => format!(
            "{}[{} = {}]",
            string_of_exp(base),
            string_of_path(path),
            string_of_exp(field)
        ),
        ExpKind::ParenE(exp) => format!("({})", string_of_exp(exp)),
        ExpKind::TupleE(exps) => format!("({})", string_of_exps(", ", exps)),
        ExpKind::CallE(id, targs, args) => format!(
            "{}{}{}",
            string_of_defid(id),
            string_of_targs(targs),
            string_of_args(args)
        ),
        ExpKind::IterE(exp, iter) => format!("{}{}", string_of_exp(exp), string_of_iter(*iter)),
        ExpKind::SubE(exp, plain_typ) => format!(
            "{} <: {}",
            string_of_exp(exp),
            string_of_plaintyp(plain_typ)
        ),
        ExpKind::AtomE(atom) => string_of_atom(atom),
        ExpKind::SeqE(exps) => string_of_exps(" ", exps),
        ExpKind::InfixE(left, atom, right) => format!(
            "{} {} {}",
            string_of_exp(left),
            string_of_atom(atom),
            string_of_exp(right)
        ),
        ExpKind::BrackE(left, exp, right) => format!(
            "`{}{}{}",
            string_of_atom(left),
            string_of_exp(exp),
            string_of_atom(right)
        ),
        ExpKind::HoleE(Hole::Num(number)) => format!("%{number}"),
        ExpKind::HoleE(Hole::Next) => "%".into(),
        ExpKind::HoleE(Hole::Rest) => "%%".into(),
        ExpKind::HoleE(Hole::None) => "!%".into(),
        ExpKind::FuseE(left, right) => format!("{}#{}", string_of_exp(left), string_of_exp(right)),
        ExpKind::UnparenE(exp) => format!("##{}", string_of_exp(exp)),
        ExpKind::LatexE(text) => format!("latex({})", escaped(text)),
    }
}

pub fn string_of_exps(separator: &str, exps: &[Exp]) -> String {
    join(exps, separator, string_of_exp)
}

// Paths

pub fn string_of_path(path: &Path) -> String {
    match &path.node {
        PathKind::RootP => String::new(),
        PathKind::IdxP(path, exp) => format!("{}[{}]", string_of_path(path), string_of_exp(exp)),
        PathKind::SliceP(path, low, high) => format!(
            "{}[{} : {}]",
            string_of_path(path),
            string_of_exp(low),
            string_of_exp(high)
        ),
        PathKind::DotP(path, atom) if matches!(path.node, PathKind::RootP) => string_of_atom(atom),
        PathKind::DotP(path, atom) => format!("{}.{}", string_of_path(path), string_of_atom(atom)),
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
    match &arg.node {
        ArgKind::ExpA(exp) => string_of_exp(exp),
        ArgKind::DefA(id) => string_of_defid(id),
    }
}

pub fn string_of_args(args: &[Arg]) -> String {
    if args.is_empty() {
        String::new()
    } else {
        format!("({})", join(args, ", ", string_of_arg))
    }
}

// Type arguments

pub fn string_of_targ(targ: &Targ) -> String {
    string_of_plaintyp(targ)
}

pub fn string_of_targs(targs: &[Targ]) -> String {
    if targs.is_empty() {
        String::new()
    } else {
        format!("<{}>", join(targs, ", ", string_of_targ))
    }
}

// Premises

pub fn string_of_prem(prem: &Prem) -> String {
    match &prem.node {
        PremKind::VarPr(id, plain_typ) => format!(
            "{} : {}",
            string_of_varid(id),
            string_of_plaintyp(plain_typ)
        ),
        PremKind::RulePr(id, exp) => format!("{}: {}", string_of_relid(id), string_of_exp(exp)),
        PremKind::RuleNotPr(id, exp) => format!("{}:/ {}", string_of_relid(id), string_of_exp(exp)),
        PremKind::IfPr(exp) => format!("if {}", string_of_exp(exp)),
        PremKind::ElsePr => "otherwise".into(),
        PremKind::IterPr(inner, iter) if matches!(inner.node, PremKind::IterPr(_, _)) => {
            format!("{}{}", string_of_prem(inner), string_of_iter(*iter))
        }
        PremKind::IterPr(inner, iter) => {
            format!("({}){}", string_of_prem(inner), string_of_iter(*iter))
        }
        PremKind::DebugPr(exp) => format!("debug {}", string_of_exp(exp)),
    }
}

pub fn string_of_prems(prems: &[Prem]) -> String {
    prems
        .iter()
        .map(|prem| format!("\n -- {}", string_of_prem(prem)))
        .collect()
}

// Rules

pub fn string_of_rule(rule: &Rule) -> String {
    let (relid, ruleid, exp, prems) = &rule.node;
    format!(
        "rule {}{}:\n  {}{}",
        string_of_relid(relid),
        string_of_ruleid(ruleid),
        string_of_exp(exp),
        string_of_prems(prems)
    )
}

pub fn string_of_rules(rules: &[Rule]) -> String {
    join(rules, "\n", string_of_rule)
}

// Tables

pub fn string_of_tablerow(table_row: &TableRow) -> String {
    format!(
        "{} => {}",
        string_of_exp(&table_row.node.0),
        string_of_exp(&table_row.node.1)
    )
}

pub fn string_of_tablerows(table_rows: &[TableRow]) -> String {
    join(table_rows, "\n  | ", string_of_tablerow)
}

// Definitions

pub fn string_of_def(definition: &Def) -> String {
    match &definition.node {
        DefKind::ExternSynD(id, _) => format!("extern syntax {}", string_of_typid(id)),
        DefKind::SynD(syntaxes) => format!(
            "syntax {}",
            join(syntaxes, ", ", |(id, tparams)| format!(
                "{}{}",
                string_of_typid(id),
                string_of_tparams(tparams)
            ))
        ),
        DefKind::TypD(id, tparams, def_typ, _) => format!(
            "syntax {}{} = {}",
            string_of_typid(id),
            string_of_tparams(tparams),
            string_of_deftyp(def_typ)
        ),
        DefKind::VarD(id, plain_typ, _) => format!(
            "var {} : {}",
            string_of_varid(id),
            string_of_plaintyp(plain_typ)
        ),
        DefKind::ExternRelD(id, not_typ, _) => format!(
            "extern relation {}: {}",
            string_of_relid(id),
            string_of_nottyp(not_typ)
        ),
        DefKind::RelD(id, not_typ, _) => format!(
            "relation {}: {}",
            string_of_relid(id),
            string_of_nottyp(not_typ)
        ),
        DefKind::RuleGroupD(relid, groupid, rules) => format!(
            "rulegroup {}{}:\n  {}",
            string_of_relid(relid),
            string_of_ruleid(groupid),
            join(rules, "\n  ", string_of_rule)
        ),
        DefKind::ExternDecD(id, tparams, params, plain_typ, _) => format!(
            "extern dec {}{}{} : {}",
            string_of_defid(id),
            string_of_tparams(tparams),
            string_of_params(params),
            string_of_plaintyp(plain_typ)
        ),
        DefKind::BuiltinDecD(id, tparams, params, plain_typ, _) => format!(
            "builtin dec {}{}{} : {}",
            string_of_defid(id),
            string_of_tparams(tparams),
            string_of_params(params),
            string_of_plaintyp(plain_typ)
        ),
        DefKind::TableDecD(id, params, plain_typ, _) => format!(
            "tbl dec {}{} : {}",
            string_of_defid(id),
            string_of_params(params),
            string_of_plaintyp(plain_typ)
        ),
        DefKind::FuncDecD(id, tparams, params, plain_typ, _) => format!(
            "dec {}{}{} : {}",
            string_of_defid(id),
            string_of_tparams(tparams),
            string_of_params(params),
            string_of_plaintyp(plain_typ)
        ),
        DefKind::TableDefD(id, rows) => format!(
            "tbl def {} =\n  {}",
            string_of_defid(id),
            join(rows, "\n  | ", string_of_tablerow)
        ),
        DefKind::FuncDefD(id, tparams, args, exp, prems) => format!(
            "def {}{}{} = {}{}",
            string_of_defid(id),
            string_of_tparams(tparams),
            string_of_args(args),
            string_of_exp(exp),
            string_of_prems(prems)
        ),
        DefKind::SepD => "\n\n".into(),
    }
}

// Spec

pub fn string_of_spec(spec: &Spec) -> String {
    spec.iter()
        .map(|definition| format!("{}\n", string_of_def(definition)))
        .collect()
}
