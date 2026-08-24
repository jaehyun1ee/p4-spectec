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
    match &exp.kind {
        ExpKind::BoolE(value) => value.to_string(),
        ExpKind::NumE(value) => string_of_num(value),
        ExpKind::TextE(text) => format!("\"{}\"", escaped(text)),
        ExpKind::VarE(id) => string_of_varid(id),
        ExpKind::UnE(operation, _, exp) => {
            format!("{}{}", string_of_unop(*operation), string_of_exp(exp))
        }
        ExpKind::BinE(operation, _, exp_l, exp_r) => format!(
            "({} {} {})",
            string_of_exp(exp_l),
            string_of_binop(*operation),
            string_of_exp(exp_r)
        ),
        ExpKind::CmpE(operation, _, exp_l, exp_r) => format!(
            "({} {} {})",
            string_of_exp(exp_l),
            string_of_cmpop(*operation),
            string_of_exp(exp_r)
        ),
        ExpKind::UpCastE(typ, exp) | ExpKind::DownCastE(typ, exp) => {
            format!("({} as {})", string_of_exp(exp), string_of_typ(typ))
        }
        ExpKind::SubE(exp, typ, _) => {
            format!("({} has type {})", string_of_exp(exp), string_of_typ(typ))
        }
        ExpKind::MatchE(exp, pattern) => format!(
            "({} matches pattern {})",
            string_of_exp(exp),
            string_of_pattern(pattern)
        ),
        ExpKind::TupleE(exps) => format!("({})", string_of_exps(", ", exps)),
        ExpKind::CaseE(notexp) => format!("({})", string_of_notexp(notexp)),
        ExpKind::StrE(fields) => format!(
            "{{{}}}",
            join(fields, ", ", |(atom, exp)| format!(
                "{} {}",
                string_of_atom(atom),
                string_of_exp(exp)
            ))
        ),
        ExpKind::OptE(Some(exp)) => format!("?({})", string_of_exp(exp)),
        ExpKind::OptE(None) => "?()".into(),
        ExpKind::ListE(exps) => format!("[{}]", string_of_exps(", ", exps)),
        ExpKind::ConsE(exp_h, exp_t) => {
            format!("{} :: {}", string_of_exp(exp_h), string_of_exp(exp_t))
        }
        ExpKind::CatE(exp_l, exp_r) => {
            format!("{} ++ {}", string_of_exp(exp_l), string_of_exp(exp_r))
        }
        ExpKind::MemE(exp_e, exp_s) => {
            format!("{} is in {}", string_of_exp(exp_e), string_of_exp(exp_s))
        }
        ExpKind::LenE(exp) => format!("|{}|", string_of_exp(exp)),
        ExpKind::DotE(exp, atom) => format!("{}.{}", string_of_exp(exp), string_of_atom(atom)),
        ExpKind::IdxE(exp_b, exp_i) => {
            format!("{}[{}]", string_of_exp(exp_b), string_of_exp(exp_i))
        }
        ExpKind::SliceE(exp_b, exp_l, exp_h) => format!(
            "{}[{} : {}]",
            string_of_exp(exp_b),
            string_of_exp(exp_l),
            string_of_exp(exp_h)
        ),
        ExpKind::UpdE(exp_b, path, exp_f) => format!(
            "{}[{} = {}]",
            string_of_exp(exp_b),
            string_of_path(path),
            string_of_exp(exp_f)
        ),
        ExpKind::CallE(id, targs, args) => format!(
            "{}{}{}",
            string_of_defid(id),
            string_of_targs(targs),
            string_of_args(args)
        ),
        ExpKind::IterE(exp, iterexp) => string_of_iterated(exp, std::slice::from_ref(iterexp)),
    }
}

pub fn string_of_exps(separator: &str, exps: &[Exp]) -> String {
    join(exps, separator, string_of_exp)
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
        string_of_guard(&case.0),
        string_of_block_with(&case.1, level + 1, 0)
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
    }
}

// Instructions

pub fn string_of_instr(instr: &Instr) -> String {
    string_of_instr_with(instr, false, 0, 0)
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
    string_of_block_with(block, 0, 0)
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

pub fn string_of_relinput(signature: &RelSignature, exps_input: &[Exp]) -> String {
    let (nottyp, inputs) = signature;
    let inputs = inputs.indices();
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
    let (nottyp, inputs) = signature;
    let inputs = inputs.indices();
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
    let (id, signature, exps, _) = relation;
    format!(
        "{}: {}",
        string_of_relid(id),
        string_of_relinput(signature, exps)
    )
}

pub fn string_of_defined_rel(relation: &Rel) -> String {
    let (id, signature, exps, block, elseblock, _) = relation;
    format!(
        "{}: {}\n\n{}{}",
        string_of_relid(id),
        string_of_relinput(signature, exps),
        string_of_block(block),
        string_of_elseblock_opt_with(elseblock, 0, block.len())
    )
}

// Functions

pub fn string_of_extern_func(function: &ExternFunc) -> String {
    let (id, tparams, params, _, _) = function;
    format!(
        "{}{}{}",
        string_of_defid(id),
        string_of_tparams(tparams),
        string_of_params(params)
    )
}

pub fn string_of_builtin_func(function: &BuiltinFunc) -> String {
    string_of_extern_func(function)
}

pub fn string_of_tablerow(row: &TableRow) -> String {
    let (exps, result, instrs) = row;
    format!(
        "\n  Row : {} -> {}:\n\n{}",
        string_of_exps(", ", exps),
        string_of_exp(result),
        string_of_block_with(instrs, 2, 0)
    )
}

pub fn string_of_tablerows(rows: &[TableRow]) -> String {
    join(rows, "\n", string_of_tablerow)
}

pub fn string_of_table_func(function: &TableFunc) -> String {
    let (id, params, _, rows, _) = function;
    format!(
        "{}{}\n=\n{}",
        string_of_defid(id),
        string_of_params(params),
        string_of_tablerows(rows)
    )
}

pub fn string_of_defined_func(function: &DefinedFunc) -> String {
    let (id, tparams, params, _, block, elseblock, _) = function;
    format!(
        "{}{}{}\n\n{}{}",
        string_of_defid(id),
        string_of_tparams(tparams),
        string_of_params(params),
        string_of_block(block),
        string_of_elseblock_opt_with(elseblock, 0, block.len())
    )
}

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
        DefKind::ExternRelD(relation) => {
            format!("extern relation {}", string_of_extern_rel(relation))
        }
        DefKind::RelD(relation) => format!("relation {}", string_of_defined_rel(relation)),
        DefKind::ExternDecD(function) => {
            format!("extern def {}", string_of_extern_func(function))
        }
        DefKind::BuiltinDecD(function) => {
            format!("builtin def {}", string_of_builtin_func(function))
        }
        DefKind::TableDecD(function) => {
            format!("tbl def {}", string_of_table_func(function))
        }
        DefKind::FuncDecD(function) => format!("def {}", string_of_defined_func(function)),
    }
}

pub fn string_of_defs(definitions: &[Def]) -> String {
    join(definitions, "\n\n", string_of_def)
}

// Spec

pub fn string_of_spec(spec: &Spec) -> String {
    string_of_defs(spec)
}
