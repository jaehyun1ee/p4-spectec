//! Text rendering for algorithmic-language data

use std::fmt;

use crate::{
    domain::mixop::Mixop,
    lang::{
        hints::input::InputHint,
        il::print::{
            self as il_print, string_of_args, string_of_atom, string_of_clauses, string_of_def_typ,
            string_of_defid, string_of_elseclause_opt, string_of_exp, string_of_exps,
            string_of_not_typ, string_of_params, string_of_relid, string_of_rulegroupid,
            string_of_tparams, string_of_typ, string_of_typid, string_of_varid,
        },
    },
};

use super::ast::*;

// - Identifiers

/// Renders rulepathid
pub fn string_of_rulepathid(id: &Id) -> String {
    id.node.clone()
}

fn indent(level: usize) -> String {
    "  ".repeat(level)
}

fn fill_notation(not_typ: &NotTyp, exps: Vec<String>) -> String {
    let (mixop, typs) = not_typ.node.split();
    assert_eq!(typs.len(), exps.len());
    Mixop::fill(&mixop, exps)
        .expect("notation arguments came from the same split notation")
        .render(string_of_atom, Clone::clone)
}

// - Rules

/// Renders relation input expressions
///
/// `exps_input` must match `input_hint.indices()`;
/// notation arity must be internally valid
pub fn string_of_ruleinput(not_typ: &NotTyp, input_hint: &InputHint, exps_input: &[Exp]) -> String {
    let mut output = String::new();
    write_ruleinput(&mut output, not_typ, input_hint, exps_input)
        .expect("writing to a String cannot fail");
    output
}

fn write_ruleinput(
    output: &mut dyn fmt::Write,
    not_typ: &NotTyp,
    input_hint: &InputHint,
    exps_input: &[Exp],
) -> fmt::Result {
    let input_indices = input_hint.indices();
    assert_eq!(input_indices.len(), exps_input.len());
    let (_, typs) = not_typ.node.split();
    let exps = (0..typs.len())
        .map(|index| {
            input_indices
                .iter()
                .zip(exps_input)
                .find_map(|(input, exp)| {
                    (*input == index as i64).then(|| il_print::string_of_exp(exp))
                })
                .unwrap_or_else(|| "%".to_owned())
        })
        .collect();
    output.write_str(&fill_notation(not_typ, exps))
}

/// Renders relation output expressions
///
/// `exps_output` must cover every non-input notation position;
/// notation arity must be internally valid
pub fn string_of_ruleoutput(
    not_typ: &NotTyp,
    input_hint: &InputHint,
    exps_output: &[Exp],
) -> String {
    let mut output = String::new();
    write_ruleoutput(&mut output, not_typ, input_hint, exps_output)
        .expect("writing to a String cannot fail");
    output
}

fn write_ruleoutput(
    output: &mut dyn fmt::Write,
    not_typ: &NotTyp,
    input_hint: &InputHint,
    exps_output: &[Exp],
) -> fmt::Result {
    let input_indices = input_hint.indices();
    let (_, typs) = not_typ.node.split();
    let outputs = (0..typs.len())
        .filter(|index| !input_indices.contains(&(*index as i64)))
        .collect::<Vec<_>>();
    assert_eq!(outputs.len(), exps_output.len());
    if exps_output.is_empty() {
        output.write_str("-- the relation holds")
    } else {
        let exps = (0..typs.len())
            .map(|index| {
                outputs
                    .iter()
                    .zip(exps_output)
                    .find_map(|(output, exp)| {
                        (*output == index).then(|| il_print::string_of_exp(exp))
                    })
                    .unwrap_or_else(|| "%".to_owned())
            })
            .collect();
        write!(output, "-- output: {}", fill_notation(not_typ, exps))
    }
}

/// Renders rulematch
pub fn string_of_rulematch(
    not_typ: &NotTyp,
    input_hint: &InputHint,
    rule_match: &RuleMatch,
) -> String {
    let mut output = String::new();
    write_rulematch(&mut output, not_typ, input_hint, rule_match)
        .expect("writing to a String cannot fail");
    output
}

