//! Text rendering for structured-language data

use std::fmt;

use crate::lang::{
    el::print::{string_of_binop, string_of_cmpop, string_of_unop},
    il::print::{
        string_of_atom, string_of_def_typ, string_of_defid, string_of_iterexps,
        string_of_iterprems as string_of_iterinstrs, string_of_notexp, string_of_pattern,
        string_of_relid, string_of_rulegroupid, string_of_targs, string_of_tparams, string_of_typ,
        string_of_typid, string_of_varid,
    },
    sl::ast::*,
    xl::num::string_of_num,
};

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

// - Identifiers

/// Renders relpathid
pub fn string_of_relpathid(id: &Id) -> String {
    string_of_rulegroupid(id)
}

fn string_of_iterated(item: &Exp, iter_exps: &[IterExp]) -> String {
    if iter_exps.is_empty() {
        string_of_exp(item)
    } else {
        format!("({}){}", string_of_exp(item), string_of_iterexps(iter_exps))
    }
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
                if index != 0 {
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
        ExpKind::Slice(exp_b, exp_l, exp_r) => {
            write_exp(output, exp_b)?;
            output.write_char('[')?;
            write_exp(output, exp_l)?;
            output.write_str(" : ")?;
            write_exp(output, exp_r)?;
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
            output.write_str(&string_of_iterated(exp, std::slice::from_ref(iter_exp)))
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

/// Renders case with
pub fn string_of_case_with(case: &Case, level: usize, index: usize) -> String {
    let mut output = String::new();
    write_case_with(&mut output, case, level, index).expect("writing to a String cannot fail");
    output
}

fn write_case_with(
    output: &mut dyn fmt::Write,
    case: &Case,
    level: usize,
    index: usize,
) -> fmt::Result {
    write!(output, "{}{index}. Case ", "  ".repeat(level))?;
    write_guard(output, &case.guard)?;
    output.write_str("\n\n")?;
    write_block_with(output, &case.block, level + 1, 0)
}

/// Renders cases with
pub fn string_of_cases_with(cases: &[Case], level: usize) -> String {
    let mut output = String::new();
    write_cases_with(&mut output, cases, level).expect("writing to a String cannot fail");
    output
}

fn write_cases_with(output: &mut dyn fmt::Write, cases: &[Case], level: usize) -> fmt::Result {
    for (index, case) in cases.iter().enumerate() {
        if index != 0 {
            output.write_str("\n\n")?;
        }
        write_case_with(output, case, level, index + 1)?;
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
    }
}

// - Instructions

/// Renders instr
pub fn string_of_instr(instr: &Instr) -> String {
    let mut output = String::new();
    write_instr(&mut output, instr).expect("writing to a String cannot fail");
    output
}

fn write_instr(output: &mut dyn fmt::Write, instr: &Instr) -> fmt::Result {
    write_instr_with(output, instr, false, 0, 0)
}

/// Renders instr with
pub fn string_of_instr_with(instr: &Instr, short: bool, level: usize, index: usize) -> String {
    let mut output = String::new();
    write_instr_with(&mut output, instr, short, level, index)
        .expect("writing to a String cannot fail");
    output
}

fn write_instr_with(
    output: &mut dyn fmt::Write,
    instr: &Instr,
    short: bool,
    level: usize,
    index: usize,
) -> fmt::Result {
    let order = format!("{}{index}. ", "  ".repeat(level));
    let mut write_order = || {
        if short {
            Ok(())
        } else {
            output.write_str(&order)
        }
    };
    match &instr.node.kind {
        InstrKind::If(IfInstr {
            exp,
            iter_exps,
            block,
            dangle,
        }) => {
            write_order()?;
            output.write_str("If (")?;
            write_exp(output, exp)?;
            write!(output, "){}, then", string_of_iterexps(iter_exps))?;
            if !short {
                output.write_str("\n\n")?;
                write_block_with(output, block, level + 1, 0)?;
                if *dangle {
                    write!(
                        output,
                        "\n\n{order}Else {}",
                        string_of_dangle(instr.node.note)
                    )?;
                }
            }
            Ok(())
        }
        InstrKind::Hold(HoldInstr {
            id,
            not_exp,
            iter_exps,
            hold_case,
        }) => match hold_case {
            HoldCase::Both(block_hold, block_not_hold) => {
                write_order()?;
                write!(
                    output,
                    "If ({}: {}){} holds, then",
                    string_of_relid(id),
                    string_of_notexp(not_exp),
                    string_of_iterexps(iter_exps)
                )?;
                if !short {
                    output.write_str("\n\n")?;
                    write_block_with(output, block_hold, level + 1, 0)?;
                    write!(output, "\n\n{order}Else,\n\n")?;
                    write_block_with(output, block_not_hold, level + 1, 0)?;
                }
                Ok(())
            }
            HoldCase::Hold(block, dangle) | HoldCase::NotHold(block, dangle) => {
                write_order()?;
                write!(
                    output,
                    "If ({}: {}){} {}, then",
                    string_of_relid(id),
                    string_of_notexp(not_exp),
                    string_of_iterexps(iter_exps),
                    if matches!(hold_case, HoldCase::NotHold(..)) {
                        "does not hold"
                    } else {
                        "holds"
                    }
                )?;
                if !short {
                    output.write_str("\n\n")?;
                    write_block_with(output, block, level + 1, 0)?;
                    if *dangle {
                        write!(
                            output,
                            "\n\n{order}Else {}",
                            string_of_dangle(instr.node.note)
                        )?;
                    }
                }
                Ok(())
            }
        },
        InstrKind::Case(CaseInstr { exp, cases, dangle }) => {
            write_order()?;
            output.write_str("Case analysis on ")?;
            write_exp(output, exp)?;
            if !short {
                output.write_str("\n\n")?;
                write_cases_with(output, cases, level + 1)?;
                if *dangle {
                    write!(
                        output,
                        "\n\n{order}Else {}",
                        string_of_dangle(instr.node.note)
                    )?;
                }
            }
            Ok(())
        }
        InstrKind::Group(GroupInstr {
            id,
            rel_signature,
            exps,
            block,
        }) => {
            write_order()?;
            write!(
                output,
                "Group {}: {}",
                string_of_relid(id),
                string_of_relinput(rel_signature, exps)
            )?;
            if !short {
                output.write_str("\n\n")?;
                write_block_with(output, block, level + 1, 0)?;
            }
            Ok(())
        }
        InstrKind::Let(LetInstr {
            exp_l,
            exp_r,
            iter_instrs,
            block,
        }) => {
            write_order()?;
            output.write_str("(Let ")?;
            write_exp(output, exp_l)?;
            output.write_str(" be ")?;
            write_exp(output, exp_r)?;
            write!(output, "){}", string_of_iterinstrs(iter_instrs))?;
            if !short {
                output.write_str("\n\n")?;
                write_block_with(output, block, level + 1, 0)?;
            }
            Ok(())
        }
        InstrKind::Rule(RuleInstr {
            id,
            not_exp,
            iter_instrs,
            block,
            ..
        }) => {
            write_order()?;
            write!(
                output,
                "({}: {}){}",
                string_of_relid(id),
                string_of_notexp(not_exp),
                string_of_iterinstrs(iter_instrs)
            )?;
            if !short {
                output.write_str("\n\n")?;
                write_block_with(output, block, level + 1, 0)?;
            }
            Ok(())
        }
        InstrKind::Result(ResultInstr { exps, .. }) if exps.is_empty() => {
            write_order()?;
            output.write_str("The relation holds")
        }
        InstrKind::Result(ResultInstr {
            rel_signature,
            exps,
        }) => {
            write_order()?;
            write!(
                output,
                "Result in: {}",
                string_of_reloutput(rel_signature, exps)
            )
        }
        InstrKind::Return(ReturnInstr { exp }) => {
            write_order()?;
            output.write_str("Return ")?;
            write_exp(output, exp)
        }
        InstrKind::Debug(DebugInstr { exp, instr: nested }) => {
            write_order()?;
            output.write_str("Debug: ")?;
            write_exp(output, exp)?;
            if !short {
                output.write_str("\n\n")?;
                write_instr_with(output, nested, false, level, index + 1)?;
            }
            Ok(())
        }
    }
}

/// Renders block
pub fn string_of_block(block: &Block) -> String {
    let mut output = String::new();
    write_block(&mut output, block).expect("writing to a String cannot fail");
    output
}

fn write_block(output: &mut dyn fmt::Write, block: &Block) -> fmt::Result {
    write_block_with(output, block, 0, 0)
}

/// Renders block with
pub fn string_of_block_with(block: &Block, level: usize, index: usize) -> String {
    let mut output = String::new();
    write_block_with(&mut output, block, level, index).expect("writing to a String cannot fail");
    output
}

fn write_block_with(
    output: &mut dyn fmt::Write,
    block: &Block,
    level: usize,
    index: usize,
) -> fmt::Result {
    for (offset, instr) in block.iter().enumerate() {
        if offset != 0 {
            output.write_str("\n\n")?;
        }
        write_instr_with(output, instr, false, level, index + offset + 1)?;
    }
    Ok(())
}

/// Renders elseblock with
pub fn string_of_elseblock_with(block: &ElseBlock, level: usize, index: usize) -> String {
    let mut output = String::new();
    write_elseblock_with(&mut output, block, level, index)
        .expect("writing to a String cannot fail");
    output
}

fn write_elseblock_with(
    output: &mut dyn fmt::Write,
    block: &ElseBlock,
    level: usize,
    index: usize,
) -> fmt::Result {
    write!(
        output,
        "{}{next}. Otherwise,\n\n",
        "  ".repeat(level),
        next = index + 1
    )?;
    write_block_with(output, block, level + 1, 0)
}

/// Renders elseblock opt with
pub fn string_of_elseblock_opt_with(
    block: &Option<ElseBlock>,
    level: usize,
    index: usize,
) -> String {
    let mut output = String::new();
    write_elseblock_opt_with(&mut output, block, level, index)
        .expect("writing to a String cannot fail");
    output
}

fn write_elseblock_opt_with(
    output: &mut dyn fmt::Write,
    block: &Option<ElseBlock>,
    level: usize,
    index: usize,
) -> fmt::Result {
    if let Some(block) = block {
        output.write_str("\n\n")?;
        write_elseblock_with(output, block, level, index)?;
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

/// Renders defined rel
pub fn string_of_defined_rel(relation: &Rel) -> String {
    let mut output = String::new();
    write_defined_rel(&mut output, relation).expect("writing to a String cannot fail");
    output
}

fn write_defined_rel(output: &mut dyn fmt::Write, relation: &Rel) -> fmt::Result {
    write!(
        output,
        "{}: {}\n\n",
        string_of_relid(&relation.id),
        string_of_relinput(&relation.rel_signature, &relation.exps_input)
    )?;
    write_block_with(output, &relation.block, 0, 0)?;
    write_elseblock_opt_with(output, &relation.else_block, 0, relation.block.len())
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
        "\n  Row : {} -> ",
        string_of_exps(", ", &table_row.exps_input)
    )?;
    write_exp(output, &table_row.exp)?;
    output.write_str(":\n\n")?;
    write_block_with(output, &table_row.block, 2, 0)
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
    for (index, table_row) in function.table_rows.iter().enumerate() {
        if index != 0 {
            output.write_char('\n')?;
        }
        write_tablerow(output, table_row)?;
    }
    Ok(())
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
        "{}{}{}\n\n",
        string_of_defid(&function.id),
        string_of_tparams(&function.tparams),
        string_of_params(&function.params)
    )?;
    write_block_with(output, &function.block, 0, 0)?;
    write_elseblock_opt_with(output, &function.else_block, 0, function.block.len())
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
