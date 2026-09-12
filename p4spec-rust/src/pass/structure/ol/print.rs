//! OL diagnostic layout follows the source printer, including Debug numbering
use super::ast::*;
use crate::lang::{
    sl::ast::Mixop,
    traits::print::{Print, Printer},
};
use std::fmt::{self, Write};

fn write_guard(output: &mut Printer<'_>, guard: &Guard) -> fmt::Result {
    match guard {
        Guard::Mem(exp) => {
            output.write_str("(% in ")?;
            exp.print(output)?;
            output.write_char(')')
        }
        _ => guard.print(output),
    }
}

fn write_case(output: &mut Printer<'_>, case: &Case, level: usize, index: usize) -> fmt::Result {
    write!(output, "{}{index}. Case ", "  ".repeat(level))?;
    write_guard(output, &case.guard)?;
    output.write_str("\n\n")?;
    write_block(output, &case.block, level + 1)
}

fn write_instr(output: &mut Printer<'_>, instr: &Instr, level: usize, index: usize) -> fmt::Result {
    let order = format!("{}{index}. ", "  ".repeat(level));
    output.write_str(&order)?;
    let instr_kind = &instr.node;
    match instr_kind {
        InstrKind::If(instr) => write_if_instr(output, instr, level),
        InstrKind::Hold(instr) => write_hold_instr(output, instr, level, &order),
        InstrKind::Case(instr) => write_case_instr(output, instr, level),
        InstrKind::Group(instr) => write_group_instr(output, instr, level),
        InstrKind::Let(instr) => write_let_instr(output, instr, level),
        InstrKind::Rule(instr) => write_rule_instr(output, instr, level),
        InstrKind::Result(instr) => write_result_instr(output, instr),
        InstrKind::Return(instr) => write_return_instr(output, instr),
        InstrKind::Debug(instr) => write_debug_instr(output, instr),
    }
}

fn write_if_instr(output: &mut Printer<'_>, instr: &IfInstr, level: usize) -> fmt::Result {
    let IfInstr {
        exp,
        iter_exps,
        block,
    } = instr;
    output.write_str("If (")?;
    exp.print(output)?;
    output.write_char(')')?;
    iter_exps.as_slice().print(output)?;
    output.write_str(", then\n\n")?;
    write_block(output, block, level + 1)
}

fn write_hold_instr(
    output: &mut Printer<'_>,
    instr: &HoldInstr,
    level: usize,
    order: &str,
) -> fmt::Result {
    let HoldInstr {
        id,
        not_exp,
        iter_exps,
        block_hold,
        block_not_hold,
    } = instr;
    output.write_str("If (")?;
    id.print(output)?;
    output.write_str(": ")?;
    not_exp.print(output)?;
    output.write_char(')')?;
    iter_exps.as_slice().print(output)?;
    output.write_str(" holds, then\n\n")?;
    write_block(output, block_hold, level + 1)?;
    write!(output, "\n\n{order}Else,\n\n")?;
    write_block(output, block_not_hold, level + 1)
}

fn write_case_instr(output: &mut Printer<'_>, instr: &CaseInstr, level: usize) -> fmt::Result {
    let CaseInstr { exp, cases, .. } = instr;
    output.write_str("Case analysis on ")?;
    exp.print(output)?;
    output.write_str("\n\n")?;
    for (index, case) in cases.iter().enumerate() {
        if index != 0 {
            output.write_str("\n\n")?;
        }
        write_case(output, case, level + 1, index + 1)?;
    }
    Ok(())
}

fn write_group_instr(output: &mut Printer<'_>, instr: &GroupInstr, level: usize) -> fmt::Result {
    let GroupInstr {
        id,
        rel_signature,
        exps,
        block,
    } = instr;
    output.write_str("Group ")?;
    id.print(output)?;
    output.write_str(": ")?;
    write_relinput(output, rel_signature, exps)?;
    output.write_str("\n\n")?;
    write_block(output, block, level + 1)
}

