use std::fmt;

use crate::{
    domain::mixop,
    lang::{il, sl::ast::*},
};

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
    il::print::string_of_num(number)
}

// Texts

pub fn string_of_text(text: &str) -> String {
    il::print::string_of_text(text)
}

// Identifiers

pub fn string_of_varid(id: &Id) -> String {
    il::print::string_of_varid(id)
}

pub fn string_of_typid(id: &Id) -> String {
    il::print::string_of_typid(id)
}

pub fn string_of_relid(id: &Id) -> String {
    il::print::string_of_relid(id)
}

pub fn string_of_relpathid(id: &Id) -> String {
    il::print::string_of_rulegroupid(id)
}

pub fn string_of_defid(id: &Id) -> String {
    il::print::string_of_defid(id)
}

// Atoms

pub fn string_of_atom(atom: &Atom) -> String {
    il::print::string_of_atom(atom)
}

pub fn string_of_atoms(atoms: &[Atom]) -> String {
    join(atoms, "", string_of_atom)
}

// Mixfix operators

pub fn string_of_mixop(operator: &Mixop) -> String {
    il::print::string_of_mixop(operator)
}

// Iterators

pub fn string_of_iter(iter: Iter) -> &'static str {
    il::print::string_of_iter(iter)
}

pub fn string_of_iterexp(iterexp: &IterExp) -> String {
    il::print::string_of_iterexp(iterexp)
}

pub fn string_of_iterexps(iterexps: &[IterExp]) -> String {
    join(iterexps, "", string_of_iterexp)
}

fn string_of_iterated(item: &Exp, iterexps: &[IterExp]) -> String {
    if iterexps.is_empty() {
        string_of_exp(item)
    } else {
        format!("({}){}", string_of_exp(item), string_of_iterexps(iterexps))
    }
}

// Variables

pub fn string_of_var(variable: &Var) -> String {
    il::print::string_of_var(variable)
}

// Types

pub fn string_of_typ(typ: &Typ) -> String {
    il::print::string_of_typ(typ)
}

pub fn string_of_typs(separator: &str, typs: &[Typ]) -> String {
    il::print::string_of_typs(separator, typs)
}

pub fn string_of_nottyp(nottyp: &NotTyp) -> String {
    il::print::string_of_nottyp(nottyp)
}

pub fn string_of_deftyp(deftyp: &DefTyp) -> String {
    il::print::string_of_deftyp(deftyp)
}

pub fn string_of_typfield(field: &TypField) -> String {
    il::print::string_of_typfield(field)
}

pub fn string_of_typfields(separator: &str, fields: &[TypField]) -> String {
    il::print::string_of_typfields(separator, fields)
}

pub fn string_of_typcase(case: &TypCase) -> String {
    il::print::string_of_typcase(case)
}

pub fn string_of_typcases(separator: &str, cases: &[TypCase]) -> String {
    il::print::string_of_typcases(separator, cases)
}

// Values

pub fn string_of_vid(vid: i64) -> String {
    format!("@{vid}")
}

pub fn string_of_value(value: &Value) -> String {
    il::print::string_of_value(value)
}

pub fn string_of_value_with(value: &Value, short: bool, level: usize) -> String {
    il::print::string_of_value_with(value, short, level)
}

// Operators

pub fn string_of_unop(operation: UnOp) -> &'static str {
    il::print::string_of_unop(operation)
}

pub fn string_of_binop(operation: BinOp) -> &'static str {
    il::print::string_of_binop(operation)
}

