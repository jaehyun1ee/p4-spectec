//! Text rendering for structured-language data

use std::fmt::{self, Write};

use crate::lang::{
    sl::ast::*,
    traits::print::{Print, Printer},
};

// == Printing

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

// - Case analysis

fn write_case_with(output: &mut Printer<'_>, case: &Case, index: usize) -> fmt::Result {
    output.write_indent()?;
    write!(output, "{index}. Case ")?;
    case.guard.print(output)?;
    output.write_str("\n\n")?;
    output.indented(|output| write_block_with(output, &case.block, 0))
}

fn write_cases_with(output: &mut Printer<'_>, cases: &[Case]) -> fmt::Result {
    for (index, case) in cases.iter().enumerate() {
        if index != 0 {
            output.write_str("\n\n")?;
        }
        write_case_with(output, case, index + 1)?;
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
        }
    }
}

// - Instructions

/// Renders instr with
pub fn render_instr_with(instr: &Instr, short: bool, level: usize, index: usize) -> String {
    let mut output = String::new();
    write_instr_with(
        &mut Printer::with_level(&mut output, level),
        instr,
        short,
        index,
    )
    .expect("writing to a String cannot fail");
    output
}

fn write_instr_with(
    output: &mut Printer<'_>,
    instr: &Instr,
    short: bool,
    index: usize,
) -> fmt::Result {
    let write_order = |output: &mut Printer<'_>| {
        if short {
            Ok(())
        } else {
            output.write_indent()?;
            write!(output, "{index}. ")
        }
    };
    match &instr.node.kind {
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
                output.indented(|output| write_block_with(output, block, 0))?;
                if *dangle {
                    output.write_str("\n\n")?;
                    write_order(output)?;
                    write!(output, "Else Dangling#{}", instr.node.note)?;
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
                write_order(output)?;
                output.write_str("If (")?;
                id.print(output)?;
                output.write_str(": ")?;
                not_exp.print(output)?;
                output.write_char(')')?;
                iter_exps.as_slice().print(output)?;
                output.write_str(" holds, then")?;
                if !short {
                    output.write_str("\n\n")?;
                    output.indented(|output| write_block_with(output, block_hold, 0))?;
                    output.write_str("\n\n")?;
                    write_order(output)?;
                    output.write_str("Else,\n\n")?;
                    output.indented(|output| write_block_with(output, block_not_hold, 0))?;
                }
                Ok(())
            }
            HoldCase::Hold(block, dangle) | HoldCase::NotHold(block, dangle) => {
                write_order(output)?;
                output.write_str("If (")?;
                id.print(output)?;
                output.write_str(": ")?;
                not_exp.print(output)?;
                output.write_char(')')?;
                iter_exps.as_slice().print(output)?;
                output.write_char(' ')?;
                output.write_str(if matches!(hold_case, HoldCase::NotHold(..)) {
                    "does not hold"
                } else {
                    "holds"
                })?;
                output.write_str(", then")?;
                if !short {
                    output.write_str("\n\n")?;
                    output.indented(|output| write_block_with(output, block, 0))?;
                    if *dangle {
                        output.write_str("\n\n")?;
                        write_order(output)?;
                        write!(output, "Else Dangling#{}", instr.node.note)?;
                    }
                }
                Ok(())
            }
        },
        InstrKind::Case(CaseInstr { exp, cases, dangle }) => {
            write_order(output)?;
            output.write_str("Case analysis on ")?;
            exp.print(output)?;
            if !short {
                output.write_str("\n\n")?;
                output.indented(|output| write_cases_with(output, cases))?;
                if *dangle {
                    output.write_str("\n\n")?;
                    write_order(output)?;
                    write!(output, "Else Dangling#{}", instr.node.note)?;
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
            write_order(output)?;
            output.write_str("Group ")?;
            id.print(output)?;
            output.write_str(": ")?;
            write_relinput(output, rel_signature, exps)?;
            if !short {
                output.write_str("\n\n")?;
                output.indented(|output| write_block_with(output, block, 0))?;
            }
            Ok(())
        }
        InstrKind::Let(LetInstr {
            exp_l,
            exp_r,
            iter_instrs,
            block,
        }) => {
            write_order(output)?;
            output.write_str("(Let ")?;
            exp_l.print(output)?;
            output.write_str(" be ")?;
            exp_r.print(output)?;
            output.write_char(')')?;
            iter_instrs.as_slice().print(output)?;
            if !short {
                output.write_str("\n\n")?;
                output.indented(|output| write_block_with(output, block, 0))?;
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
            write_order(output)?;
            output.write_char('(')?;
            id.print(output)?;
            output.write_str(": ")?;
            not_exp.print(output)?;
            output.write_char(')')?;
            iter_instrs.as_slice().print(output)?;
            if !short {
                output.write_str("\n\n")?;
                output.indented(|output| write_block_with(output, block, 0))?;
            }
            Ok(())
        }
        InstrKind::Result(ResultInstr { exps, .. }) if exps.is_empty() => {
            write_order(output)?;
            output.write_str("The relation holds")
        }
        InstrKind::Result(ResultInstr {
            rel_signature,
            exps,
        }) => {
            write_order(output)?;
            output.write_str("Result in: ")?;
            write_reloutput(output, rel_signature, exps)
        }
        InstrKind::Return(ReturnInstr { exp }) => {
            write_order(output)?;
            output.write_str("Return ")?;
            exp.print(output)
        }
        InstrKind::Debug(DebugInstr { exp, instr: nested }) => {
            write_order(output)?;
            output.write_str("Debug: ")?;
            exp.print(output)?;
            if !short {
                output.write_str("\n\n")?;
                write_instr_with(output, nested, false, index + 1)?;
            }
            Ok(())
        }
    }
}

fn write_block_with(output: &mut Printer<'_>, block: &Block, index: usize) -> fmt::Result {
    for (offset, instr) in block.iter().enumerate() {
        if offset != 0 {
            output.write_str("\n\n")?;
        }
        write_instr_with(output, instr, false, index + offset + 1)?;
    }
    Ok(())
}

fn write_elseblock_with(output: &mut Printer<'_>, block: &ElseBlock, index: usize) -> fmt::Result {
    output.write_indent()?;
    write!(output, "{next}. Otherwise,\n\n", next = index + 1)?;
    output.indented(|output| write_block_with(output, block, 0))
}

fn write_elseblock_opt_with(
    output: &mut Printer<'_>,
    block: &Option<ElseBlock>,
    index: usize,
) -> fmt::Result {
    if let Some(block) = block {
        output.write_str("\n\n")?;
        write_elseblock_with(output, block, index)?;
    }
    Ok(())
}

impl Print for Instr {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        write_instr_with(printer, self, false, 0)
    }
}

impl Print for Block {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        write_block_with(printer, self, 0)
    }
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
        write_block_with(printer, &self.block, 0)?;
        write_elseblock_opt_with(printer, &self.else_block, self.block.len())
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
        printer.indented(|printer| {
            printer.indented(|printer| write_block_with(printer, &self.block, 0))
        })
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
        for (index, table_row) in self.table_rows.iter().enumerate() {
            if index != 0 {
                printer.write_char('\n')?;
            }
            table_row.print(printer)?;
        }
        Ok(())
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
        write_block_with(printer, &self.block, 0)?;
        write_elseblock_opt_with(printer, &self.else_block, self.block.len())
    }
}

// - Definitions

impl Print for Def {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        match &self.node {
            DefKind::ExternTyp(ExternTypDef { id, .. }) => {
                printer.write_str("extern syntax ")?;
                id.print(printer)
            }
            DefKind::Typ(TypDef {
                id,
                tparams,
                def_typ,
                ..
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
            DefKind::Var(VarDef { id, typ, .. }) => {
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
