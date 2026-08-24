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

pub fn string_of_num(number: &Num) -> String {
    el::print::string_of_num(number)
}
pub fn string_of_text(text: &str) -> String {
    text.into()
}
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
pub fn string_of_atom(atom: &Atom) -> String {
    atom.node.source_string()
}
pub fn string_of_atoms(atoms: &[Atom]) -> String {
    join(atoms, "", string_of_atom)
}
pub fn string_of_mixop(mixop: &Mixop) -> String {
    mixop.to_string()
}
pub fn string_of_iter(iter: Iter) -> &'static str {
    match iter {
        Iter::Opt => "?",
        Iter::List => "*",
    }
}
pub fn string_of_var(variable: &Var) -> String {
    format!(
        "{}{}",
        string_of_varid(&variable.0),
        join(&variable.2, "", |iter| string_of_iter(*iter).into())
    )
}

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
        string_of_nottyp(&case.0),
        string_of_typorigin(&case.1),
        string_of_hints(&case.2)
    )
}
pub fn string_of_typcases(separator: &str, cases: &[TypCase]) -> String {
    join(cases, separator, string_of_typcase)
}

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
pub fn string_of_exp(exp: &Exp) -> String {
    match &exp.kind {
        ExpKind::BoolE(value) => value.to_string(),
        ExpKind::NumE(value) => string_of_num(value),
        ExpKind::TextE(text) => format!("\"{}\"", escaped(text)),
        ExpKind::VarE(id) => string_of_varid(id),
        ExpKind::UnE(op, _, exp) => format!("{}{}", string_of_unop(*op), string_of_exp(exp)),
        ExpKind::BinE(op, _, left, right) => format!(
            "({} {} {})",
            string_of_exp(left),
            string_of_binop(*op),
            string_of_exp(right)
        ),
        ExpKind::CmpE(op, _, left, right) => format!(
            "({} {} {})",
            string_of_exp(left),
            string_of_cmpop(*op),
            string_of_exp(right)
        ),
        ExpKind::UpCastE(typ, exp) | ExpKind::DownCastE(typ, exp) => {
            format!("{} as {}", string_of_exp(exp), string_of_typ(typ))
        }
        ExpKind::SubE(exp, typ, _) => format!("{} <: {}", string_of_exp(exp), string_of_typ(typ)),
        ExpKind::MatchE(exp, pattern) => format!(
            "{} matches {}",
            string_of_exp(exp),
            string_of_pattern(pattern)
        ),
        ExpKind::TupleE(exps) => format!("({})", string_of_exps(", ", exps)),
        ExpKind::CaseE(notexp) => string_of_notexp(notexp),
        ExpKind::StrE(fields) => format!(
            "{{{}}}",
            join(fields, ", ", |(atom, exp)| format!(
                "{} {}",
                string_of_atom(atom),
                string_of_exp(exp)
            ))
        ),
        ExpKind::OptE(exp) => format!(
            "?({})",
            string_of_exps("", exp.as_deref().map_or(&[], std::slice::from_ref))
        ),
        ExpKind::ListE(exps) => format!("[{}]", string_of_exps(", ", exps)),
        ExpKind::ConsE(head, tail) => format!("{} :: {}", string_of_exp(head), string_of_exp(tail)),
        ExpKind::CatE(left, right) => {
            format!("{} ++ {}", string_of_exp(left), string_of_exp(right))
        }
        ExpKind::MemE(element, set) => {
            format!("{} <- {}", string_of_exp(element), string_of_exp(set))
        }
        ExpKind::LenE(exp) => format!("|{}|", string_of_exp(exp)),
        ExpKind::DotE(exp, atom) => format!("{}.{}", string_of_exp(exp), string_of_atom(atom)),
        ExpKind::IdxE(base, index) => format!("{}[{}]", string_of_exp(base), string_of_exp(index)),
        ExpKind::SliceE(base, low, high) => format!(
            "{}[{} : {}]",
            string_of_exp(base),
            string_of_exp(low),
            string_of_exp(high)
        ),
        ExpKind::UpdE(base, path, field) => format!(
            "{}[{} = {}]",
            string_of_exp(base),
            string_of_path(path),
            string_of_exp(field)
        ),
        ExpKind::CallE(id, targs, args) => format!(
            "{}{}{}",
            string_of_defid(id),
            string_of_targs(targs),
            string_of_args(args)
        ),
        ExpKind::IterE(exp, iterexp) => {
            format!("{}{}", string_of_exp(exp), string_of_iterexp(iterexp))
        }
    }
}
pub fn string_of_exps(separator: &str, exps: &[Exp]) -> String {
    join(exps, separator, string_of_exp)
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
            iterated.2.push(iterexp.0);
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
pub fn string_of_path(path: &Path) -> String {
    match &path.kind {
        PathKind::RootP => String::new(),
        PathKind::IdxP(path, exp) => format!("{}[{}]", string_of_path(path), string_of_exp(exp)),
        PathKind::SliceP(path, low, high) => format!(
            "{}[{} : {}]",
            string_of_path(path),
            string_of_exp(low),
            string_of_exp(high)
        ),
        PathKind::DotP(path, atom) if matches!(path.kind, PathKind::RootP) => string_of_atom(atom),
        PathKind::DotP(path, atom) => format!("{}.{}", string_of_path(path), string_of_atom(atom)),
    }
}
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
pub fn string_of_targ(targ: &Targ) -> String {
    string_of_typ(targ)
}
pub fn string_of_targs(targs: &[Targ]) -> String {
    if targs.is_empty() {
        String::new()
    } else {
        format!("<{}>", join(targs, ", ", string_of_targ))
    }
}
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
        iterated.2.push(iterprem.0);
        format!(
            "{} {} {}",
            string_of_var(variable),
            arrow,
            string_of_var(&iterated)
        )
    };
    format!(
        "{}{{{}}}",
        string_of_iter(iterprem.0),
        join(
            &iterprem
                .1
                .iter()
                .map(|var| render(var, "<-"))
                .chain(iterprem.2.iter().map(|var| render(var, "->")))
                .collect::<Vec<_>>(),
            ", ",
            Clone::clone
        )
    )
}
pub fn string_of_iterprems(iterprems: &[IterPrem]) -> String {
    join(iterprems, "", string_of_iterprem)
}
pub fn string_of_rule(rule: &Rule) -> String {
    format!(
        "rule {}: {}{}",
        string_of_ruleid(&rule.node.0),
        string_of_notexp(&rule.node.1),
        string_of_prems_with(2, &rule.node.2)
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
pub fn string_of_clause(index: i64, clause: &Clause) -> String {
    format!(
        "clause {index} : {} = {}{}",
        string_of_args(&clause.node.0),
        string_of_exp(&clause.node.1),
        string_of_prems_with(1, &clause.node.2)
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
pub fn string_of_def(definition: &Def) -> String {
    match &definition.node {
        DefKind::ExternTypD(id, _) => format!("extern syntax {}", string_of_typid(id)),
        DefKind::TypD(id, tparams, deftyp, _) => format!(
            "syntax {}{} = {}",
            string_of_typid(id),
            string_of_tparams(tparams),
            string_of_deftyp(deftyp)
        ),
        DefKind::VarD(id, typ, _) => {
            format!("var {} : {}", string_of_varid(id), string_of_typ(typ))
        }
        DefKind::ExternRelD(id, nottyp, _, _) => format!(
            "extern relation {}: {}",
            string_of_relid(id),
            string_of_nottyp(nottyp)
        ),
        DefKind::RelD(id, nottyp, _, groups, elsegroup, _) => format!(
            "relation {}: {}\n\n{}{}",
            string_of_relid(id),
            string_of_nottyp(nottyp),
            string_of_rulegroups(groups),
            string_of_elsegroup_opt(elsegroup)
        ),
        DefKind::ExternDecD(id, tparams, params, typ, _) => format!(
            "extern def {}{}{} : {}",
            string_of_defid(id),
            string_of_tparams(tparams),
            string_of_params(params),
            string_of_typ(typ)
        ),
        DefKind::BuiltinDecD(id, tparams, params, typ, _) => format!(
            "builtin def {}{}{} : {}",
            string_of_defid(id),
            string_of_tparams(tparams),
            string_of_params(params),
            string_of_typ(typ)
        ),
        DefKind::TableDecD(id, params, typ, rows, _) => format!(
            "tbl def {}{} : {} ={}",
            string_of_defid(id),
            string_of_params(params),
            string_of_typ(typ),
            string_of_tablerows(rows)
        ),
        DefKind::FuncDecD(id, tparams, params, typ, clauses, elseclause, _) => format!(
            "def {}{}{} : {} ={}{}",
            string_of_defid(id),
            string_of_tparams(tparams),
            string_of_params(params),
            string_of_typ(typ),
            string_of_clauses(clauses),
            string_of_elseclause_opt(elseclause)
        ),
    }
}
pub fn string_of_defs(definitions: &[Def]) -> String {
    join(definitions, "\n\n", string_of_def)
}
pub fn string_of_spec(spec: &Spec) -> String {
    string_of_defs(spec)
}
