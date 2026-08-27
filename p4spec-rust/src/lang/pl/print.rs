//! Text rendering for prose-language data

use std::fmt::{self, Write};

use crate::lang::traits::print::{Print, Printer};

use super::ast::*;

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

// - Expressions

impl Print for Exp {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        match &self.node.node.kind {
            ExpKind::Bool(value) => write!(printer, "{value}"),
            ExpKind::Num(value) => value.print(printer),
            ExpKind::Text(text) => write!(printer, "\"{}\"", escaped(text)),
            ExpKind::Var(id) => printer.write_str(&id.node),
            ExpKind::Un(op, _, exp) => {
                op.print(printer)?;
                exp.print(printer)
            }
            ExpKind::Bin(op, _, exp_l, exp_r) => {
                printer.write_char('(')?;
                exp_l.print(printer)?;
                printer.write_char(' ')?;
                op.print(printer)?;
                printer.write_char(' ')?;
                exp_r.print(printer)?;
                printer.write_char(')')
            }
            ExpKind::Cmp(op, _, exp_l, exp_r) => {
                printer.write_char('(')?;
                exp_l.print(printer)?;
                printer.write_char(' ')?;
                op.print(printer)?;
                printer.write_char(' ')?;
                exp_r.print(printer)?;
                printer.write_char(')')
            }
            ExpKind::UpCast(typ, exp) | ExpKind::DownCast(typ, exp) => {
                printer.write_char('(')?;
                exp.print(printer)?;
                printer.write_str(" as ")?;
                typ.print(printer)?;
                printer.write_char(')')
            }
            ExpKind::Sub(exp, typ, _) => {
                printer.write_char('(')?;
                exp.print(printer)?;
                printer.write_str(" has type ")?;
                typ.print(printer)?;
                printer.write_char(')')
            }
            ExpKind::Match(exp, pattern) => {
                printer.write_char('(')?;
                exp.print(printer)?;
                printer.write_str(" matches pattern ")?;
                pattern.print(printer)?;
                printer.write_char(')')
            }
            ExpKind::Tuple(exps) => {
                printer.write_char('(')?;
                printer.separated(exps, ", ")?;
                printer.write_char(')')
            }
            ExpKind::Case(not_exp) => {
                printer.write_char('(')?;
                not_exp.print(printer)?;
                printer.write_char(')')
            }
            ExpKind::Str(fields) => {
                printer.write_char('{')?;
                for (index, (atom, exp)) in fields.iter().enumerate() {
                    if index > 0 {
                        printer.write_str(", ")?;
                    }
                    atom.print(printer)?;
                    printer.write_char(' ')?;
                    exp.print(printer)?;
                }
                printer.write_char('}')
            }
            ExpKind::Opt(Some(exp)) => {
                printer.write_str("?(")?;
                exp.print(printer)?;
                printer.write_char(')')
            }
            ExpKind::Opt(None) => printer.write_str("?()"),
            ExpKind::List(exps) => {
                printer.write_char('[')?;
                printer.separated(exps, ", ")?;
                printer.write_char(']')
            }
            ExpKind::Cons(exp_head, exp_tail) => {
                exp_head.print(printer)?;
                printer.write_str(" :: ")?;
                exp_tail.print(printer)
            }
            ExpKind::Cat(exp_l, exp_r) => {
                exp_l.print(printer)?;
                printer.write_str(" ++ ")?;
                exp_r.print(printer)
            }
            ExpKind::Mem(exp_e, exp_s) => {
                exp_e.print(printer)?;
                printer.write_str(" is in ")?;
                exp_s.print(printer)
            }
            ExpKind::Len(exp) => {
                printer.write_char('|')?;
                exp.print(printer)?;
                printer.write_char('|')
            }
            ExpKind::Dot(exp, atom) => {
                exp.print(printer)?;
                printer.write_char('.')?;
                atom.print(printer)
            }
            ExpKind::Idx(exp_b, exp_i) => {
                exp_b.print(printer)?;
                printer.write_char('[')?;
                exp_i.print(printer)?;
                printer.write_char(']')
            }
            ExpKind::Slice(exp_b, exp_i, exp_n) => {
                exp_b.print(printer)?;
                printer.write_char('[')?;
                exp_i.print(printer)?;
                printer.write_str(" : ")?;
                exp_n.print(printer)?;
                printer.write_char(']')
            }
            ExpKind::Upd(exp_b, path, exp_f) => {
                exp_b.print(printer)?;
                printer.write_char('[')?;
                path.print(printer)?;
                printer.write_str(" = ")?;
                exp_f.print(printer)?;
                printer.write_char(']')
            }
            ExpKind::Call(id, targs, args) => {
                printer.write_char('$')?;
                id.print(printer)?;
                if !targs.is_empty() {
                    printer.write_char('<')?;
                    printer.separated(targs, ", ")?;
                    printer.write_char('>')?;
                }
                args.as_slice().print(printer)
            }
            ExpKind::Iter(exp, iter_exp) => {
                printer.write_char('(')?;
                exp.print(printer)?;
                printer.write_char(')')?;
                std::slice::from_ref(iter_exp).print(printer)
            }
        }
    }
}

