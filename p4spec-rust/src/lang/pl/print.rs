//! Text rendering for prose-language data

use std::fmt;

use crate::lang::{
    el::print::{string_of_binop, string_of_cmpop, string_of_unop},
    il::print::{
        string_of_atom, string_of_def_typ, string_of_defid, string_of_iter, string_of_iterexps,
        string_of_pattern, string_of_relid, string_of_targs, string_of_tparams, string_of_typ,
        string_of_typid, string_of_varid,
    },
    xl::num::string_of_num,
};

use super::ast::*;

fn join<T>(items: &[T], sep: &str, render: impl Fn(&T) -> String) -> String {
    items.iter().map(render).collect::<Vec<_>>().join(sep)
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

// - Values

/// Renders vid
pub fn string_of_vid(vid: i64) -> String {
    format!("@{vid}")
}

// - Expressions

/// Renders exp
pub fn string_of_exp(exp: &Exp) -> String {
    let mut output = String::new();
    write_exp(&mut output, exp).expect("writing to a String cannot fail");
    output
}
fn write_exp(output: &mut dyn fmt::Write, exp: &Exp) -> fmt::Result {
    match &exp.node.node.kind {
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
            output.write_char('(')?;
            write_exp(output, exp)?;
            write!(output, " as {})", string_of_typ(typ))
        }
        ExpKind::Sub(exp, typ, _) => {
            output.write_char('(')?;
            write_exp(output, exp)?;
            write!(output, " has type {})", string_of_typ(typ))
        }
        ExpKind::Match(exp, pattern) => {
            output.write_char('(')?;
            write_exp(output, exp)?;
            write!(output, " matches pattern {})", string_of_pattern(pattern))
        }
        ExpKind::Tuple(exps) => {
            output.write_char('(')?;
            write_exps(output, ", ", exps)?;
            output.write_char(')')
        }
        ExpKind::Case(not_exp) => write!(output, "({})", string_of_notexp(not_exp)),
        ExpKind::Str(fields) => {
            output.write_char('{')?;
            for (index, (atom, exp)) in fields.iter().enumerate() {
                if index > 0 {
                    output.write_str(", ")?;
                }
                write!(output, "{} ", string_of_atom(atom))?;
                write_exp(output, exp)?;
            }
            output.write_char('}')
        }
        ExpKind::Opt(Some(exp)) => {
            output.write_str("?(")?;
            write_exp(output, exp)?;
            output.write_char(')')
        }
        ExpKind::Opt(None) => output.write_str("?()"),
        ExpKind::List(exps) => {
            output.write_char('[')?;
            write_exps(output, ", ", exps)?;
            output.write_char(']')
        }
        ExpKind::Cons(exp_head, exp_tail) => {
            write_exp(output, exp_head)?;
            output.write_str(" :: ")?;
            write_exp(output, exp_tail)
        }
        ExpKind::Cat(exp_l, exp_r) => {
            write_exp(output, exp_l)?;
            output.write_str(" ++ ")?;
            write_exp(output, exp_r)
        }
        ExpKind::Mem(exp_e, exp_s) => {
            write_exp(output, exp_e)?;
            output.write_str(" is in ")?;
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
            write!(output, "[{} = ", string_of_path(path))?;
            write_exp(output, exp_f)?;
            output.write_char(']')
        }
        ExpKind::Call(id, targs, args) => write!(
            output,
            "{}{}{}",
            string_of_defid(id),
            string_of_targs(targs),
            string_of_args(args)
        ),
        ExpKind::Iter(exp, iter_exp) => {
            output.write_char('(')?;
            write_exp(output, exp)?;
            write!(
                output,
                "){}",
                string_of_iterexps(std::slice::from_ref(iter_exp))
            )
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
        if index > 0 {
            output.write_str(sep)?;
        }
        write_exp(output, exp)?;
    }
    Ok(())
}

/// Renders not_exp
pub fn string_of_notexp(not_exp: &NotExp) -> String {
    let mut output = String::new();
    write_notexp(&mut output, not_exp).expect("writing to a String cannot fail");
    output
}

fn write_notexp(output: &mut dyn fmt::Write, not_exp: &NotExp) -> fmt::Result {
    output.write_str(&not_exp.render(string_of_atom, string_of_exp))
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
        ParamKind::Exp(_, exp) => write_exp(output, exp),
        ParamKind::Def(id, tparams, params, typ) => write!(
            output,
            "{}{}{} : {}",
            string_of_defid(id),
            string_of_tparams(tparams),
            string_of_params(params),
            string_of_typ(typ)
        ),
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

// - Danglings

/// Renders dangle
pub fn string_of_dangle(iid: Iid) -> String {
    format!("Dangling#{iid}")
}

// - Case analysis

type TierPrinter<Tier> = fn(&Tier, bool, usize, usize) -> String;

fn write_case_with<Tier>(
    output: &mut dyn fmt::Write,
    case: &Case<Tier>,
    tier_printer: TierPrinter<Tier>,
    level: usize,
    index: usize,
) -> fmt::Result {
    write!(output, "{}{index}. Case ", "  ".repeat(level))?;
    write_guard(output, &case.guard)?;
    output.write_str("\n\n")?;
    write_block_with(output, &case.block, tier_printer, level + 1, 0)
}

fn string_of_cases_with<Tier>(
    cases: &[Case<Tier>],
    tier_printer: TierPrinter<Tier>,
    level: usize,
) -> String {
    let mut output = String::new();
    write_cases_with(&mut output, cases, tier_printer, level)
        .expect("writing to a String cannot fail");
    output
}

fn write_cases_with<Tier>(
    output: &mut dyn fmt::Write,
    cases: &[Case<Tier>],
    tier_printer: TierPrinter<Tier>,
    level: usize,
) -> fmt::Result {
    for (index, case) in cases.iter().enumerate() {
        if index != 0 {
            output.write_str("\n\n")?;
        }
        write_case_with(output, case, tier_printer, level, index + 1)?;
    }
    Ok(())
}

/// Renders guard
pub fn string_of_guard(guard: &Guard) -> String {
    let mut output = String::new();
    write_guard(&mut output, guard).expect("writing to a String cannot fail");
    output
}

fn write_guard(output: &mut dyn fmt::Write, guard: &Guard) -> fmt::Result {
    match guard {
        Guard::Bool(value) => write!(output, "{value}"),
        Guard::Cmp(op, _, exp) => {
            write!(output, "(% {} ", string_of_cmpop(*op))?;
            write_exp(output, exp)?;
            output.write_char(')')
        }
        Guard::Sub(typ, _) => write!(output, "(% has type {})", string_of_typ(typ)),
        Guard::Match(pattern) => {
            write!(output, "(% matches pattern {})", string_of_pattern(pattern))
        }
        Guard::Mem(exp) => {
            output.write_str("(% is in ")?;
            write_exp(output, exp)?;
            output.write_char(')')
        }
        Guard::CheckLetSub(typ, _, exp) => {
            output.write_str("(let ")?;
            write_exp(output, exp)?;
            write!(output, " be %, % has type {})", string_of_typ(typ))
        }
        Guard::CheckLetMatch(pattern, exp) => {
            output.write_str("(let ")?;
            write_exp(output, exp)?;
            write!(
                output,
                " be %, % matches pattern {})",
                string_of_pattern(pattern)
            )
        }
    }
}

// - Instructions
// Shared control flow parameterized by the tier

fn string_of_instr_with<Tier>(
    instr: &Instr<Tier>,
    tier_printer: TierPrinter<Tier>,
    short: bool,
    level: usize,
    index: usize,
) -> String {
    let order = format!("{}{index}. ", "  ".repeat(level));
    match &instr.node.node.kind {
        InstrKind::If(IfInstr {
            exp,
            iter_exps,
            block,
            dangle,
        }) => {
            let summary = format!(
                "If ({}){}, then",
                string_of_exp(exp),
                string_of_iterexps(iter_exps)
            );
            if short {
                summary
            } else {
                format!(
                    "{order}{summary}\n\n{}{}",
                    string_of_block_with(block, tier_printer, level + 1, 0),
                    if *dangle {
                        format!(
                            "\n\n{order}Else {}",
                            string_of_dangle(instr.node.node.note.iid)
                        )
                    } else {
                        String::new()
                    }
                )
            }
        }
        InstrKind::Hold(HoldInstr {
            id,
            not_exp,
            iter_exps,
            hold_case,
        }) => {
            let holding = |negative: bool| {
                format!(
                    "If ({}: {}){} {}, then",
                    string_of_relid(id),
                    string_of_notexp(not_exp),
                    string_of_iterexps(iter_exps),
                    if negative { "does not hold" } else { "holds" }
                )
            };
            match hold_case {
                HoldCase::Both(block_hold, block_not_hold) => {
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
                HoldCase::Hold(block, dangle) | HoldCase::NotHold(block, dangle) => {
                    let summary = holding(matches!(hold_case, HoldCase::NotHold(..)));
                    if short {
                        summary
                    } else {
                        format!(
                            "{order}{summary}\n\n{}{}",
                            string_of_block_with(block, tier_printer, level + 1, 0),
                            if *dangle {
                                format!(
                                    "\n\n{order}Else {}",
                                    string_of_dangle(instr.node.node.note.iid)
                                )
                            } else {
                                String::new()
                            }
                        )
                    }
                }
            }
        }
        InstrKind::Case(CaseInstr { exp, cases, dangle }) => {
            let summary = format!("Case analysis on {}", string_of_exp(exp));
            if short {
                summary
            } else {
                format!(
                    "{order}{summary}\n\n{}{}",
                    string_of_cases_with(cases, tier_printer, level + 1),
                    if *dangle {
                        format!(
                            "\n\n{order}Else {}",
                            string_of_dangle(instr.node.node.note.iid)
                        )
                    } else {
                        String::new()
                    }
                )
            }
        }
        InstrKind::Let(LetInstr {
            exp_l,
            exp_r,
            iter_instrs,
        }) => render_leaf(
            short,
            &order,
            format!(
                "(Let {} be {}){}",
                string_of_exp(exp_l),
                string_of_exp(exp_r),
                string_of_iterinstrs(iter_instrs)
            ),
        ),
        InstrKind::Debug(DebugInstr { exp }) => {
            render_leaf(short, &order, format!("Debug: {}", string_of_exp(exp)))
        }
        InstrKind::Destruct(DestructInstr {
            bindings: fields,
            exp: exp_r,
        }) => render_leaf(
            short,
            &order,
            format!(
                "(Destruct ({}) = {})",
                join(fields, ", ", |(_, exp)| string_of_exp(exp)),
                string_of_exp(exp_r)
            ),
        ),
        InstrKind::CheckLetSub(CheckLetSubInstr {
            typ,
            exp_l,
            exp_r,
            block,
            ..
        }) => render_nested(
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
        InstrKind::CheckLetMatch(CheckLetMatchInstr {
            pattern,
            exp_l,
            exp_r,
            block,
        }) => render_nested(
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
        InstrKind::OptionGet(OptionGetInstr {
            exp_l,
            exp_r,
            block,
        }) => render_nested(
            short,
            &order,
            format!(
                "(Let {} be ! {})",
                string_of_exp(exp_l),
                string_of_exp(exp_r)
            ),
            string_of_block_with(block, tier_printer, level + 1, 0),
        ),
        InstrKind::Tier(TierInstr { tier }) => tier_printer(tier, short, level, index),
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

fn write_instr_with<Tier>(
    output: &mut dyn fmt::Write,
    instr: &Instr<Tier>,
    tier_printer: TierPrinter<Tier>,
    short: bool,
    level: usize,
    index: usize,
) -> fmt::Result {
    output.write_str(&string_of_instr_with(
        instr,
        tier_printer,
        short,
        level,
        index,
    ))
}

fn string_of_block_with<Tier>(
    block: &Block<Tier>,
    tier_printer: TierPrinter<Tier>,
    level: usize,
    index: usize,
) -> String {
    let mut output = String::new();
    write_block_with(&mut output, block, tier_printer, level, index)
        .expect("writing to a String cannot fail");
    output
}

fn write_block_with<Tier>(
    output: &mut dyn fmt::Write,
    block: &Block<Tier>,
    tier_printer: TierPrinter<Tier>,
    level: usize,
    index: usize,
) -> fmt::Result {
    for (offset, instr) in block.iter().enumerate() {
        if offset != 0 {
            output.write_str("\n\n")?;
        }
        write_instr_with(
            output,
            instr,
            tier_printer,
            false,
            level,
            index + offset + 1,
        )?;
    }
    Ok(())
}

fn string_of_elseblock_opt_with<Tier>(
    block: &Option<Block<Tier>>,
    tier_printer: TierPrinter<Tier>,
    level: usize,
    index: usize,
) -> String {
    let mut output = String::new();
    write_elseblock_opt_with(&mut output, block, tier_printer, level, index)
        .expect("writing to a String cannot fail");
    output
}

fn write_elseblock_opt_with<Tier>(
    output: &mut dyn fmt::Write,
    block: &Option<Block<Tier>>,
    tier_printer: TierPrinter<Tier>,
    level: usize,
    index: usize,
) -> fmt::Result {
    if let Some(block) = block {
        write!(
            output,
            "\n\n{}{next}. Otherwise,\n\n",
            "  ".repeat(level),
            next = index + 1
        )?;
        write_block_with(output, block, tier_printer, level + 1, 0)?;
    }
    Ok(())
}

/// Renders iter_instr
pub fn string_of_iterinstr(iter_instr: &IterInstr) -> &'static str {
    string_of_iter(iter_instr.iter)
}

/// Renders iter_instrs
pub fn string_of_iterinstrs(iter_instrs: &[IterInstr]) -> String {
    let mut output = String::new();
    write_iterinstrs(&mut output, iter_instrs).expect("writing to a String cannot fail");
    output
}

fn write_iterinstrs(output: &mut dyn fmt::Write, iter_instrs: &[IterInstr]) -> fmt::Result {
    for iter_instr in iter_instrs {
        output.write_str(string_of_iterinstr(iter_instr))?;
    }
    Ok(())
}

// - Relations

/// Renders relation input expressions
///
/// `exps_input` must match `rel_signature.input_hint.indices()`;
/// notation arity must be internally valid
pub fn string_of_relinput(rel_signature: &RelSignature, exps_input: &[Exp]) -> String {
    let mut output = String::new();
    write_relinput(&mut output, rel_signature, exps_input)
        .expect("writing to a String cannot fail");
    output
}

fn write_relinput(
    output: &mut dyn fmt::Write,
    rel_signature: &RelSignature,
    exps_input: &[Exp],
) -> fmt::Result {
    let not_typ = &rel_signature.not_typ;
    let input_indices = rel_signature.input_hint.indices();
    assert_eq!(input_indices.len(), exps_input.len());
    let args = (0..not_typ.node.arity()).map(|index| {
        input_indices
            .iter()
            .position(|input| *input == index as i64)
            .map_or_else(
                || "%".into(),
                |position| string_of_exp(&exps_input[position]),
            )
    });
    let rendered = not_typ
        .node
        .to_mixop()
        .to_string(args, string_of_atom)
        .expect("relation input arity matches notation");
    output.write_str(&rendered)
}

/// Renders relation output expressions
///
/// `exps_output` must cover every non-input notation position;
/// notation arity must be internally valid
pub fn string_of_reloutput(rel_signature: &RelSignature, exps_output: &[Exp]) -> String {
    let mut output = String::new();
    write_reloutput(&mut output, rel_signature, exps_output)
        .expect("writing to a String cannot fail");
    output
}

fn write_reloutput(
    output: &mut dyn fmt::Write,
    rel_signature: &RelSignature,
    exps_output: &[Exp],
) -> fmt::Result {
    let not_typ = &rel_signature.not_typ;
    let input_indices = rel_signature.input_hint.indices();
    let outputs = (0..not_typ.node.arity())
        .filter(|index| !input_indices.contains(&(*index as i64)))
        .collect::<Vec<_>>();
    assert_eq!(outputs.len(), exps_output.len());
    let args = (0..not_typ.node.arity()).map(|index| {
        outputs
            .iter()
            .position(|output| *output == index)
            .map_or_else(
                || "%".into(),
                |position| string_of_exp(&exps_output[position]),
            )
    });
    let rendered = not_typ
        .node
        .to_mixop()
        .to_string(args, string_of_atom)
        .expect("relation output arity matches notation");
    output.write_str(&rendered)
}

/// Renders extern rel
pub fn string_of_extern_rel(relation: &ExternRel) -> String {
    let mut output = String::new();
    write_extern_rel(&mut output, relation).expect("writing to a String cannot fail");
    output
}

fn write_extern_rel(output: &mut dyn fmt::Write, relation: &ExternRel) -> fmt::Result {
    write!(
        output,
        "{}: {}",
        string_of_relid(&relation.id),
        string_of_relinput(&relation.rel_signature, &relation.exps_input)
    )
}

// - Group-body tier

fn string_of_instr_group_tier_with(
    tier: &InstrGroup,
    short: bool,
    level: usize,
    index: usize,
) -> String {
    let order = format!("{}{index}. ", "  ".repeat(level));
    match tier {
        InstrGroup::Result(ResultGroupInstr { exps_output, .. }) if exps_output.is_empty() => {
            render_leaf(short, &order, "The relation holds".into())
        }
        InstrGroup::Result(ResultGroupInstr {
            rel_signature,
            exps_output,
        }) => render_leaf(
            short,
            &order,
            format!(
                "Result in: {}",
                string_of_reloutput(rel_signature, exps_output)
            ),
        ),
        InstrGroup::Return(ReturnGroupInstr { exp }) => {
            render_leaf(short, &order, format!("Return {}", string_of_exp(exp)))
        }
        InstrGroup::Rule(RuleGroupInstr {
            id,
            not_exp,
            iter_instrs,
            ..
        }) => render_leaf(
            short,
            &order,
            format!(
                "({}: {}){}",
                string_of_relid(id),
                string_of_notexp(not_exp),
                string_of_iterinstrs(iter_instrs)
            ),
        ),
        InstrGroup::Backtrack(BacktrackGroupInstr { blocks }) => {
            let summary = format!("Block ({} arms)", blocks.len());
            if short {
                summary
            } else {
                let indent = "  ".repeat(level);
                let arms = blocks
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

/// Renders instr group
pub fn string_of_instr_group(instr: &Instr<InstrGroup>) -> String {
    let mut output = String::new();
    write_instr_group(&mut output, instr).expect("writing to a String cannot fail");
    output
}

fn write_instr_group(output: &mut dyn fmt::Write, instr: &Instr<InstrGroup>) -> fmt::Result {
    write_instr_with(output, instr, string_of_instr_group_tier_with, false, 0, 0)
}

/// Renders block group
pub fn string_of_block_group(block: &BlockGroup) -> String {
    let mut output = String::new();
    write_block_group(&mut output, block).expect("writing to a String cannot fail");
    output
}

fn write_block_group(output: &mut dyn fmt::Write, block: &BlockGroup) -> fmt::Result {
    write_block_group_with(output, block, 0, 0)
}

/// Renders block group with
pub fn string_of_block_group_with(block: &BlockGroup, level: usize, index: usize) -> String {
    let mut output = String::new();
    write_block_group_with(&mut output, block, level, index)
        .expect("writing to a String cannot fail");
    output
}

fn write_block_group_with(
    output: &mut dyn fmt::Write,
    block: &BlockGroup,
    level: usize,
    index: usize,
) -> fmt::Result {
    write_block_with(output, block, string_of_instr_group_tier_with, level, index)
}

// - Dispatch tier

fn string_of_instr_dispatch_tier_with(
    tier: &InstrDispatch,
    short: bool,
    level: usize,
    index: usize,
) -> String {
    let order = format!("{}{index}. ", "  ".repeat(level));
    match tier {
        InstrDispatch::Group(GroupDispatchInstr {
            id_group,
            rel_signature,
            exps_input,
            block,
            ..
        }) => {
            let summary = format!(
                "Group {}: {}",
                string_of_relid(id_group),
                string_of_relinput(rel_signature, exps_input)
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
        InstrDispatch::Route(RouteDispatchInstr { blocks }) => {
            let summary = format!("Block ({} arms)", blocks.len());
            if short {
                summary
            } else {
                let indent = "  ".repeat(level);
                let arms = blocks
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

/// Renders instr dispatch
pub fn string_of_instr_dispatch(instr: &Instr<InstrDispatch>) -> String {
    let mut output = String::new();
    write_instr_dispatch(&mut output, instr).expect("writing to a String cannot fail");
    output
}

fn write_instr_dispatch(output: &mut dyn fmt::Write, instr: &Instr<InstrDispatch>) -> fmt::Result {
    write_instr_with(
        output,
        instr,
        string_of_instr_dispatch_tier_with,
        false,
        0,
        0,
    )
}

/// Renders block dispatch
pub fn string_of_block_dispatch(block: &BlockDispatch) -> String {
    let mut output = String::new();
    write_block_dispatch(&mut output, block).expect("writing to a String cannot fail");
    output
}

fn write_block_dispatch(output: &mut dyn fmt::Write, block: &BlockDispatch) -> fmt::Result {
    write_block_dispatch_with(output, block, 0, 0)
}

/// Renders block dispatch with
pub fn string_of_block_dispatch_with(block: &BlockDispatch, level: usize, index: usize) -> String {
    let mut output = String::new();
    write_block_dispatch_with(&mut output, block, level, index)
        .expect("writing to a String cannot fail");
    output
}

fn write_block_dispatch_with(
    output: &mut dyn fmt::Write,
    block: &BlockDispatch,
    level: usize,
    index: usize,
) -> fmt::Result {
    write_block_with(
        output,
        block,
        string_of_instr_dispatch_tier_with,
        level,
        index,
    )
}

/// Renders defined rel
pub fn string_of_defined_rel(relation: &Rel) -> String {
    let mut output = String::new();
    write_defined_rel(&mut output, relation).expect("writing to a String cannot fail");
    output
}

fn write_defined_rel(output: &mut dyn fmt::Write, relation: &Rel) -> fmt::Result {
    write!(
        output,
        "{}: {}\n\n{}{}",
        string_of_relid(&relation.id),
        string_of_relinput(&relation.rel_signature, &relation.exps_input),
        string_of_block_dispatch(&relation.block),
        string_of_elseblock_opt_with(
            &relation.block_else_opt,
            string_of_instr_dispatch_tier_with,
            0,
            relation.block.len()
        )
    )
}

// - Functions

/// Renders extern func
pub fn string_of_extern_func(function: &ExternFunc) -> String {
    let mut output = String::new();
    write_extern_func(&mut output, function).expect("writing to a String cannot fail");
    output
}

fn write_extern_func(output: &mut dyn fmt::Write, function: &ExternFunc) -> fmt::Result {
    write!(
        output,
        "{}{}{}",
        string_of_defid(&function.id),
        string_of_tparams(&function.tparams),
        string_of_params(&function.params)
    )
}

/// Renders builtin func
pub fn string_of_builtin_func(function: &BuiltinFunc) -> String {
    let mut output = String::new();
    write_builtin_func(&mut output, function).expect("writing to a String cannot fail");
    output
}

fn write_builtin_func(output: &mut dyn fmt::Write, function: &BuiltinFunc) -> fmt::Result {
    write!(
        output,
        "{}{}{}",
        string_of_defid(&function.id),
        string_of_tparams(&function.tparams),
        string_of_params(&function.params)
    )
}

/// Renders tablerow
pub fn string_of_tablerow(table_row: &TableRow) -> String {
    let mut output = String::new();
    write_tablerow(&mut output, table_row).expect("writing to a String cannot fail");
    output
}

fn write_tablerow(output: &mut dyn fmt::Write, table_row: &TableRow) -> fmt::Result {
    write!(
        output,
        "\n  Row : {} -> {}:\n\n{}",
        string_of_exps(", ", &table_row.exps_input),
        string_of_exp(&table_row.exp),
        string_of_block_group_with(&table_row.block, 2, 0)
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
        if index != 0 {
            output.write_char('\n')?;
        }
        write_tablerow(output, table_row)?;
    }
    Ok(())
}

/// Renders table func
pub fn string_of_table_func(function: &TableFunc) -> String {
    let mut output = String::new();
    write_table_func(&mut output, function).expect("writing to a String cannot fail");
    output
}

fn write_table_func(output: &mut dyn fmt::Write, function: &TableFunc) -> fmt::Result {
    write!(
        output,
        "{}{}\n=\n",
        string_of_defid(&function.id),
        string_of_params(&function.params)
    )?;
    write_tablerows(output, &function.rows)
}

/// Renders defined func
pub fn string_of_defined_func(function: &DefinedFunc) -> String {
    let mut output = String::new();
    write_defined_func(&mut output, function).expect("writing to a String cannot fail");
    output
}

fn write_defined_func(output: &mut dyn fmt::Write, function: &DefinedFunc) -> fmt::Result {
    write!(
        output,
        "{}{}{}\n\n{}{}",
        string_of_defid(&function.id),
        string_of_tparams(&function.tparams),
        string_of_params(&function.params),
        string_of_block_group(&function.block),
        string_of_elseblock_opt_with(
            &function.block_else_opt,
            string_of_instr_group_tier_with,
            0,
            function.block.len()
        )
    )
}

// - Definitions

/// Renders def
pub fn string_of_def(definition: &Def) -> String {
    let mut output = String::new();
    write_def(&mut output, definition).expect("writing to a String cannot fail");
    output
}

fn write_def(output: &mut dyn fmt::Write, definition: &Def) -> fmt::Result {
    match &definition.node.node {
        DefKind::ExternTyp(ExternTypDef { id }) => {
            write!(output, "extern syntax {}", string_of_typid(id))
        }
        DefKind::Typ(TypDef {
            id,
            tparams,
            def_typ,
        }) => write!(
            output,
            "syntax {}{} = {}",
            string_of_typid(id),
            string_of_tparams(tparams),
            string_of_def_typ(def_typ)
        ),
        DefKind::Var(VarDef { id, typ }) => write!(
            output,
            "var {} : {}",
            string_of_varid(id),
            string_of_typ(typ)
        ),
        DefKind::ExternRel(relation) => {
            output.write_str("extern relation ")?;
            write_extern_rel(output, relation)
        }
        DefKind::Rel(relation) => {
            output.write_str("relation ")?;
            write_defined_rel(output, relation)
        }
        DefKind::ExternDec(function) => {
            output.write_str("extern def ")?;
            write_extern_func(output, function)
        }
        DefKind::BuiltinDec(function) => {
            output.write_str("builtin def ")?;
            write_builtin_func(output, function)
        }
        DefKind::TableDec(function) => {
            output.write_str("tbl def ")?;
            write_table_func(output, function)
        }
        DefKind::FuncDec(function) => {
            output.write_str("def ")?;
            write_defined_func(output, function)
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
