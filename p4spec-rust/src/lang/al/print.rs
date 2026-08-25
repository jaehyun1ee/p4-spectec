use std::fmt;

use crate::{
    domain::mixfix::Mixop,
    lang::{hints::input::InputHint, il::print as il_print},
};

use super::ast::*;

// Numbers

pub use crate::lang::il::print::string_of_num;

// Texts

pub use crate::lang::il::print::string_of_text;

// Identifiers

pub use crate::lang::il::print::{
    string_of_defid, string_of_relid, string_of_rulegroupid, string_of_typid, string_of_varid,
};

pub fn string_of_rulepathid(id: &Id) -> String {
    id.node.clone()
}

// Atoms

pub use crate::lang::il::print::{string_of_atom, string_of_atoms};

// Mixfix operators

pub use crate::lang::il::print::string_of_mixop;

// Iterators

pub use crate::lang::il::print::string_of_iter;

// Variables

pub use crate::lang::il::print::string_of_var;

// Types

pub use crate::lang::il::print::{
    string_of_deftyp, string_of_nottyp, string_of_typ, string_of_typcase, string_of_typcases,
    string_of_typfield, string_of_typfields, string_of_typs,
};

// Values

pub use crate::lang::il::print::{string_of_short_value, string_of_value, string_of_value_with};

// Operators

pub use crate::lang::il::print::{string_of_binop, string_of_cmpop, string_of_unop};

// Expressions

pub use crate::lang::il::print::{
    string_of_exp, string_of_exps, string_of_iterexp, string_of_iterexps, string_of_notexp,
};

// Patterns

pub use crate::lang::il::print::string_of_pattern;

// Paths

pub use crate::lang::il::print::string_of_path;

// Parameters

pub use crate::lang::il::print::{string_of_param, string_of_params};

// Type parameters

pub use crate::lang::il::print::{string_of_tparam, string_of_tparams};

// Arguments

pub use crate::lang::il::print::{string_of_arg, string_of_args};

// Type arguments

pub use crate::lang::il::print::{string_of_targ, string_of_targs};

// Premises

pub use crate::lang::il::print::{string_of_prem, string_of_prems, string_of_prems_with};

fn indent(level: usize) -> String {
    "  ".repeat(level)
}

fn fill_notation(nottyp: &NotTyp, exps: Vec<String>) -> String {
    let (mixop, typs) = nottyp.node.split();
    assert_eq!(typs.len(), exps.len());
    Mixop::fill(&mixop, exps)
        .expect("notation arguments came from the same split notation")
        .render(string_of_atom, Clone::clone)
}

// Rules

/// Renders relation input expressions
///
/// `exps_input` must match `inputs.indices()`;
/// notation arity must be internally valid
pub fn string_of_ruleinput(nottyp: &NotTyp, inputs: &InputHint, exps_input: &[Exp]) -> String {
    let inputs = inputs.indices();
    assert_eq!(inputs.len(), exps_input.len());
    let (_, typs) = nottyp.node.split();
    let exps = (0..typs.len())
        .map(|index| {
            inputs
                .iter()
                .zip(exps_input)
                .find_map(|(input, exp)| {
                    (*input == index as i64).then(|| il_print::string_of_exp(exp))
                })
                .unwrap_or_else(|| "%".to_owned())
        })
        .collect();
    fill_notation(nottyp, exps)
}

/// Renders relation output expressions
///
/// `exps_output` must cover every non-input notation position;
/// notation arity must be internally valid
pub fn string_of_ruleoutput(nottyp: &NotTyp, inputs: &InputHint, exps_output: &[Exp]) -> String {
    let inputs = inputs.indices();
    let (_, typs) = nottyp.node.split();
    let outputs = (0..typs.len())
        .filter(|index| !inputs.contains(&(*index as i64)))
        .collect::<Vec<_>>();
    assert_eq!(outputs.len(), exps_output.len());
    if exps_output.is_empty() {
        "-- the relation holds".to_owned()
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
        format!("-- output: {}", fill_notation(nottyp, exps))
    }
}

pub fn string_of_rulematch(nottyp: &NotTyp, inputs: &InputHint, rulematch: &RuleMatch) -> String {
    format!(
        "{}(signature) {}\n{}{}{}",
        indent(2),
        string_of_ruleinput(nottyp, inputs, &rulematch.signature),
        indent(2),
        string_of_ruleinput(nottyp, inputs, &rulematch.inputs),
        il_print::string_of_prems_with(2, &rulematch.premises)
    )
}