impl Print for NotExp {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        self.print_with(printer, |exp, printer| exp.print(printer))
    }
}

// - Paths

impl Print for Path {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        match &self.node.kind {
            PathKind::Root => Ok(()),
            PathKind::Idx(path, exp_i) => {
                path.print(printer)?;
                printer.write_char('[')?;
                exp_i.print(printer)?;
                printer.write_char(']')
            }
            PathKind::Slice(path, exp_i, exp_n) => {
                path.print(printer)?;
                printer.write_char('[')?;
                exp_i.print(printer)?;
                printer.write_str(" : ")?;
                exp_n.print(printer)?;
                printer.write_char(']')
            }
            PathKind::Dot(path, atom) if matches!(path.node.kind, PathKind::Root) => {
                atom.print(printer)
            }
            PathKind::Dot(path, atom) => {
                path.print(printer)?;
                printer.write_char('.')?;
                atom.print(printer)
            }
        }
    }
}

// - Parameters

impl Print for Param {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        match &self.node {
            ParamKind::Exp(_, exp) => exp.print(printer),
            ParamKind::Def(id, tparams, params, typ) => {
                printer.write_char('$')?;
                id.print(printer)?;
                if !tparams.is_empty() {
                    printer.write_char('<')?;
                    printer.separated(tparams, ", ")?;
                    printer.write_char('>')?;
                }
                params.as_slice().print(printer)?;
                printer.write_str(" : ")?;
                typ.print(printer)
            }
        }
    }
}

impl Print for [Param] {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        if self.is_empty() {
            return Ok(());
        }
        printer.write_char('(')?;
        for (index, param) in self.iter().enumerate() {
            if index != 0 {
                printer.write_str(", ")?;
            }
            param.print(printer)?;
        }
        printer.write_char(')')
    }
}

// - Arguments

impl Print for Arg {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        match &self.node {
            ArgKind::Exp(exp) => exp.print(printer),
            ArgKind::Def(id) => {
                printer.write_char('$')?;
                id.print(printer)
            }
        }
    }
}

impl Print for [Arg] {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        if self.is_empty() {
            return Ok(());
        }
        printer.write_char('(')?;
        for (index, arg) in self.iter().enumerate() {
            if index != 0 {
                printer.write_str(", ")?;
            }
            arg.print(printer)?;
        }
        printer.write_char(')')
    }
}

// - Case analysis

