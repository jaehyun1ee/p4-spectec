use std::fmt;

use crate::{domain::mixop, lang::sl};

use super::ast::*;

fn join<Item>(items: &[Item], separator: &str, render: impl Fn(&Item) -> String) -> String {
    items.iter().map(render).collect::<Vec<_>>().join(separator)
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
    sl::print::string_of_num(number)
}

// Texts

pub fn string_of_text(text: &str) -> String {
    sl::print::string_of_text(text)
}

// Identifiers

pub fn string_of_varid(id: &Id) -> String {
    sl::print::string_of_varid(id)
}

pub fn string_of_typid(id: &Id) -> String {
    sl::print::string_of_typid(id)
}

pub fn string_of_relid(id: &Id) -> String {
    sl::print::string_of_relid(id)
}

pub fn string_of_relpathid(id: &Id) -> String {
    sl::print::string_of_relpathid(id)
}

pub fn string_of_defid(id: &Id) -> String {
    sl::print::string_of_defid(id)
}

// Atoms

pub fn string_of_atom(atom: &Atom) -> String {
    sl::print::string_of_atom(atom)
}

pub fn string_of_atoms(atoms: &[Atom]) -> String {
    join(atoms, "", string_of_atom)
}

// Mixfix operators

pub fn string_of_mixop(operator: &Mixop) -> String {
    sl::print::string_of_mixop(operator)
}

// Iterators

pub fn string_of_iter(iter: Iter) -> &'static str {
    sl::print::string_of_iter(iter)
}

pub fn string_of_iterexp(iterexp: &IterExp) -> String {
    sl::print::string_of_iterexp(iterexp)
}

pub fn string_of_iterexps(iterexps: &[IterExp]) -> String {
    sl::print::string_of_iterexps(iterexps)
}

// Variables

pub fn string_of_var(variable: &Var) -> String {
    sl::print::string_of_var(variable)
}

// Types

pub fn string_of_typ(typ: &Typ) -> String {
    sl::print::string_of_typ(typ)
}

pub fn string_of_typs(separator: &str, typs: &[Typ]) -> String {
    sl::print::string_of_typs(separator, typs)
}

pub fn string_of_nottyp(nottyp: &NotTyp) -> String {
    sl::print::string_of_nottyp(nottyp)
}

pub fn string_of_deftyp(deftyp: &DefTyp) -> String {
    sl::print::string_of_deftyp(deftyp)
}

pub fn string_of_typfield(field: &TypField) -> String {
    sl::print::string_of_typfield(field)
}

pub fn string_of_typfields(separator: &str, fields: &[TypField]) -> String {
    sl::print::string_of_typfields(separator, fields)
}

pub fn string_of_typcase(case: &TypCase) -> String {
    sl::print::string_of_typcase(case)
}

pub fn string_of_typcases(separator: &str, cases: &[TypCase]) -> String {
    sl::print::string_of_typcases(separator, cases)
}

// Values

pub fn string_of_vid(vid: i64) -> String {
    sl::print::string_of_vid(vid)
}

pub fn string_of_value(value: &Value) -> String {
    sl::print::string_of_value(value)
}

pub fn string_of_value_with(value: &Value, short: bool, level: usize) -> String {
    sl::print::string_of_value_with(value, short, level)
}

// Operators

pub fn string_of_unop(operation: UnOp) -> &'static str {
    sl::print::string_of_unop(operation)
}

pub fn string_of_binop(operation: BinOp) -> &'static str {
    sl::print::string_of_binop(operation)
}

pub fn string_of_cmpop(operation: CmpOp) -> &'static str {
    sl::print::string_of_cmpop(operation)
}

// Expressions