fn write_rulematch(
    output: &mut dyn fmt::Write,
    not_typ: &NotTyp,
    input_hint: &InputHint,
    rule_match: &RuleMatch,
) -> fmt::Result {
    write!(output, "{}(signature) ", indent(2))?;
    write_ruleinput(output, not_typ, input_hint, &rule_match.exps_signature)?;
    output.write_char('\n')?;
    output.write_str(&indent(2))?;
    write_ruleinput(output, not_typ, input_hint, &rule_match.exps_input)?;
    output.write_str(&il_print::string_of_prems_with(2, &rule_match.prems))
}

/// Renders rulepath
pub fn string_of_rulepath(
    not_typ: &NotTyp,
    input_hint: &InputHint,
    rule_path: &RulePath,
) -> String {
    let mut output = String::new();
    write_rulepath(&mut output, not_typ, input_hint, rule_path)
        .expect("writing to a String cannot fail");
    output
}

fn write_rulepath(
    output: &mut dyn fmt::Write,
    not_typ: &NotTyp,
    input_hint: &InputHint,
    rule_path: &RulePath,
) -> fmt::Result {
    write!(
        output,
        "{}rulepath {}{}\n{}",
        indent(2),
        string_of_rulepathid(&rule_path.id),
        il_print::string_of_prems_with(2, &rule_path.prems),
        indent(2)
    )?;
    write_ruleoutput(output, not_typ, input_hint, &rule_path.exps_output)
}

/// Renders rulepaths
pub fn string_of_rulepaths(
    not_typ: &NotTyp,
    input_hint: &InputHint,
    rule_paths: &[RulePath],
) -> String {
    let mut output = String::new();
    write_rulepaths(&mut output, not_typ, input_hint, rule_paths)
        .expect("writing to a String cannot fail");
    output
}

fn write_rulepaths(
    output: &mut dyn fmt::Write,
    not_typ: &NotTyp,
    input_hint: &InputHint,
    rule_paths: &[RulePath],
) -> fmt::Result {
    for (index, rule_path) in rule_paths.iter().enumerate() {
        if index != 0 {
            output.write_str("\n\n")?;
        }
        write_rulepath(output, not_typ, input_hint, rule_path)?;
    }
    Ok(())
}

/// Renders rulegroup
pub fn string_of_rulegroup(
    not_typ: &NotTyp,
    input_hint: &InputHint,
    rule_group: &RuleGroup,
) -> String {
    let mut output = String::new();
    write_rulegroup(&mut output, not_typ, input_hint, rule_group)
        .expect("writing to a String cannot fail");
    output
}

fn write_rulegroup(
    output: &mut dyn fmt::Write,
    not_typ: &NotTyp,
    input_hint: &InputHint,
    rule_group: &RuleGroup,
) -> fmt::Result {
    write!(
        output,
        "{}rulegroup {}\n\n {}match\n\n",
        indent(1),
        string_of_rulegroupid(&rule_group.node.id),
        indent(1)
    )?;
    write_rulematch(output, not_typ, input_hint, &rule_group.node.rule_match)?;
    write!(output, "\n\n {}paths\n\n", indent(1))?;
    write_rulepaths(output, not_typ, input_hint, &rule_group.node.rule_paths)
}

/// Renders rulegroups
pub fn string_of_rulegroups(
    not_typ: &NotTyp,
    input_hint: &InputHint,
    rule_groups: &[RuleGroup],
) -> String {
    let mut output = String::new();
    write_rulegroups(&mut output, not_typ, input_hint, rule_groups)
        .expect("writing to a String cannot fail");
    output
}

fn write_rulegroups(
    output: &mut dyn fmt::Write,
    not_typ: &NotTyp,
    input_hint: &InputHint,
    rule_groups: &[RuleGroup],
) -> fmt::Result {
    for (index, rule_group) in rule_groups.iter().enumerate() {
        if index != 0 {
            output.write_str("\n\n")?;
        }
        write_rulegroup(output, not_typ, input_hint, rule_group)?;
    }
    Ok(())
}