type TierPrinter<Tier> = fn(&mut Printer<'_>, &Tier, bool, usize, usize) -> fmt::Result;

fn write_case_with<Tier>(
    output: &mut Printer<'_>,
    case: &Case<Tier>,
    tier_printer: TierPrinter<Tier>,
    level: usize,
    index: usize,
) -> fmt::Result {
    write!(output, "{}{index}. Case ", "  ".repeat(level))?;
    case.guard.print(output)?;
    output.write_str("\n\n")?;
    write_block_with(output, &case.block, tier_printer, level + 1, 0)
}

fn write_cases_with<Tier>(
    output: &mut Printer<'_>,
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

impl Print for Guard {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        match self {
            Guard::Bool(value) => write!(printer, "{value}"),
            Guard::Cmp(op, _, exp) => {
                printer.write_str("(% ")?;
                op.print(printer)?;
                printer.write_char(' ')?;
                exp.print(printer)?;
                printer.write_char(')')
            }
            Guard::Sub(typ, _) => {
                printer.write_str("(% has type ")?;
                typ.print(printer)?;
                printer.write_char(')')
            }
            Guard::Match(pattern) => {
                printer.write_str("(% matches pattern ")?;
                pattern.print(printer)?;
                printer.write_char(')')
            }
            Guard::Mem(exp) => {
                printer.write_str("(% is in ")?;
                exp.print(printer)?;
                printer.write_char(')')
            }
            Guard::CheckLetSub(typ, _, exp) => {
                printer.write_str("(let ")?;
                exp.print(printer)?;
                printer.write_str(" be %, % has type ")?;
                typ.print(printer)?;
                printer.write_char(')')
            }
            Guard::CheckLetMatch(pattern, exp) => {
                printer.write_str("(let ")?;
                exp.print(printer)?;
                printer.write_str(" be %, % matches pattern ")?;
                pattern.print(printer)?;
                printer.write_char(')')
            }
        }
    }
}

// - Instructions
// Shared control flow parameterized by the tier

fn write_instr_with<Tier>(
    output: &mut Printer<'_>,
    instr: &Instr<Tier>,
    tier_printer: TierPrinter<Tier>,
    short: bool,
    level: usize,
    index: usize,
) -> fmt::Result {
    let order = format!("{}{index}. ", "  ".repeat(level));
    let write_order = |output: &mut Printer<'_>| {
        if short {
            Ok(())
        } else {
            output.write_str(&order)
        }
    };

    match &instr.node.node.kind {
        InstrKind::If(IfInstr {
            exp,
            iter_exps,
            block,
            dangle,
        }) => {
            write_order(output)?;
            output.write_str("If (")?;
            exp.print(output)?;
            output.write_char(')')?;
            iter_exps.as_slice().print(output)?;
            output.write_str(", then")?;
            if !short {
                output.write_str("\n\n")?;
                write_block_with(output, block, tier_printer, level + 1, 0)?;
                if *dangle {
                    write!(
                        output,
                        "\n\n{order}Else Dangling#{}",
                        instr.node.node.note.iid
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
        }) => {
            let write_holding = |output: &mut Printer<'_>, negative: bool| {
                output.write_str("If (")?;
                id.print(output)?;
                output.write_str(": ")?;
                not_exp.print(output)?;
                output.write_char(')')?;
                iter_exps.as_slice().print(output)?;
                output.write_char(' ')?;
                output.write_str(if negative { "does not hold" } else { "holds" })?;
                output.write_str(", then")
            };
            match hold_case {
                HoldCase::Both(block_hold, block_not_hold) => {
                    write_order(output)?;
                    write_holding(output, false)?;
                    if !short {
                        output.write_str("\n\n")?;
                        write_block_with(output, block_hold, tier_printer, level + 1, 0)?;
                        write!(output, "\n\n{order}Else,\n\n")?;
                        write_block_with(output, block_not_hold, tier_printer, level + 1, 0)?;
                    }
                    Ok(())
                }
                HoldCase::Hold(block, dangle) | HoldCase::NotHold(block, dangle) => {
                    write_order(output)?;
                    write_holding(output, matches!(hold_case, HoldCase::NotHold(..)))?;
                    if !short {
                        output.write_str("\n\n")?;
                        write_block_with(output, block, tier_printer, level + 1, 0)?;
                        if *dangle {
                            write!(
                                output,
                                "\n\n{order}Else Dangling#{}",
                                instr.node.node.note.iid
                            )?;
                        }
                    }
                    Ok(())
                }
            }
        }
        InstrKind::Case(CaseInstr { exp, cases, dangle }) => {
            write_order(output)?;
            output.write_str("Case analysis on ")?;
            exp.print(output)?;
            if !short {
                output.write_str("\n\n")?;
                write_cases_with(output, cases, tier_printer, level + 1)?;
                if *dangle {
                    write!(
                        output,
                        "\n\n{order}Else Dangling#{}",
                        instr.node.node.note.iid
                    )?;
                }
            }
            Ok(())
        }
        InstrKind::Let(LetInstr {
            exp_l,
            exp_r,
            iter_instrs,
        }) => {
            write_order(output)?;
            output.write_str("(Let ")?;
            exp_l.print(output)?;
            output.write_str(" be ")?;
            exp_r.print(output)?;
            output.write_char(')')?;
            iter_instrs.as_slice().print(output)
        }
        InstrKind::Debug(DebugInstr { exp }) => {
            write_order(output)?;
            output.write_str("Debug: ")?;
            exp.print(output)
        }
        InstrKind::Destruct(DestructInstr {
            bindings: fields,
            exp: exp_r,
        }) => {
            write_order(output)?;
            output.write_str("(Destruct (")?;
            for (index, (_, exp)) in fields.iter().enumerate() {
                if index != 0 {
                    output.write_str(", ")?;
                }
                exp.print(output)?;
            }
            output.write_str(") = ")?;
            exp_r.print(output)?;
            output.write_char(')')
        }
        InstrKind::CheckLetSub(CheckLetSubInstr {
            typ,
            exp_l,
            exp_r,
            block,
            ..
        }) => {
            write_order(output)?;
            output.write_str("(Let ")?;
            exp_l.print(output)?;
            output.write_str(" be ")?;
            exp_r.print(output)?;
            output.write_str(", ")?;
            exp_r.print(output)?;
            output.write_str(" has type ")?;
            typ.print(output)?;
            output.write_char(')')?;
            if !short {
                output.write_str("\n\n")?;
                write_block_with(output, block, tier_printer, level + 1, 0)?;
            }
            Ok(())
        }
        InstrKind::CheckLetMatch(CheckLetMatchInstr {
            pattern,
            exp_l,
            exp_r,
            block,
        }) => {
            write_order(output)?;
            output.write_str("(Let ")?;
            exp_l.print(output)?;
            output.write_str(" be ")?;
            exp_r.print(output)?;
            output.write_str(", ")?;
            exp_r.print(output)?;
            output.write_str(" matches pattern ")?;
            pattern.print(output)?;
            output.write_char(')')?;
            if !short {
                output.write_str("\n\n")?;
                write_block_with(output, block, tier_printer, level + 1, 0)?;
            }
            Ok(())
        }
        InstrKind::OptionGet(OptionGetInstr {
            exp_l,
            exp_r,
            block,
        }) => {
            write_order(output)?;
            output.write_str("(Let ")?;
            exp_l.print(output)?;
            output.write_str(" be ! ")?;
            exp_r.print(output)?;
            output.write_char(')')?;
            if !short {
                output.write_str("\n\n")?;
                write_block_with(output, block, tier_printer, level + 1, 0)?;
            }
            Ok(())
        }
        InstrKind::Tier(TierInstr { tier }) => tier_printer(output, tier, short, level, index),
    }
}

fn write_block_with<Tier>(
    output: &mut Printer<'_>,
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

fn write_elseblock_opt_with<Tier>(
    output: &mut Printer<'_>,
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

// - Relations

fn write_relinput(
    output: &mut Printer<'_>,
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
            .map(|position| &exps_input[position])
    });
    let mixfix =
        Mixop::fill(&not_typ.node.to_mixop(), args).expect("relation input arity matches notation");
    mixfix.print_with(output, |exp, output| match exp {
        Some(exp) => exp.print(output),
        None => output.write("%"),
    })
}

fn write_reloutput(
    output: &mut Printer<'_>,
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
            .map(|position| &exps_output[position])
    });
    let mixfix = Mixop::fill(&not_typ.node.to_mixop(), args)
        .expect("relation output arity matches notation");
    mixfix.print_with(output, |exp, output| match exp {
        Some(exp) => exp.print(output),
        None => output.write("%"),
    })
}