pub fn string_of_exp(exp: &Exp) -> String {
    let mut output = String::new();
    write_exp(&mut output, exp).expect("writing to a String cannot fail");
    output
}
fn write_exp(output: &mut dyn fmt::Write, exp: &Exp) -> fmt::Result {
    match &exp.node.kind {
        ExpKind::BoolE(value) => write!(output, "{value}"),
        ExpKind::NumE(value) => output.write_str(&string_of_num(value)),
        ExpKind::TextE(text) => write!(output, "\"{}\"", escaped(text)),
        ExpKind::VarE(id) => output.write_str(&id.node),
        ExpKind::UnE(op, _, exp) => {
            output.write_str(string_of_unop(*op))?;
            write_exp(output, exp)
        }
        ExpKind::BinE(op, _, l, r) => {
            output.write_char('(')?;
            write_exp(output, l)?;
            write!(output, " {} ", string_of_binop(*op))?;
            write_exp(output, r)?;
            output.write_char(')')
        }
        ExpKind::CmpE(op, _, l, r) => {
            output.write_char('(')?;
            write_exp(output, l)?;
            write!(output, " {} ", string_of_cmpop(*op))?;
            write_exp(output, r)?;
            output.write_char(')')
        }
        ExpKind::UpCastE(typ, exp) | ExpKind::DownCastE(typ, exp) => {
            output.write_char('(')?;
            write_exp(output, exp)?;
            write!(output, " as {})", string_of_typ(typ))
        }
        ExpKind::SubE(exp, typ, _) => {
            output.write_char('(')?;
            write_exp(output, exp)?;
            write!(output, " has type {})", string_of_typ(typ))
        }
        ExpKind::MatchE(exp, pat) => {
            output.write_char('(')?;
            write_exp(output, exp)?;
            write!(output, " matches pattern {})", string_of_pattern(pat))
        }
        ExpKind::TupleE(exps) => {
            output.write_char('(')?;
            write_exps(output, ", ", exps)?;
            output.write_char(')')
        }
        ExpKind::CaseE(n) => write!(output, "({})", string_of_notexp(n)),
        ExpKind::StrE(fields) => {
            output.write_char('{')?;
            for (i, (a, e)) in fields.iter().enumerate() {
                if i > 0 {
                    output.write_str(", ")?;
                }
                write!(output, "{} ", string_of_atom(a))?;
                write_exp(output, e)?;
            }
            output.write_char('}')
        }
        ExpKind::OptE(Some(exp)) => {
            output.write_str("?(")?;
            write_exp(output, exp)?;
            output.write_char(')')
        }
        ExpKind::OptE(None) => output.write_str("?()"),
        ExpKind::ListE(es) => {
            output.write_char('[')?;
            write_exps(output, ", ", es)?;
            output.write_char(']')
        }
        ExpKind::ConsE(h, t) => {
            write_exp(output, h)?;
            output.write_str(" :: ")?;
            write_exp(output, t)
        }
        ExpKind::CatE(l, r) => {
            write_exp(output, l)?;
            output.write_str(" ++ ")?;
            write_exp(output, r)
        }
        ExpKind::MemE(e, s) => {
            write_exp(output, e)?;
            output.write_str(" is in ")?;
            write_exp(output, s)
        }
        ExpKind::LenE(e) => {
            output.write_char('|')?;
            write_exp(output, e)?;
            output.write_char('|')
        }
        ExpKind::DotE(e, a) => {
            write_exp(output, e)?;
            write!(output, ".{}", string_of_atom(a))
        }
        ExpKind::IdxE(b, i) => {
            write_exp(output, b)?;
            output.write_char('[')?;
            write_exp(output, i)?;
            output.write_char(']')
        }
        ExpKind::SliceE(b, l, h) => {
            write_exp(output, b)?;
            output.write_char('[')?;
            write_exp(output, l)?;
            output.write_str(" : ")?;
            write_exp(output, h)?;
            output.write_char(']')
        }
        ExpKind::UpdE(b, p, f) => {
            write_exp(output, b)?;
            write!(output, "[{} = ", string_of_path(p))?;
            write_exp(output, f)?;
            output.write_char(']')
        }
        ExpKind::CallE(id, ts, as_) => write!(
            output,
            "{}{}{}",
            string_of_defid(id),
            string_of_targs(ts),
            string_of_args(as_)
        ),
        ExpKind::IterE(e, i) => {
            output.write_char('(')?;
            write_exp(output, e)?;
            write!(output, "){}", string_of_iterexps(std::slice::from_ref(i)))
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
        if index > 0 {
            output.write_str(separator)?;
        }
        write_exp(output, exp)?;
    }
    Ok(())
}

pub fn string_of_notexp(notexp: &NotExp) -> String {
    notexp.render(string_of_atom, string_of_exp)
}

// Patterns

pub fn string_of_pattern(pattern: &Pattern) -> String {
    sl::print::string_of_pattern(pattern)
}

// Paths

pub fn string_of_path(path: &Path) -> String {
    match &path.kind {
        PathKind::RootP => String::new(),
        PathKind::IdxP(path, exp) => format!("{}[{}]", string_of_path(path), string_of_exp(exp)),
        PathKind::SliceP(path, exp_l, exp_h) => format!(
            "{}[{} : {}]",
            string_of_path(path),
            string_of_exp(exp_l),
            string_of_exp(exp_h)
        ),
        PathKind::DotP(path, atom) if matches!(path.kind, PathKind::RootP) => string_of_atom(atom),
        PathKind::DotP(path, atom) => {
            format!("{}.{}", string_of_path(path), string_of_atom(atom))
        }
    }
}

// Parameters

pub fn string_of_param(param: &Param) -> String {
    match &param.node {
        ParamKind::ExpP(_, exp) => string_of_exp(exp),
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
    sl::print::string_of_tparam(tparam)
}

pub fn string_of_tparams(tparams: &[TParam]) -> String {
    sl::print::string_of_tparams(tparams)
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
    sl::print::string_of_targ(targ)
}

pub fn string_of_targs(targs: &[Targ]) -> String {
    sl::print::string_of_targs(targs)
}

// Danglings

pub fn string_of_dangle(iid: Iid) -> String {
    format!("Dangling#{iid}")
}

// Case analysis

type TierPrinter<Tier> = fn(&Tier, bool, usize, usize) -> String;

fn string_of_case_with<Tier>(
    case: &Case<Tier>,
    tier_printer: TierPrinter<Tier>,
    level: usize,
    index: usize,
) -> String {
    format!(
        "{}{index}. Case {}\n\n{}",
        "  ".repeat(level),
        string_of_guard(&case.guard),
        string_of_block_with(&case.block, tier_printer, level + 1, 0)
    )
}

fn string_of_cases_with<Tier>(
    cases: &[Case<Tier>],
    tier_printer: TierPrinter<Tier>,
    level: usize,
) -> String {
    cases
        .iter()
        .enumerate()
        .map(|(index, case)| string_of_case_with(case, tier_printer, level, index + 1))
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub fn string_of_guard(guard: &Guard) -> String {
    match guard {
        Guard::BoolG(value) => value.to_string(),
        Guard::CmpG(operation, _, exp) => {
            format!("(% {} {})", string_of_cmpop(*operation), string_of_exp(exp))
        }
        Guard::SubG(typ, _) => format!("(% has type {})", string_of_typ(typ)),
        Guard::MatchG(pattern) => {
            format!("(% matches pattern {})", string_of_pattern(pattern))
        }
        Guard::MemG(exp) => format!("(% is in {})", string_of_exp(exp)),
        Guard::CheckLetSubG(typ, _, exp) => format!(
            "(let {} be %, % has type {})",
            string_of_exp(exp),
            string_of_typ(typ)
        ),
        Guard::CheckLetMatchG(pattern, exp) => format!(
            "(let {} be %, % matches pattern {})",
            string_of_exp(exp),
            string_of_pattern(pattern)
        ),
    }
}

// Instructions: shared control flow, parametric over the tier

fn string_of_instr_with<Tier>(
    instr: &Instr<Tier>,
    tier_printer: TierPrinter<Tier>,
    short: bool,
    level: usize,
    index: usize,
) -> String {
    let order = format!("{}{index}. ", "  ".repeat(level));
    match &instr.node.kind {
        InstrKind::IfI(exp, iterexps, block, dangle) => {
            let summary = format!(
                "If ({}){}, then",
                string_of_exp(exp),
                string_of_iterexps(iterexps)
            );
            if short {
                summary
            } else {
                format!(
                    "{order}{summary}\n\n{}{}",
                    string_of_block_with(block, tier_printer, level + 1, 0),
                    if *dangle {
                        format!("\n\n{order}Else {}", string_of_dangle(instr.node.iid))
                    } else {
                        String::new()
                    }
                )
            }
        }
        InstrKind::HoldI(id, notexp, iterexps, holdcase) => {
            let holding = |negative: bool| {
                format!(
                    "If ({}: {}){} {}, then",
                    string_of_relid(id),
                    string_of_notexp(notexp),
                    string_of_iterexps(iterexps),
                    if negative { "does not hold" } else { "holds" }
                )
            };
            match holdcase {
                HoldCase::BothH(block_hold, block_not_hold) => {
                    let summary = holding(false);
                    if short {
                        summary
                    } else {
                        format!(
                            "{order}{summary}\n\n{}\n\n{order}Else,\n\n{}",
                            string_of_block_with(block_hold, tier_printer, level + 1, 0),
                            string_of_block_with(block_not_hold, tier_printer, level + 1, 0)
                        )
                    }
                }
                HoldCase::HoldH(block, dangle) | HoldCase::NotHoldH(block, dangle) => {
                    let summary = holding(matches!(holdcase, HoldCase::NotHoldH(..)));
                    if short {
                        summary
                    } else {
                        format!(
                            "{order}{summary}\n\n{}{}",
                            string_of_block_with(block, tier_printer, level + 1, 0),
                            if *dangle {
                                format!("\n\n{order}Else {}", string_of_dangle(instr.node.iid))
                            } else {
                                String::new()
                            }
                        )
                    }
                }
            }
        }
        InstrKind::CaseI(exp, cases, dangle) => {
            let summary = format!("Case analysis on {}", string_of_exp(exp));
            if short {
                summary
            } else {
                format!(
                    "{order}{summary}\n\n{}{}",
                    string_of_cases_with(cases, tier_printer, level + 1),
                    if *dangle {
                        format!("\n\n{order}Else {}", string_of_dangle(instr.node.iid))
                    } else {
                        String::new()
                    }
                )
            }
        }
        InstrKind::LetI(exp_l, exp_r, iterinstrs) => render_leaf(
            short,
            &order,
            format!(
                "(Let {} be {}){}",
                string_of_exp(exp_l),
                string_of_exp(exp_r),
                string_of_iterinstrs(iterinstrs)
            ),
        ),
        InstrKind::DebugI(exp) => {
            render_leaf(short, &order, format!("Debug: {}", string_of_exp(exp)))
        }
        InstrKind::DestructI(fields, exp_r) => render_leaf(
            short,
            &order,
            format!(
                "(Destruct ({}) = {})",
                join(fields, ", ", |(_, exp)| string_of_exp(exp)),
                string_of_exp(exp_r)
            ),
        ),
        InstrKind::CheckLetSubI(typ, _, exp_l, exp_r, block) => render_nested(
            short,
            &order,
            format!(
                "(Let {} be {}, {} has type {})",
                string_of_exp(exp_l),
                string_of_exp(exp_r),
                string_of_exp(exp_r),
                string_of_typ(typ)
            ),
            string_of_block_with(block, tier_printer, level + 1, 0),
        ),
        InstrKind::CheckLetMatchI(pattern, exp_l, exp_r, block) => render_nested(
            short,
            &order,
            format!(
                "(Let {} be {}, {} matches pattern {})",
                string_of_exp(exp_l),
                string_of_exp(exp_r),
                string_of_exp(exp_r),
                string_of_pattern(pattern)
            ),
            string_of_block_with(block, tier_printer, level + 1, 0),
        ),
        InstrKind::OptionGetI(exp_l, exp_r, block) => render_nested(
            short,
            &order,
            format!(
                "(Let {} be ! {})",
                string_of_exp(exp_l),
                string_of_exp(exp_r)
            ),
            string_of_block_with(block, tier_printer, level + 1, 0),
        ),
        InstrKind::TierI(tier) => tier_printer(tier, short, level, index),
    }
}

fn render_leaf(short: bool, order: &str, summary: String) -> String {
    if short {
        summary
    } else {
        format!("{order}{summary}")
    }
}

fn render_nested(short: bool, order: &str, summary: String, nested: String) -> String {
    if short {
        summary
    } else {
        format!("{order}{summary}\n\n{nested}")
    }
}

fn string_of_block_with<Tier>(
    block: &Block<Tier>,
    tier_printer: TierPrinter<Tier>,
    level: usize,
    index: usize,
) -> String {
    block
        .iter()
        .enumerate()
        .map(|(offset, instr)| {
            string_of_instr_with(instr, tier_printer, false, level, index + offset + 1)
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn string_of_elseblock_opt_with<Tier>(
    block: &Option<Block<Tier>>,
    tier_printer: TierPrinter<Tier>,
    level: usize,
    index: usize,
) -> String {
    block.as_ref().map_or_else(String::new, |block| {
        format!(
            "\n\n{}{next}. Otherwise,\n\n{}",
            "  ".repeat(level),
            string_of_block_with(block, tier_printer, level + 1, 0),
            next = index + 1
        )
    })
}

pub fn string_of_iterinstr(iterinstr: &IterInstr) -> &'static str {
    string_of_iter(iterinstr.iter)
}

pub fn string_of_iterinstrs(iterinstrs: &[IterInstr]) -> String {
    join(iterinstrs, "", |iterinstr| {
        string_of_iterinstr(iterinstr).to_owned()
    })
}

// Relations

pub fn string_of_relinput(signature: &RelSignature, exps_input: &[Exp]) -> String {
    let nottyp = &signature.notation;
    let inputs = signature.input_hint.indices();
    assert_eq!(inputs.len(), exps_input.len());
    let args = (0..nottyp.node.arity()).map(|index| {
        inputs
            .iter()
            .position(|input| *input == index as i64)
            .map_or_else(
                || "%".into(),
                |position| string_of_exp(&exps_input[position]),
            )
    });
    mixop::assemble(&nottyp.node.to_mixop(), args, string_of_atom)
        .expect("relation input arity matches notation")
}

pub fn string_of_reloutput(signature: &RelSignature, exps_output: &[Exp]) -> String {
    let nottyp = &signature.notation;
    let inputs = signature.input_hint.indices();
    let outputs = (0..nottyp.node.arity())
        .filter(|index| !inputs.contains(&(*index as i64)))
        .collect::<Vec<_>>();
    assert_eq!(outputs.len(), exps_output.len());
    let args = (0..nottyp.node.arity()).map(|index| {
        outputs
            .iter()
            .position(|output| *output == index)
            .map_or_else(
                || "%".into(),
                |position| string_of_exp(&exps_output[position]),
            )
    });
    mixop::assemble(&nottyp.node.to_mixop(), args, string_of_atom)
        .expect("relation output arity matches notation")
}

pub fn string_of_extern_rel(relation: &ExternRel) -> String {
    format!(
        "{}: {}",
        string_of_relid(&relation.id),
        string_of_relinput(&relation.signature, &relation.inputs)
    )
}

// Group-body tier

fn string_of_instr_group_tier_with(
    tier: &InstrGroup,
    short: bool,
    level: usize,
    index: usize,
) -> String {
    let order = format!("{}{index}. ", "  ".repeat(level));
    match tier {
        InstrGroup::ResultI { outputs, .. } if outputs.is_empty() => {
            render_leaf(short, &order, "The relation holds".into())
        }
        InstrGroup::ResultI { signature, outputs } => render_leaf(
            short,
            &order,
            format!("Result in: {}", string_of_reloutput(signature, outputs)),
        ),
        InstrGroup::ReturnI(exp) => {
            render_leaf(short, &order, format!("Return {}", string_of_exp(exp)))
        }
        InstrGroup::RuleI {
            rule_id,
            notation,
            iterations,
            ..
        } => render_leaf(
            short,
            &order,
            format!(
                "({}: {}){}",
                string_of_relid(rule_id),
                string_of_notexp(notation),
                string_of_iterinstrs(iterations)
            ),
        ),
        InstrGroup::BacktrackI(arms) => {
            let summary = format!("Block ({} arms)", arms.len());
            if short {
                summary
            } else {
                let indent = "  ".repeat(level);
                let arms = arms
                    .iter()
                    .enumerate()
                    .map(|(index, arm)| {
                        format!(
                            "{indent}Arm {}:\n\n{}",
                            index + 1,
                            string_of_block_group_with(arm, level + 1, 0)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n\n");
                format!("{order}{summary}\n\n{arms}")
            }
        }
    }
}

pub fn string_of_instr_group(instr: &Instr<InstrGroup>) -> String {
    string_of_instr_with(instr, string_of_instr_group_tier_with, false, 0, 0)
}

pub fn string_of_block_group(block: &BlockGroup) -> String {
    string_of_block_group_with(block, 0, 0)
}

pub fn string_of_block_group_with(block: &BlockGroup, level: usize, index: usize) -> String {
    string_of_block_with(block, string_of_instr_group_tier_with, level, index)
}

// Dispatch tier

fn string_of_instr_dispatch_tier_with(
    tier: &InstrDispatch,
    short: bool,
    level: usize,
    index: usize,
) -> String {
    let order = format!("{}{index}. ", "  ".repeat(level));
    match tier {
        InstrDispatch::GroupI {
            group_id,
            signature,
            inputs,
            block,
            ..
        } => {
            let summary = format!(
                "Group {}: {}",
                string_of_relid(group_id),
                string_of_relinput(signature, inputs)
            );
            if short {
                summary
            } else {
                format!(
                    "{order}{summary}\n\n{}",
                    string_of_block_group_with(block, level + 1, 0)
                )
            }
        }
        InstrDispatch::RouteI(arms) => {
            let summary = format!("Block ({} arms)", arms.len());
            if short {
                summary
            } else {
                let indent = "  ".repeat(level);
                let arms = arms
                    .iter()
                    .enumerate()
                    .map(|(index, arm)| {
                        format!(
                            "{indent}Arm {}:\n\n{}",
                            index + 1,
                            string_of_block_dispatch_with(arm, level + 1, 0)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n\n");
                format!("{order}{summary}\n\n{arms}")
            }
        }
    }
}

pub fn string_of_instr_dispatch(instr: &Instr<InstrDispatch>) -> String {
    string_of_instr_with(instr, string_of_instr_dispatch_tier_with, false, 0, 0)
}

pub fn string_of_block_dispatch(block: &BlockDispatch) -> String {
    string_of_block_dispatch_with(block, 0, 0)
}

pub fn string_of_block_dispatch_with(block: &BlockDispatch, level: usize, index: usize) -> String {
    string_of_block_with(block, string_of_instr_dispatch_tier_with, level, index)
}

pub fn string_of_defined_rel(relation: &Rel) -> String {
    format!(
        "{}: {}\n\n{}{}",
        string_of_relid(&relation.id),
        string_of_relinput(&relation.signature, &relation.inputs),
        string_of_block_dispatch(&relation.block),
        string_of_elseblock_opt_with(
            &relation.else_block,
            string_of_instr_dispatch_tier_with,
            0,
            relation.block.len()
        )
    )
}

// Functions

pub fn string_of_extern_func(function: &ExternFunc) -> String {
    format!(
        "{}{}{}",
        string_of_defid(&function.id),
        string_of_tparams(&function.tparams),
        string_of_params(&function.params)
    )
}

pub fn string_of_builtin_func(function: &BuiltinFunc) -> String {
    format!(
        "{}{}{}",
        string_of_defid(&function.id),
        string_of_tparams(&function.tparams),
        string_of_params(&function.params)
    )
}

pub fn string_of_tablerow(row: &TableRow) -> String {
    format!(
        "\n  Row : {} -> {}:\n\n{}",
        string_of_exps(", ", &row.inputs),
        string_of_exp(&row.expression),
        string_of_block_group_with(&row.block, 2, 0)
    )
}

pub fn string_of_tablerows(rows: &[TableRow]) -> String {
    join(rows, "\n", string_of_tablerow)
}

pub fn string_of_table_func(function: &TableFunc) -> String {
    format!(
        "{}{}\n=\n{}",
        string_of_defid(&function.id),
        string_of_params(&function.params),
        string_of_tablerows(&function.rows)
    )
}

pub fn string_of_defined_func(function: &DefinedFunc) -> String {
    format!(
        "{}{}{}\n\n{}{}",
        string_of_defid(&function.id),
        string_of_tparams(&function.tparams),
        string_of_params(&function.params),
        string_of_block_group(&function.block),
        string_of_elseblock_opt_with(
            &function.else_block,
            string_of_instr_group_tier_with,
            0,
            function.block.len()
        )
    )
}

// Definitions

pub fn string_of_def(definition: &Def) -> String {
    let mut output = String::new();
    write_def(&mut output, definition).expect("writing to a String cannot fail");
    output
}

fn write_def(output: &mut dyn fmt::Write, definition: &Def) -> fmt::Result {
    match &definition.node.kind {
        DefKind::ExternTypD(id) => write!(output, "extern syntax {}", string_of_typid(id)),
        DefKind::TypD(id, tparams, deftyp) => write!(
            output,
            "syntax {}{} = {}",
            string_of_typid(id),
            string_of_tparams(tparams),
            string_of_deftyp(deftyp)
        ),
        DefKind::VarD(id, typ) => write!(
            output,
            "var {} : {}",
            string_of_varid(id),
            string_of_typ(typ)
        ),
        DefKind::ExternRelD(relation) => {
            write!(output, "extern relation {}", string_of_extern_rel(relation))
        }
        DefKind::RelD(relation) => write!(output, "relation {}", string_of_defined_rel(relation)),
        DefKind::ExternDecD(function) => {
            write!(output, "extern def {}", string_of_extern_func(function))
        }
        DefKind::BuiltinDecD(function) => {
            write!(output, "builtin def {}", string_of_builtin_func(function))
        }
        DefKind::TableDecD(function) => {
            write!(output, "tbl def {}", string_of_table_func(function))
        }
        DefKind::FuncDecD(function) => write!(output, "def {}", string_of_defined_func(function)),
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