fn write_let_instr(output: &mut Printer<'_>, instr: &LetInstr, level: usize) -> fmt::Result {
    let LetInstr {
        exp_l,
        exp_r,
        iter_instrs,
        block,
    } = instr;
    output.write_str("(Let ")?;
    exp_l.print(output)?;
    output.write_str(" be ")?;
    exp_r.print(output)?;
    output.write_char(')')?;
    iter_instrs.as_slice().print(output)?;
    output.write_str("\n\n")?;
    write_block(output, block, level + 1)
}

fn write_rule_instr(output: &mut Printer<'_>, instr: &RuleInstr, level: usize) -> fmt::Result {
    let RuleInstr {
        id,
        not_exp,
        iter_instrs,
        block,
        ..
    } = instr;
    output.write_char('(')?;
    id.print(output)?;
    output.write_str(": ")?;
    not_exp.print(output)?;
    output.write_char(')')?;
    iter_instrs.as_slice().print(output)?;
    output.write_str("\n\n")?;
    write_block(output, block, level + 1)
}

fn write_result_instr(output: &mut Printer<'_>, instr: &ResultInstr) -> fmt::Result {
    let ResultInstr {
        rel_signature,
        exps,
    } = instr;
    if exps.is_empty() {
        return output.write_str("The relation holds");
    }
    output.write_str("Result in ")?;
    write_reloutput(output, rel_signature, exps)
}

fn write_return_instr(output: &mut Printer<'_>, instr: &ReturnInstr) -> fmt::Result {
    output.write_str("Return ")?;
    instr.exp.print(output)
}

fn write_debug_instr(output: &mut Printer<'_>, instr: &DebugInstr) -> fmt::Result {
    let DebugInstr { exp, instr } = instr;
    output.write_str("Debug: ")?;
    exp.print(output)?;
    output.write_char('\n')?;
    write_instr(output, instr, 0, 0)
}

fn write_block(output: &mut Printer<'_>, block: &Block, level: usize) -> fmt::Result {
    for (index, instr) in block.iter().enumerate() {
        if index != 0 {
            output.write_str("\n\n")?;
        }
        write_instr(output, instr, level, index + 1)?;
    }
    Ok(())
}

impl Print for Case {
    fn print(&self, output: &mut Printer<'_>) -> fmt::Result {
        write_case(output, self, 0, 0)
    }
}
impl Print for Instr {
    fn print(&self, output: &mut Printer<'_>) -> fmt::Result {
        write_instr(output, self, 0, 0)
    }
}
impl Print for Block {
    fn print(&self, output: &mut Printer<'_>) -> fmt::Result {
        write_block(output, self, 0)
    }
}

fn write_relinput(
    output: &mut Printer<'_>,
    rel_signature: &RelSignature,
    exps_input: &[Exp],
) -> fmt::Result {
    let not_typ = &rel_signature.not_typ;
    let indices_input = rel_signature.input_hint.indices();
    assert_eq!(indices_input.len(), exps_input.len());
    let args = (0..not_typ.node.arity()).map(|index| {
        indices_input
            .iter()
            .position(|index_input| *index_input == index as i64)
            .map(|index_exp| &exps_input[index_exp])
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
    let indices_input = rel_signature.input_hint.indices();
    let indices_output = (0..not_typ.node.arity())
        .filter(|index| !indices_input.contains(&(*index as i64)))
        .collect::<Vec<_>>();
    assert_eq!(indices_output.len(), exps_output.len());
    let args = (0..not_typ.node.arity()).map(|index| {
        indices_output
            .iter()
            .position(|index_output| *index_output == index)
            .map(|index_exp| &exps_output[index_exp])
    });
    let mixfix = Mixop::fill(&not_typ.node.to_mixop(), args)
        .expect("relation output arity matches notation");
    mixfix.print_with(output, |exp, output| match exp {
        Some(exp) => exp.print(output),
        None => output.write("%"),
    })
}