impl Print for ExternRel {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        self.id.print(printer)?;
        printer.write_str(": ")?;
        write_relinput(printer, &self.rel_signature, &self.exps_input)
    }
}

impl Print for Rel {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        self.id.print(printer)?;
        printer.write_str(": ")?;
        write_relinput(printer, &self.rel_signature, &self.exps_input)?;
        printer.write_str("\n\n")?;
        self.block.print(printer)?;
        write_elseblock_opt_with(
            printer,
            &self.block_else_opt,
            write_instr_dispatch_tier_with,
            0,
            self.block.len(),
        )
    }
}

// - Group-body tier

fn write_instr_group_tier_with(
    output: &mut Printer<'_>,
    tier: &InstrGroup,
    short: bool,
    level: usize,
    index: usize,
) -> fmt::Result {
    let order = format!("{}{index}. ", "  ".repeat(level));
    if !short {
        output.write_str(&order)?;
    }

    match tier {
        InstrGroup::Result(ResultGroupInstr { exps_output, .. }) if exps_output.is_empty() => {
            output.write_str("The relation holds")
        }
        InstrGroup::Result(ResultGroupInstr {
            rel_signature,
            exps_output,
        }) => {
            output.write_str("Result in: ")?;
            write_reloutput(output, rel_signature, exps_output)
        }
        InstrGroup::Return(ReturnGroupInstr { exp }) => {
            output.write_str("Return ")?;
            exp.print(output)
        }
        InstrGroup::Rule(RuleGroupInstr {
            id,
            not_exp,
            iter_instrs,
            ..
        }) => {
            output.write_char('(')?;
            id.print(output)?;
            output.write_str(": ")?;
            not_exp.print(output)?;
            output.write_char(')')?;
            iter_instrs.as_slice().print(output)
        }
        InstrGroup::Backtrack(BacktrackGroupInstr { blocks }) => {
            write!(output, "Block ({} arms)", blocks.len())?;
            if !short {
                let indent = "  ".repeat(level);
                output.write_str("\n\n")?;
                for (arm_index, arm) in blocks.iter().enumerate() {
                    if arm_index != 0 {
                        output.write_str("\n\n")?;
                    }
                    write!(output, "{indent}Arm {}:\n\n", arm_index + 1)?;
                    write_block_group_with(output, arm, level + 1, 0)?;
                }
            }
            Ok(())
        }
    }
}