/// Renders elsegroup
pub fn string_of_elsegroup(
    not_typ: &NotTyp,
    input_hint: &InputHint,
    else_group: &ElseGroup,
) -> String {
    let mut output = String::new();
    write_elsegroup(&mut output, not_typ, input_hint, else_group)
        .expect("writing to a String cannot fail");
    output
}

fn write_elsegroup(
    output: &mut dyn fmt::Write,
    not_typ: &NotTyp,
    input_hint: &InputHint,
    else_group: &ElseGroup,
) -> fmt::Result {
    write!(
        output,
        "{}rulegroup {}\n\n {}match\n\n",
        indent(1),
        string_of_rulegroupid(&else_group.node.id),
        indent(1)
    )?;
    write_rulematch(output, not_typ, input_hint, &else_group.node.rule_match)?;
    write!(output, "\n\n {}paths\n\n", indent(1))?;
    write_rulepaths(
        output,
        not_typ,
        input_hint,
        std::slice::from_ref(&else_group.node.rule_path),
    )
}

/// Renders elsegroup opt
pub fn string_of_elsegroup_opt(
    not_typ: &NotTyp,
    input_hint: &InputHint,
    else_group: &Option<ElseGroup>,
) -> String {
    let mut output = String::new();
    write_elsegroup_opt(&mut output, not_typ, input_hint, else_group)
        .expect("writing to a String cannot fail");
    output
}

fn write_elsegroup_opt(
    output: &mut dyn fmt::Write,
    not_typ: &NotTyp,
    input_hint: &InputHint,
    else_group: &Option<ElseGroup>,
) -> fmt::Result {
    if let Some(else_group) = else_group {
        write!(output, "\n\n{}elsegroup\n\n", indent(1))?;
        write_elsegroup(output, not_typ, input_hint, else_group)?;
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
    write!(
        output,
        "\n{}(signature) {}\n{}{} -> {}{}",
        indent(2),
        string_of_exps(", ", &table_row.node.exps_signature),
        indent(2),
        string_of_args(&table_row.node.args),
        string_of_exp(&table_row.node.exp),
        il_print::string_of_prems_with(2, &table_row.node.prems)
    )
}

/// Renders tablerows
pub fn string_of_tablerows(table_rows: &[TableRow]) -> String {
    let mut output = String::new();
    write_tablerows(&mut output, table_rows).expect("writing to a String cannot fail");
    output
}

fn write_tablerows(output: &mut dyn fmt::Write, table_rows: &[TableRow]) -> fmt::Result {
    for (index, table_row) in table_rows.iter().enumerate() {
        write!(output, "\n{}row {index} :", indent(1))?;
        write_tablerow(output, table_row)?;
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
        DefKind::ExternTyp(ExternTypDef { id, .. }) => {
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
        DefKind::ExternRel(ExternRelDef { id, not_typ, .. }) => write!(
            output,
            "extern relation {}: {}",
            string_of_relid(id),
            string_of_not_typ(not_typ)
        ),
        DefKind::Rel(RelDef {
            id,
            not_typ,
            input_hint,
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
            write_rulegroups(output, not_typ, input_hint, rule_groups)?;
            write_elsegroup_opt(output, not_typ, input_hint, else_group)
        }
        DefKind::ExternDec(ExternDecDef {
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
        DefKind::BuiltinDec(BuiltinDecDef {
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
        DefKind::TableDec(TableDecDef {
            id,
            params,
            typ,
            table_rows,
            ..
        }) => {
            write!(
                output,
                "tbl def {}{} : {} =",
                string_of_defid(id),
                string_of_params(params),
                string_of_typ(typ)
            )?;
            write_tablerows(output, table_rows)
        }
        DefKind::FuncDec(FuncDecDef {
            id,
            tparams,
            params,
            typ,
            clauses,
            else_clause,
            ..
        }) => write!(
            output,
            "def {}{}{} : {} ={}{}",
            string_of_defid(id),
            string_of_tparams(tparams),
            string_of_params(params),
            string_of_typ(typ),
            string_of_clauses(clauses),
            string_of_elseclause_opt(else_clause)
        ),
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
