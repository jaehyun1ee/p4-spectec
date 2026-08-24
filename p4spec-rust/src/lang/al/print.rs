use crate::{domain::mixfix::Mixop, lang::il::print as il_print};

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

pub fn string_of_ruleinput(nottyp: &NotTyp, inputs: &[i64], exps_input: &[Exp]) -> String {
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

pub fn string_of_ruleoutput(nottyp: &NotTyp, inputs: &[i64], exps_output: &[Exp]) -> String {
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

pub fn string_of_rulematch(nottyp: &NotTyp, inputs: &[i64], rulematch: &RuleMatch) -> String {
    let (exps_signature, exps_input, prems) = rulematch;
    format!(
        "{}(signature) {}\n{}{}{}",
        indent(2),
        string_of_ruleinput(nottyp, inputs, exps_signature),
        indent(2),
        string_of_ruleinput(nottyp, inputs, exps_input),
        il_print::string_of_prems_with(2, prems)
    )
}

pub fn string_of_rulepath(nottyp: &NotTyp, inputs: &[i64], rulepath: &RulePath) -> String {
    let (id, prems, exps_output) = rulepath;
    format!(
        "{}rulepath {}{}\n{}{}",
        indent(2),
        string_of_rulepathid(id),
        il_print::string_of_prems_with(2, prems),
        indent(2),
        string_of_ruleoutput(nottyp, inputs, exps_output)
    )
}

pub fn string_of_rulepaths(nottyp: &NotTyp, inputs: &[i64], rulepaths: &[RulePath]) -> String {
    rulepaths
        .iter()
        .map(|rulepath| string_of_rulepath(nottyp, inputs, rulepath))
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub fn string_of_rulegroup(nottyp: &NotTyp, inputs: &[i64], rulegroup: &RuleGroup) -> String {
    let (id, rulematch, rulepaths) = &rulegroup.node;
    format!(
        "{}rulegroup {}\n\n {}match\n\n{}\n\n {}paths\n\n{}",
        indent(1),
        string_of_rulegroupid(id),
        indent(1),
        string_of_rulematch(nottyp, inputs, rulematch),
        indent(1),
        string_of_rulepaths(nottyp, inputs, rulepaths)
    )
}

pub fn string_of_rulegroups(nottyp: &NotTyp, inputs: &[i64], rulegroups: &[RuleGroup]) -> String {
    rulegroups
        .iter()
        .map(|rulegroup| string_of_rulegroup(nottyp, inputs, rulegroup))
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub fn string_of_elsegroup(nottyp: &NotTyp, inputs: &[i64], elsegroup: &ElseGroup) -> String {
    let (id, rulematch, rulepath) = &elsegroup.node;
    format!(
        "{}rulegroup {}\n\n {}match\n\n{}\n\n {}paths\n\n{}",
        indent(1),
        string_of_rulegroupid(id),
        indent(1),
        string_of_rulematch(nottyp, inputs, rulematch),
        indent(1),
        string_of_rulepaths(nottyp, inputs, std::slice::from_ref(rulepath))
    )
}

pub fn string_of_elsegroup_opt(
    nottyp: &NotTyp,
    inputs: &[i64],
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
    let (exps_signature, args, exp, prems) = &tablerow.node;
    format!(
        "\n{}(signature) {}\n{}{} -> {}{}",
        indent(2),
        string_of_exps(", ", exps_signature),
        indent(2),
        string_of_args(args),
        string_of_exp(exp),
        il_print::string_of_prems_with(2, prems)
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
        DefKind::RelD(id, nottyp, inputs, rulegroups, elsegroup, _) => format!(
            "relation {}: {}\n\n{}{}",
            string_of_relid(id),
            string_of_nottyp(nottyp),
            string_of_rulegroups(nottyp, inputs, rulegroups),
            string_of_elsegroup_opt(nottyp, inputs, elsegroup)
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
        DefKind::TableDecD(id, params, typ, tablerows, _) => format!(
            "tbl def {}{} : {} ={}",
            string_of_defid(id),
            string_of_params(params),
            string_of_typ(typ),
            string_of_tablerows(tablerows)
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
    definitions
        .iter()
        .map(string_of_def)
        .collect::<Vec<_>>()
        .join("\n\n")
}

// Spec

pub fn string_of_spec(spec: &Spec) -> String {
    string_of_defs(spec)
}