fn write_block_group_with(
    output: &mut Printer<'_>,
    block: &BlockGroup,
    level: usize,
    index: usize,
) -> fmt::Result {
    write_block_with(output, block, write_instr_group_tier_with, level, index)
}

impl Print for Instr<InstrGroup> {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        write_instr_with(printer, self, write_instr_group_tier_with, false, 0, 0)
    }
}

impl Print for BlockGroup {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        write_block_group_with(printer, self, 0, 0)
    }
}

// - Dispatch tier

fn write_instr_dispatch_tier_with(
    output: &mut Printer<'_>,
    tier: &InstrDispatch,
    short: bool,
    level: usize,
    index: usize,
) -> fmt::Result {
    let order = format!("{}{index}. ", "  ".repeat(level));
    if !short {
        output.write_str(&order)?;
    }

    match tier {
        InstrDispatch::Group(GroupDispatchInstr {
            id_group,
            rel_signature,
            exps_input,
            block,
            ..
        }) => {
            output.write_str("Group ")?;
            id_group.print(output)?;
            output.write_str(": ")?;
            write_relinput(output, rel_signature, exps_input)?;
            if !short {
                output.write_str("\n\n")?;
                write_block_group_with(output, block, level + 1, 0)?;
            }
            Ok(())
        }
        InstrDispatch::Route(RouteDispatchInstr { blocks }) => {
            write!(output, "Block ({} arms)", blocks.len())?;
            if !short {
                let indent = "  ".repeat(level);
                output.write_str("\n\n")?;
                for (arm_index, arm) in blocks.iter().enumerate() {
                    if arm_index != 0 {
                        output.write_str("\n\n")?;
                    }
                    write!(output, "{indent}Arm {}:\n\n", arm_index + 1)?;
                    write_block_dispatch_with(output, arm, level + 1, 0)?;
                }
            }
            Ok(())
        }
    }
}

fn write_block_dispatch_with(
    output: &mut Printer<'_>,
    block: &BlockDispatch,
    level: usize,
    index: usize,
) -> fmt::Result {
    write_block_with(output, block, write_instr_dispatch_tier_with, level, index)
}