pub fn string_of_cmpop(operation: CmpOp) -> &'static str {
    il::print::string_of_cmpop(operation)
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
        ExpKind::UnE(operation, _, exp) => {
            output.write_str(string_of_unop(*operation))?;
            write_exp(output, exp)
        }
        ExpKind::BinE(operation, _, exp_l, exp_r) => {
            output.write_char('(')?;
            write_exp(output, exp_l)?;
            write!(output, " {} ", string_of_binop(*operation))?;
            write_exp(output, exp_r)?;
            output.write_char(')')
        }
        ExpKind::CmpE(operation, _, exp_l, exp_r) => {
            output.write_char('(')?;
            write_exp(output, exp_l)?;
            write!(output, " {} ", string_of_cmpop(*operation))?;
            write_exp(output, exp_r)?;
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
        ExpKind::MatchE(exp, pattern) => {
            output.write_char('(')?;
            write_exp(output, exp)?;
            write!(output, " matches pattern {})", string_of_pattern(pattern))
        }
        ExpKind::TupleE(exps) => {
            output.write_char('(')?;
            write_exps(output, ", ", exps)?;
            output.write_char(')')
        }
        ExpKind::CaseE(notexp) => write!(output, "({})", string_of_notexp(notexp)),
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
        ExpKind::OptE(Some(exp)) => {
            output.write_str("?(")?;
            write_exp(output, exp)?;
            output.write_char(')')
        }
        ExpKind::OptE(None) => output.write_str("?()"),
        ExpKind::ListE(exps) => {
            output.write_char('[')?;
            write_exps(output, ", ", exps)?;
            output.write_char(']')
        }
        ExpKind::ConsE(exp_h, exp_t) => {
            write_exp(output, exp_h)?;
            output.write_str(" :: ")?;
            write_exp(output, exp_t)
        }
        ExpKind::CatE(exp_l, exp_r) => {
            write_exp(output, exp_l)?;
            output.write_str(" ++ ")?;
            write_exp(output, exp_r)
        }
        ExpKind::MemE(exp_e, exp_s) => {
            write_exp(output, exp_e)?;
            output.write_str(" is in ")?;
            write_exp(output, exp_s)
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
        ExpKind::IdxE(exp_b, exp_i) => {
            write_exp(output, exp_b)?;
            output.write_char('[')?;
            write_exp(output, exp_i)?;
            output.write_char(']')
        }
        ExpKind::SliceE(exp_b, exp_l, exp_h) => {
            write_exp(output, exp_b)?;
            output.write_char('[')?;
            write_exp(output, exp_l)?;
            output.write_str(" : ")?;
            write_exp(output, exp_h)?;
            output.write_char(']')
        }
        ExpKind::UpdE(exp_b, path, exp_f) => {
            write_exp(output, exp_b)?;
            write!(output, "[{} = ", string_of_path(path))?;
            write_exp(output, exp_f)?;
            output.write_char(']')
        }
        ExpKind::CallE(id, targs, args) => write!(
            output,
            "{}{}{}",
            string_of_defid(id),
            string_of_targs(targs),
            string_of_args(args)
        ),
        ExpKind::IterE(exp, iterexp) => {
            output.write_str(&string_of_iterated(exp, std::slice::from_ref(iterexp)))
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

// Patterns

pub fn string_of_pattern(pattern: &Pattern) -> String {
    il::print::string_of_pattern(pattern)
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
    il::print::string_of_tparam(tparam)
}

pub fn string_of_tparams(tparams: &[TParam]) -> String {
    il::print::string_of_tparams(tparams)
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
    il::print::string_of_targ(targ)
}

pub fn string_of_targs(targs: &[Targ]) -> String {
    il::print::string_of_targs(targs)
}

// Danglings

pub fn string_of_dangle(iid: Iid) -> String {
    format!("Dangling#{iid}")
}

// Case analysis

pub fn string_of_case_with(case: &Case, level: usize, index: usize) -> String {
    let order = format!("{}{index}. ", "  ".repeat(level));
    format!(
        "{}Case {}\n\n{}",
        order,
        string_of_guard(&case.guard),
        string_of_block_with(&case.block, level + 1, 0)
    )
}

pub fn string_of_cases_with(cases: &[Case], level: usize) -> String {
    cases
        .iter()
        .enumerate()
        .map(|(index, case)| string_of_case_with(case, level, index + 1))
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub fn string_of_guard(guard: &Guard) -> String {
    let mut output = String::new();
    write_guard(&mut output, guard).expect("writing to a String cannot fail");
    output
}

fn write_guard(output: &mut dyn fmt::Write, guard: &Guard) -> fmt::Result {
    match guard {
        Guard::BoolG(value) => write!(output, "{value}"),
        Guard::CmpG(operation, _, exp) => {
            write!(output, "(% {} ", string_of_cmpop(*operation))?;
            write_exp(output, exp)?;
            output.write_char(')')
        }
        Guard::SubG(typ, _) => write!(output, "(% has type {})", string_of_typ(typ)),
        Guard::MatchG(pattern) => {
            write!(output, "(% matches pattern {})", string_of_pattern(pattern))
        }
        Guard::MemG(exp) => {
            output.write_str("(% is in ")?;
            write_exp(output, exp)?;
            output.write_char(')')
        }
    }
}

// Instructions

pub fn string_of_instr(instr: &Instr) -> String {
    let mut output = String::new();
    write_instr_with(&mut output, instr, false, 0, 0).expect("writing to a String cannot fail");
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
    match &instr.kind {
        InstrKind::IfI(exp, iterexps, block, dangle) => {
            write_order()?;
            output.write_str("If (")?;
            write_exp(output, exp)?;
            write!(output, "){}, then", string_of_iterexps(iterexps))?;
            if !short {
                output.write_str("\n\n")?;
                write_block_with(output, block, level + 1, 0)?;
                if *dangle {
                    write!(output, "\n\n{order}Else {}", string_of_dangle(instr.iid))?;
                }
            }
            Ok(())
        }
        InstrKind::HoldI(id, notation, iterexps, holdcase) => match holdcase {
            HoldCase::BothH(block_hold, block_not_hold) => {
                write_order()?;
                write!(
                    output,
                    "If ({}: {}){} holds, then",
                    string_of_relid(id),
                    string_of_notexp(notation),
                    string_of_iterexps(iterexps)
                )?;
                if !short {
                    output.write_str("\n\n")?;
                    write_block_with(output, block_hold, level + 1, 0)?;
                    write!(output, "\n\n{order}Else,\n\n")?;
                    write_block_with(output, block_not_hold, level + 1, 0)?;
                }
                Ok(())
            }
            HoldCase::HoldH(block, dangle) | HoldCase::NotHoldH(block, dangle) => {
                write_order()?;
                write!(
                    output,
                    "If ({}: {}){} {}, then",
                    string_of_relid(id),
                    string_of_notexp(notation),
                    string_of_iterexps(iterexps),
                    if matches!(holdcase, HoldCase::NotHoldH(..)) {
                        "does not hold"
                    } else {
                        "holds"
                    }
                )?;
                if !short {
                    output.write_str("\n\n")?;
                    write_block_with(output, block, level + 1, 0)?;
                    if *dangle {
                        write!(output, "\n\n{order}Else {}", string_of_dangle(instr.iid))?;
                    }
                }
                Ok(())
            }
        },
        InstrKind::CaseI(exp, cases, dangle) => {
            write_order()?;
            output.write_str("Case analysis on ")?;
            write_exp(output, exp)?;
            if !short {
                output.write_str("\n\n")?;
                write_cases_with(output, cases, level + 1)?;
                if *dangle {
                    write!(output, "\n\n{order}Else {}", string_of_dangle(instr.iid))?;
                }
            }
            Ok(())
        }
        InstrKind::GroupI(id, signature, exps, block) => {
            write_order()?;
            write!(
                output,
                "Group {}: {}",
                string_of_relid(id),
                string_of_relinput(signature, exps)
            )?;
            if !short {
                output.write_str("\n\n")?;
                write_block_with(output, block, level + 1, 0)?;
            }
            Ok(())
        }
        InstrKind::LetI(left, right, iterinstrs, block) => {
            write_order()?;
            output.write_str("(Let ")?;
            write_exp(output, left)?;
            output.write_str(" be ")?;
            write_exp(output, right)?;
            write!(output, "){}", string_of_iterinstrs(iterinstrs))?;
            if !short {
                output.write_str("\n\n")?;
                write_block_with(output, block, level + 1, 0)?;
            }
            Ok(())
        }
        InstrKind::RuleI(id, notation, _, iterinstrs, block) => {
            write_order()?;
            write!(
                output,
                "({}: {}){}",
                string_of_relid(id),
                string_of_notexp(notation),
                string_of_iterinstrs(iterinstrs)
            )?;
            if !short {
                output.write_str("\n\n")?;
                write_block_with(output, block, level + 1, 0)?;
            }
            Ok(())
        }
        InstrKind::ResultI(_, exps) if exps.is_empty() => {
            write_order()?;
            output.write_str("The relation holds")
        }
        InstrKind::ResultI(signature, exps) => {
            write_order()?;
            write!(
                output,
                "Result in: {}",
                string_of_reloutput(signature, exps)
            )
        }
        InstrKind::ReturnI(exp) => {
            write_order()?;
            output.write_str("Return ")?;
            write_exp(output, exp)
        }
        InstrKind::DebugI(exp, nested) => {
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

fn write_cases_with(output: &mut dyn fmt::Write, cases: &[Case], level: usize) -> fmt::Result {
    for (index, case) in cases.iter().enumerate() {
        if index != 0 {
            output.write_str("\n\n")?;
        }
        write!(
            output,
            "{}{next}. Case ",
            "  ".repeat(level),
            next = index + 1
        )?;
        write_guard(output, &case.guard)?;
        output.write_str("\n\n")?;
        write_block_with(output, &case.block, level + 1, 0)?;
    }
    Ok(())
}

pub fn string_of_instr_with(instr: &Instr, short: bool, level: usize, index: usize) -> String {
    let order = format!("{}{index}. ", "  ".repeat(level));
    match &instr.kind {
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
                    string_of_block_with(block, level + 1, 0),
                    if *dangle {
                        format!("\n\n{order}Else {}", string_of_dangle(instr.iid))
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
                            string_of_block_with(block_hold, level + 1, 0),
                            string_of_block_with(block_not_hold, level + 1, 0)
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
                            string_of_block_with(block, level + 1, 0),
                            if *dangle {
                                format!("\n\n{order}Else {}", string_of_dangle(instr.iid))
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
                    string_of_cases_with(cases, level + 1),
                    if *dangle {
                        format!("\n\n{order}Else {}", string_of_dangle(instr.iid))
                    } else {
                        String::new()
                    }
                )
            }
        }
        InstrKind::GroupI(id, signature, exps, block) => {
            let summary = format!(
                "Group {}: {}",
                string_of_relid(id),
                string_of_relinput(signature, exps)
            );
            if short {
                summary
            } else {
                format!(
                    "{order}{summary}\n\n{}",
                    string_of_block_with(block, level + 1, 0)
                )
            }
        }
        InstrKind::LetI(exp_l, exp_r, iterinstrs, block) => {
            let summary = format!(
                "(Let {} be {}){}",
                string_of_exp(exp_l),
                string_of_exp(exp_r),
                string_of_iterinstrs(iterinstrs)
            );
            if short {
                summary
            } else {
                format!(
                    "{order}{summary}\n\n{}",
                    string_of_block_with(block, level + 1, 0)
                )
            }
        }
        InstrKind::RuleI(id, notexp, _, iterinstrs, block) => {
            let summary = format!(
                "({}: {}){}",
                string_of_relid(id),
                string_of_notexp(notexp),
                string_of_iterinstrs(iterinstrs)
            );
            if short {
                summary
            } else {
                format!(
                    "{order}{summary}\n\n{}",
                    string_of_block_with(block, level + 1, 0)
                )
            }
        }
        InstrKind::ResultI(_, exps) if exps.is_empty() => {
            if short {
                "The relation holds".into()
            } else {
                format!("{order}The relation holds")
            }
        }
        InstrKind::ResultI(signature, exps) => {
            let summary = format!("Result in: {}", string_of_reloutput(signature, exps));
            if short {
                summary
            } else {
                format!("{order}{summary}")
            }
        }
        InstrKind::ReturnI(exp) => {
            let summary = format!("Return {}", string_of_exp(exp));
            if short {
                summary
            } else {
                format!("{order}{summary}")
            }
        }
        InstrKind::DebugI(exp, nested) => {
            let summary = format!("Debug: {}", string_of_exp(exp));
            if short {
                summary
            } else {
                format!(
                    "{order}{summary}\n\n{}",
                    string_of_instr_with(nested, false, level, index + 1)
                )
            }
        }
    }
}

pub fn string_of_block(block: &Block) -> String {
    let mut output = String::new();
    write_block_with(&mut output, block, 0, 0).expect("writing to a String cannot fail");
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

pub fn string_of_block_with(block: &Block, level: usize, index: usize) -> String {
    block
        .iter()
        .enumerate()
        .map(|(offset, instr)| string_of_instr_with(instr, false, level, index + offset + 1))
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub fn string_of_elseblock_with(block: &ElseBlock, level: usize, index: usize) -> String {
    format!(
        "{}{next}. Otherwise,\n\n{}",
        "  ".repeat(level),
        string_of_block_with(block, level + 1, 0),
        next = index + 1
    )
}

pub fn string_of_elseblock_opt_with(
    block: &Option<ElseBlock>,
    level: usize,
    index: usize,
) -> String {
    block.as_ref().map_or_else(String::new, |block| {
        format!("\n\n{}", string_of_elseblock_with(block, level, index))
    })
}

pub fn string_of_iterinstr(iterinstr: &IterInstr) -> String {
    il::print::string_of_iterprem(iterinstr)
}

pub fn string_of_iterinstrs(iterinstrs: &[IterInstr]) -> String {
    join(iterinstrs, "", string_of_iterinstr)
}

// Relations

/// Renders relation input expressions
///
/// `exps_input` must match `signature.input_hint.indices()`;
/// notation arity must be internally valid
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

/// Renders relation output expressions
///
/// `exps_output` must cover every non-input notation position;
/// notation arity must be internally valid
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

pub fn string_of_defined_rel(relation: &Rel) -> String {
    format!(
        "{}: {}\n\n{}{}",
        string_of_relid(&relation.id),
        string_of_relinput(&relation.signature, &relation.inputs),
        string_of_block(&relation.block),
        string_of_elseblock_opt_with(&relation.else_block, 0, relation.block.len())
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
        string_of_block_with(&row.block, 2, 0)
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
        string_of_block(&function.block),
        string_of_elseblock_opt_with(&function.else_block, 0, function.block.len())
    )
}

fn write_elseblock_opt_with(
    output: &mut dyn fmt::Write,
    block: &Option<ElseBlock>,
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
        write_block_with(output, block, level + 1, 0)?;
    }
    Ok(())
}

fn write_extern_rel(output: &mut dyn fmt::Write, relation: &ExternRel) -> fmt::Result {
    write!(
        output,
        "{}: {}",
        string_of_relid(&relation.id),
        string_of_relinput(&relation.signature, &relation.inputs)
    )
}

fn write_defined_rel(output: &mut dyn fmt::Write, relation: &Rel) -> fmt::Result {
    write!(
        output,
        "{}: {}\n\n",
        string_of_relid(&relation.id),
        string_of_relinput(&relation.signature, &relation.inputs)
    )?;
    write_block_with(output, &relation.block, 0, 0)?;
    write_elseblock_opt_with(output, &relation.else_block, 0, relation.block.len())
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

fn write_builtin_func(output: &mut dyn fmt::Write, function: &BuiltinFunc) -> fmt::Result {
    write!(
        output,
        "{}{}{}",
        string_of_defid(&function.id),
        string_of_tparams(&function.tparams),
        string_of_params(&function.params)
    )
}

fn write_tablerow(output: &mut dyn fmt::Write, row: &TableRow) -> fmt::Result {
    write!(
        output,
        "\n  Row : {} -> ",
        string_of_exps(", ", &row.inputs)
    )?;
    write_exp(output, &row.expression)?;
    output.write_str(":\n\n")?;
    write_block_with(output, &row.block, 2, 0)
}

fn write_table_func(output: &mut dyn fmt::Write, function: &TableFunc) -> fmt::Result {
    write!(
        output,
        "{}{}\n=\n",
        string_of_defid(&function.id),
        string_of_params(&function.params)
    )?;
    for (index, row) in function.rows.iter().enumerate() {
        if index != 0 {
            output.write_char('\n')?;
        }
        write_tablerow(output, row)?;
    }
    Ok(())
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
        DefKind::ExternRelD(relation) => {
            output.write_str("extern relation ")?;
            write_extern_rel(output, relation)
        }
        DefKind::RelD(relation) => {
            output.write_str("relation ")?;
            write_defined_rel(output, relation)
        }
        DefKind::ExternDecD(function) => {
            output.write_str("extern def ")?;
            write_extern_func(output, function)
        }
        DefKind::BuiltinDecD(function) => {
            output.write_str("builtin def ")?;
            write_builtin_func(output, function)
        }
        DefKind::TableDecD(function) => {
            output.write_str("tbl def ")?;
            write_table_func(output, function)
        }
        DefKind::FuncDecD(function) => {
            output.write_str("def ")?;
            write_defined_func(output, function)
        }
    }
}

pub fn string_of_defs(definitions: &[Def]) -> String {
    join(definitions, "\n\n", string_of_def)
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