pub fn string_of_rulepath(nottyp: &NotTyp, inputs: &InputHint, rulepath: &RulePath) -> String {
    format!(
        "{}rulepath {}{}\n{}{}",
        indent(2),
        string_of_rulepathid(&rulepath.rule_id),
        il_print::string_of_prems_with(2, &rulepath.premises),
        indent(2),
        string_of_ruleoutput(nottyp, inputs, &rulepath.outputs)
    )
}

pub fn string_of_rulepaths(nottyp: &NotTyp, inputs: &InputHint, rulepaths: &[RulePath]) -> String {
    rulepaths
        .iter()
        .map(|rulepath| string_of_rulepath(nottyp, inputs, rulepath))
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub fn string_of_rulegroup(nottyp: &NotTyp, inputs: &InputHint, rulegroup: &RuleGroup) -> String {
    format!(
        "{}rulegroup {}\n\n {}match\n\n{}\n\n {}paths\n\n{}",
        indent(1),
        string_of_rulegroupid(&rulegroup.node.id),
        indent(1),
        string_of_rulematch(nottyp, inputs, &rulegroup.node.rule_match),
        indent(1),
        string_of_rulepaths(nottyp, inputs, &rulegroup.node.paths)
    )
}

pub fn string_of_rulegroups(
    nottyp: &NotTyp,
    inputs: &InputHint,
    rulegroups: &[RuleGroup],
) -> String {
    rulegroups
        .iter()
        .map(|rulegroup| string_of_rulegroup(nottyp, inputs, rulegroup))
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub fn string_of_elsegroup(nottyp: &NotTyp, inputs: &InputHint, elsegroup: &ElseGroup) -> String {
    format!(
        "{}rulegroup {}\n\n {}match\n\n{}\n\n {}paths\n\n{}",
        indent(1),
        string_of_rulegroupid(&elsegroup.node.id),
        indent(1),
        string_of_rulematch(nottyp, inputs, &elsegroup.node.rule_match),
        indent(1),
        string_of_rulepaths(nottyp, inputs, std::slice::from_ref(&elsegroup.node.path))
    )
}

pub fn string_of_elsegroup_opt(
    nottyp: &NotTyp,
    inputs: &InputHint,
    elsegroup: &Option<ElseGroup>,
) -> String {
    elsegroup.as_ref().map_or_else(String::new, |elsegroup| {
        format!(
            "\n\n{}elsegroup\n\n{}",
            indent(1),
            string_of_elsegroup(nottyp, inputs, elsegroup)
        )
    })
}

// Clause

pub use crate::lang::il::print::{
    string_of_clause, string_of_clauses, string_of_elseclause, string_of_elseclause_opt,
};

// Table rows

pub fn string_of_tablerow(tablerow: &TableRow) -> String {
    format!(
        "\n{}(signature) {}\n{}{} -> {}{}",
        indent(2),
        string_of_exps(", ", &tablerow.node.signature),
        indent(2),
        string_of_args(&tablerow.node.args),
        string_of_exp(&tablerow.node.expression),
        il_print::string_of_prems_with(2, &tablerow.node.premises)
    )
}

pub fn string_of_tablerows(tablerows: &[TableRow]) -> String {
    tablerows
        .iter()
        .enumerate()
        .map(|(index, tablerow)| {
            format!(
                "\n{}row {index} :{}",
                indent(1),
                string_of_tablerow(tablerow)
            )
        })
        .collect()
}

// Hints

pub use crate::lang::il::print::{string_of_hint, string_of_hints};

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
        DefKind::RelD(id, nottyp, inputs, groups, elsegroup, _) => write!(
            output,
            "relation {}: {}\n\n{}{}",
            string_of_relid(id),
            string_of_nottyp(nottyp),
            string_of_rulegroups(nottyp, inputs, groups),
            string_of_elsegroup_opt(nottyp, inputs, elsegroup)
        ),
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
        DefKind::TableDecD(id, params, typ, rows, _) => write!(
            output,
            "tbl def {}{} : {} ={}",
            string_of_defid(id),
            string_of_params(params),
            string_of_typ(typ),
            string_of_tablerows(rows)
        ),
        DefKind::FuncDecD(id, tparams, params, typ, clauses, elseclause, _) => write!(
            output,
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
    definitions
        .iter()
        .map(string_of_def)
        .collect::<Vec<_>>()
        .join("\n\n")
}

// Spec

/// Renders a specification without source or hint metadata
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