impl Print for Instr<InstrDispatch> {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        write_instr_with(printer, self, write_instr_dispatch_tier_with, false, 0, 0)
    }
}

impl Print for BlockDispatch {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        write_block_dispatch_with(printer, self, 0, 0)
    }
}

// - Functions

impl Print for ExternFunc {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        printer.write_char('$')?;
        self.id.print(printer)?;
        if !self.tparams.is_empty() {
            printer.write_char('<')?;
            printer.separated(&self.tparams, ", ")?;
            printer.write_char('>')?;
        }
        self.params.as_slice().print(printer)
    }
}

impl Print for BuiltinFunc {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        printer.write_char('$')?;
        self.id.print(printer)?;
        if !self.tparams.is_empty() {
            printer.write_char('<')?;
            printer.separated(&self.tparams, ", ")?;
            printer.write_char('>')?;
        }
        self.params.as_slice().print(printer)
    }
}

impl Print for TableRow {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        printer.write_str("\n  Row : ")?;
        printer.separated(&self.exps_input, ", ")?;
        printer.write_str(" -> ")?;
        self.exp.print(printer)?;
        printer.write_str(":\n\n")?;
        write_block_group_with(printer, &self.block, 2, 0)
    }
}

impl Print for [TableRow] {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        for (index, table_row) in self.iter().enumerate() {
            if index != 0 {
                printer.write_char('\n')?;
            }
            table_row.print(printer)?;
        }
        Ok(())
    }
}

impl Print for TableFunc {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        printer.write_char('$')?;
        self.id.print(printer)?;
        self.params.as_slice().print(printer)?;
        printer.write_str("\n=\n")?;
        self.rows.print(printer)
    }
}

impl Print for DefinedFunc {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        printer.write_char('$')?;
        self.id.print(printer)?;
        if !self.tparams.is_empty() {
            printer.write_char('<')?;
            printer.separated(&self.tparams, ", ")?;
            printer.write_char('>')?;
        }
        self.params.as_slice().print(printer)?;
        printer.write_str("\n\n")?;
        self.block.print(printer)?;
        write_elseblock_opt_with(
            printer,
            &self.block_else_opt,
            write_instr_group_tier_with,
            0,
            self.block.len(),
        )
    }
}

// - Definitions

impl Print for Def {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        match &self.node.node {
            DefKind::ExternTyp(ExternTypDef { id }) => {
                printer.write_str("extern syntax ")?;
                id.print(printer)
            }
            DefKind::Typ(TypDef {
                id,
                tparams,
                def_typ,
            }) => {
                printer.write_str("syntax ")?;
                id.print(printer)?;
                if !tparams.is_empty() {
                    printer.write_char('<')?;
                    printer.separated(tparams, ", ")?;
                    printer.write_char('>')?;
                }
                printer.write_str(" = ")?;
                def_typ.print(printer)
            }
            DefKind::Var(VarDef { id, typ }) => {
                printer.write_str("var ")?;
                id.print(printer)?;
                printer.write_str(" : ")?;
                typ.print(printer)
            }
            DefKind::ExternRel(relation) => {
                printer.write_str("extern relation ")?;
                relation.print(printer)
            }
            DefKind::Rel(relation) => {
                printer.write_str("relation ")?;
                relation.print(printer)
            }
            DefKind::ExternDec(function) => {
                printer.write_str("extern def ")?;
                function.print(printer)
            }
            DefKind::BuiltinDec(function) => {
                printer.write_str("builtin def ")?;
                function.print(printer)
            }
            DefKind::TableDec(function) => {
                printer.write_str("tbl def ")?;
                function.print(printer)
            }
            DefKind::FuncDec(function) => {
                printer.write_str("def ")?;
                function.print(printer)
            }
        }
    }
}

impl Print for [Def] {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        for (index, definition) in self.iter().enumerate() {
            if index != 0 {
                printer.write_str("\n\n")?;
            }
            definition.print(printer)?;
        }
        Ok(())
    }
}

// - Specifications

impl Print for Spec {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        self.as_slice().print(printer)
    }
}
