//! AsciiDoc source blocks for elaboration-language definitions
//!
//! Rendering follows the original EL backend rather than the language printer.
//! Types use display atoms so notation brackets and operators read naturally;
//! executable syntax uses source atoms so it remains unambiguous. Documents
//! choose flat or broken layouts at 80 columns.
//! `render_def` is the entry point.

use crate::lang::{
    common::{Iter, notation::atom::Atom as AtomKind, prim::num},
    el::ast::*,
    traits::print::Print,
};

use super::doc::{self, Doc};

// == Documents

const WIDTH: usize = 80;

#[derive(Clone, Copy)]
enum AtomMode {
    Source,
    Display,
}

fn text(text: impl Into<String>) -> Doc {
    doc::text(text)
}

fn space() -> Doc {
    text(" ")
}

// == Iterators

fn doc_of_iter(iter: Iter) -> Doc {
    text(match iter {
        Iter::Opt => "?",
        Iter::List => "*",
    })
}

// == Identifiers

fn doc_of_id(id: &Id) -> Doc {
    text(id.node.clone())
}

fn doc_of_defid(id: &Id) -> Doc {
    text(format!("${}", id.node))
}

fn doc_of_rule_suffix(id_suffix: &Id) -> Doc {
    if id_suffix.node.is_empty() { doc::empty() } else { text(format!("/{}", id_suffix.node)) }
}

// == Atoms

fn string_of_display_atom(atom: &AtomKind) -> String {
    match atom {
        AtomKind::Keyword(id) => id.clone(),
        AtomKind::Tag(id) if id == "EMPTY" => "/* empty */".to_owned(),
        AtomKind::Tag(id) => format!("_{id}"),
        AtomKind::Operator(op) => op.clone(),
        AtomKind::Sub => "<:".to_owned(),
        AtomKind::Sup => ":>".to_owned(),
        AtomKind::Turnstile => "|-".to_owned(),
        AtomKind::Tilesturn => "-|".to_owned(),
        AtomKind::Arrow => "->".to_owned(),
        AtomKind::ArrowSub => "->_".to_owned(),
        AtomKind::DoubleArrowSub => "=>_".to_owned(),
        AtomKind::DoubleArrowLong => "==>".to_owned(),
        AtomKind::SqArrow => "~>".to_owned(),
        AtomKind::SqArrowStar => "~>*".to_owned(),
        AtomKind::Dot => ".".to_owned(),
        AtomKind::Dot2 => "..".to_owned(),
        AtomKind::Dot3 => "...".to_owned(),
        AtomKind::Semicolon => ";".to_owned(),
        AtomKind::Colon => ":".to_owned(),
        AtomKind::ColonEq => ":=".to_owned(),
        AtomKind::Tilde2 => "~~".to_owned(),
        AtomKind::Backslash => "\\".to_owned(),
        AtomKind::LAngle => "<".to_owned(),
        AtomKind::RAngle => ">".to_owned(),
        AtomKind::LParen => "(".to_owned(),
        AtomKind::RParen => ")".to_owned(),
        AtomKind::LBrack => "[".to_owned(),
        AtomKind::RBrack => "]".to_owned(),
        AtomKind::LBrace => "{".to_owned(),
        AtomKind::RBrace => "}".to_owned(),
    }
}

fn string_of_atom(atom_mode: AtomMode, atom: &Atom) -> String {
    match atom_mode {
        AtomMode::Source => Print::to_string(&atom.node),
        AtomMode::Display => string_of_display_atom(&atom.node),
    }
}

fn doc_of_atom(atom_mode: AtomMode, atom: &Atom) -> Doc {
    text(string_of_atom(atom_mode, atom))
}

// == Lists

/// Groups a comma-separated list, indenting its elements when it breaks.
fn doc_of_comma_list<T>(
    indent: usize,
    open: &str,
    close: &str,
    doc_of_item: impl Fn(&T) -> Doc,
    items: &[T],
) -> Doc {
    // Empty lists retain their delimiters without a break
    if items.is_empty() {
        return text(format!("{open}{close}"));
    }

    doc::group(doc::concat([
        text(open),
        doc::nest(
            indent,
            doc::cat(
                doc::break_(""),
                doc::join(doc::cat(text(","), doc::break_(" ")), items.iter().map(doc_of_item)),
            ),
        ),
        text(close),
    ]))
}

/// Omits empty lists, including their opening and closing delimiters.
fn doc_of_optional_comma_list<T>(
    indent: usize,
    open: &str,
    close: &str,
    doc_of_item: impl Fn(&T) -> Doc,
    items: &[T],
) -> Doc {
    if items.is_empty() {
        doc::empty()
    } else {
        doc_of_comma_list(indent, open, close, doc_of_item, items)
    }
}

// == Operators

/// Groups display brackets with breakable spaces around the inner document.
fn doc_of_bracket(atom_mode: AtomMode, doc_body: Doc, atom_l: &Atom, atom_r: &Atom) -> Doc {
    doc::group(doc::concat([
        doc_of_atom(atom_mode, atom_l),
        doc::nest(2, doc::cat(doc::break_(" "), doc_body)),
        doc::break_(" "),
        doc_of_atom(atom_mode, atom_r),
    ]))
}

fn doc_of_infix(doc_l: Doc, op: impl Into<String>, doc_r: Doc) -> Doc {
    doc::group(doc::cat(
        doc_l,
        doc::nest(4, doc::concat([doc::break_(" "), text(op), space(), doc_r])),
    ))
}

fn string_of_unop(op: &UnOp) -> String {
    match op {
        UnOp::Bool(op) => Print::to_string(op),
        UnOp::Num(op) => Print::to_string(op),
    }
}

fn string_of_binop(op: &BinOp) -> String {
    match op {
        BinOp::Bool(op) => Print::to_string(op),
        BinOp::Num(op) => Print::to_string(op),
    }
}

fn string_of_cmpop(op: &CmpOp) -> String {
    match op {
        CmpOp::Bool(op) => Print::to_string(op),
        CmpOp::Num(op) => Print::to_string(op),
    }
}

// == Types

fn doc_of_typ(atom_mode: AtomMode, typ: &Typ) -> Doc {
    match typ {
        Typ::Plain(plain_typ) => doc_of_plain_typ(atom_mode, plain_typ),
        Typ::Notation(not_typ) => doc_of_not_typ(atom_mode, not_typ),
    }
}

fn doc_of_plain_typ(atom_mode: AtomMode, plain_typ: &PlainTyp) -> Doc {
    match &plain_typ.node {
        PlainTypKind::Bool => text("bool"),
        PlainTypKind::Num(typ) => text(Print::to_string(typ)),
        PlainTypKind::Text => text("text"),
        PlainTypKind::Var(id_typ, targs) => {
            doc::cat(doc_of_id(id_typ), doc_of_targs(atom_mode, targs))
        }
        PlainTypKind::Paren(plain_typ) => doc_of_comma_list(
            2,
            "(",
            ")",
            |typ| doc_of_plain_typ(atom_mode, typ),
            std::slice::from_ref(plain_typ),
        ),
        PlainTypKind::Tuple(plain_typs) => {
            doc_of_comma_list(2, "(", ")", |typ| doc_of_plain_typ(atom_mode, typ), plain_typs)
        }
        PlainTypKind::Iter(plain_typ, iter) => {
            doc::cat(doc_of_plain_typ(atom_mode, plain_typ), doc_of_iter(*iter))
        }
    }
}

fn doc_of_not_typ(atom_mode: AtomMode, not_typ: &NotTyp) -> Doc {
    match &not_typ.node {
        NotTypKind::Atom(atom) => doc_of_atom(atom_mode, atom),
        NotTypKind::Seq(typs) => doc::flow(typs.iter().map(|typ| doc_of_typ(atom_mode, typ))),
        NotTypKind::Infix(typ_l, atom, typ_r) => doc_of_infix(
            doc_of_typ(atom_mode, typ_l),
            string_of_atom(atom_mode, atom),
            doc_of_typ(atom_mode, typ_r),
        ),
        NotTypKind::Brack(atom_l, typ, atom_r) => {
            doc_of_bracket(atom_mode, doc_of_typ(atom_mode, typ), atom_l, atom_r)
        }
    }
}

fn doc_of_targ(atom_mode: AtomMode, targ: &Targ) -> Doc {
    doc_of_plain_typ(atom_mode, targ)
}

fn doc_of_targs(atom_mode: AtomMode, targs: &[Targ]) -> Doc {
    doc_of_optional_comma_list(2, "<", ">", |targ| doc_of_targ(atom_mode, targ), targs)
}

fn doc_of_typ_field(atom_mode: AtomMode, typ_field: &TypField) -> Doc {
    doc::group(doc::concat([
        doc_of_atom(atom_mode, &typ_field.atom),
        space(),
        doc_of_plain_typ(atom_mode, &typ_field.typ),
    ]))
}

fn doc_of_typ_case(atom_mode: AtomMode, typ_case: &TypCase) -> Doc {
    doc::nest(4, doc_of_typ(atom_mode, &typ_case.typ))
}

/// Renders aliases, structs, and variants with their distinct line structure.
fn doc_of_def_typ(atom_mode: AtomMode, def_typ: &DefTyp) -> Doc {
    match &def_typ.node {
        DefTypKind::Plain(plain_typ) => {
            doc::cat(text(" = "), doc_of_plain_typ(atom_mode, plain_typ))
        }
        DefTypKind::Struct(typ_fields) if typ_fields.is_empty() => text(" = {}"),
        DefTypKind::Struct(typ_fields) => doc::concat([
            text(" = {"),
            doc::nest(
                2,
                doc::cat(
                    doc::line(),
                    doc::join(
                        doc::cat(text(","), doc::line()),
                        typ_fields
                            .iter()
                            .map(|field| doc_of_typ_field(atom_mode, field)),
                    ),
                ),
            ),
            doc::line(),
            text("}"),
        ]),
        DefTypKind::Variant(typ_cases) if typ_cases.is_empty() => {
            doc::cat(doc::line(), doc::nest(4, doc::concat([text(":"), doc::line(), text(";")])))
        }
        DefTypKind::Variant(typ_cases) => {
            let (typ_case_head, typ_cases_tail) = typ_cases.split_first().expect("nonempty cases");
            doc::nest(
                4,
                doc::concat([
                    doc::line(),
                    text(": "),
                    doc_of_typ_case(atom_mode, typ_case_head),
                    doc::concat(typ_cases_tail.iter().map(|typ_case| {
                        doc::group(doc::concat([
                            doc::break_(" "),
                            text("| "),
                            doc_of_typ_case(atom_mode, typ_case),
                        ]))
                    })),
                    doc::line(),
                    text(";"),
                ]),
            )
        }
    }
}

// == Expressions

/// Renders every EL expression form using the requested atom spelling.
fn doc_of_exp(atom_mode: AtomMode, exp: &Exp) -> Doc {
    match &exp.node {
        ExpKind::Bool(value) => text(value.to_string()),
        ExpKind::Num(NumOp::Dec, num::Number::Nat(nat)) => text(nat.to_string()),
        ExpKind::Num(NumOp::Hex, num::Number::Nat(nat)) => {
            text(format!("0x{}", nat.as_bigint().to_str_radix(16).to_uppercase()))
        }
        ExpKind::Num(_, num) => text(Print::to_string(num)),
        ExpKind::Text(text_value) => text(format!("\"{}\"", escaped(text_value))),
        ExpKind::Id(id_var) => doc_of_id(id_var),
        ExpKind::Un(op, exp_inner) => {
            doc::cat(text(string_of_unop(op)), doc_of_exp(atom_mode, exp_inner))
        }
        ExpKind::Bin(exp_l, op, exp_r) => doc_of_infix(
            doc_of_exp(atom_mode, exp_l),
            string_of_binop(op),
            doc_of_exp(atom_mode, exp_r),
        ),
        ExpKind::Cmp(exp_l, op, exp_r) => doc_of_infix(
            doc_of_exp(atom_mode, exp_l),
            string_of_cmpop(op),
            doc_of_exp(atom_mode, exp_r),
        ),
        ExpKind::Arith(exp_inner) => doc::group(doc::concat([
            text("$("),
            doc::nest(2, doc::cat(doc::break_(""), doc_of_exp(atom_mode, exp_inner))),
            text(")"),
        ])),
        ExpKind::Eps => text("eps"),
        ExpKind::List(exps) => {
            doc_of_comma_list(2, "[", "]", |exp| doc_of_exp(atom_mode, exp), exps)
        }
        ExpKind::Cons(exp_l, exp_r) => {
            doc_of_infix(doc_of_exp(atom_mode, exp_l), "::", doc_of_exp(atom_mode, exp_r))
        }
        ExpKind::Cat(exp_l, exp_r) => {
            doc_of_infix(doc_of_exp(atom_mode, exp_l), "++", doc_of_exp(atom_mode, exp_r))
        }
        ExpKind::Idx(exp_base, exp_idx) => doc::cat(
            doc_of_exp(atom_mode, exp_base),
            doc::group(doc::concat([
                text("["),
                doc::nest(2, doc::cat(doc::break_(""), doc_of_exp(atom_mode, exp_idx))),
                text("]"),
            ])),
        ),
        ExpKind::Slice(exp_base, exp_idx, exp_len) => doc::cat(
            doc_of_exp(atom_mode, exp_base),
            doc::group(doc::concat([
                text("["),
                doc::nest(
                    2,
                    doc::cat(
                        doc::break_(""),
                        doc_of_infix(
                            doc_of_exp(atom_mode, exp_idx),
                            ":",
                            doc_of_exp(atom_mode, exp_len),
                        ),
                    ),
                ),
                text("]"),
            ])),
        ),
        ExpKind::Len(exp_inner) => {
            doc::concat([text("|"), doc_of_exp(atom_mode, exp_inner), text("|")])
        }
        ExpKind::Mem(exp_l, exp_r) => {
            doc_of_infix(doc_of_exp(atom_mode, exp_l), "<-", doc_of_exp(atom_mode, exp_r))
        }
        ExpKind::Str(fields) if fields.is_empty() => text("{}"),
        ExpKind::Str(fields) => doc::group(doc::concat([
            text("{"),
            doc::nest(
                2,
                doc::cat(
                    doc::break_(""),
                    doc::join(
                        doc::cat(text(","), doc::break_(" ")),
                        fields.iter().map(|(atom, exp_field)| {
                            doc::concat([
                                doc_of_atom(atom_mode, atom),
                                space(),
                                doc_of_exp(atom_mode, exp_field),
                            ])
                        }),
                    ),
                ),
            ),
            doc::break_(""),
            text("}"),
        ])),
        ExpKind::Dot(exp_base, atom) => {
            doc::concat([doc_of_exp(atom_mode, exp_base), text("."), doc_of_atom(atom_mode, atom)])
        }
        ExpKind::Upd(exp_base, path, exp_field) => doc::cat(
            doc_of_exp(atom_mode, exp_base),
            doc::group(doc::concat([
                text("["),
                doc::nest(
                    2,
                    doc::concat([
                        doc::break_(""),
                        doc_of_path(atom_mode, path),
                        text(" = "),
                        doc_of_exp(atom_mode, exp_field),
                    ]),
                ),
                text("]"),
            ])),
        ),
        ExpKind::Paren(exp_inner) => doc::group(doc::concat([
            text("("),
            doc::nest(2, doc::cat(doc::break_(""), doc_of_exp(atom_mode, exp_inner))),
            text(")"),
        ])),
        ExpKind::Tuple(exps) => {
            doc_of_comma_list(2, "(", ")", |exp| doc_of_exp(atom_mode, exp), exps)
        }
        ExpKind::Call(id_def, targs, args) => doc::group(doc::concat([
            doc_of_defid(id_def),
            doc_of_targs(atom_mode, targs),
            doc_of_args(atom_mode, args),
        ])),
        ExpKind::Iter(exp_inner, iter) => {
            doc::cat(doc_of_exp(atom_mode, exp_inner), doc_of_iter(*iter))
        }
        ExpKind::Sub(exp_inner, plain_typ) => doc_of_infix(
            doc_of_exp(atom_mode, exp_inner),
            "<:",
            doc_of_plain_typ(atom_mode, plain_typ),
        ),
        ExpKind::Atom(atom) => doc_of_atom(atom_mode, atom),
        ExpKind::Seq(exps) => doc::flow(exps.iter().map(|exp| doc_of_exp(atom_mode, exp))),
        ExpKind::Infix(exp_l, atom, exp_r) => doc_of_infix(
            doc_of_exp(atom_mode, exp_l),
            string_of_atom(atom_mode, atom),
            doc_of_exp(atom_mode, exp_r),
        ),
        ExpKind::Brack(atom_l, exp_inner, atom_r) => {
            doc_of_bracket(atom_mode, doc_of_exp(atom_mode, exp_inner), atom_l, atom_r)
        }
        ExpKind::Hole(Hole::Num(num)) => text(format!("%{num}")),
        ExpKind::Hole(Hole::Next) => text("%"),
        ExpKind::Hole(Hole::Rest) => text("%%"),
        ExpKind::Hole(Hole::None) => text("!%"),
        ExpKind::Fuse(exp_l, _, exp_r) => {
            doc::concat([doc_of_exp(atom_mode, exp_l), text("#"), doc_of_exp(atom_mode, exp_r)])
        }
        ExpKind::Unparen(exp_inner) => doc::cat(text("##"), doc_of_exp(atom_mode, exp_inner)),
        ExpKind::Latex(text_value) => text(format!("latex(\"{}\")", escaped(text_value))),
    }
}

// == Paths

fn doc_of_path(atom_mode: AtomMode, path: &Path) -> Doc {
    match &path.node {
        PathKind::Root => doc::empty(),
        PathKind::Idx(path_base, exp_idx) => doc::concat([
            doc_of_path(atom_mode, path_base),
            text("["),
            doc_of_exp(atom_mode, exp_idx),
            text("]"),
        ]),
        PathKind::Slice(path_base, exp_idx, exp_len) => doc::concat([
            doc_of_path(atom_mode, path_base),
            text("["),
            doc_of_exp(atom_mode, exp_idx),
            text(" : "),
            doc_of_exp(atom_mode, exp_len),
            text("]"),
        ]),
        PathKind::Dot(path_base, atom) if matches!(path_base.node, PathKind::Root) => {
            doc_of_atom(atom_mode, atom)
        }
        PathKind::Dot(path_base, atom) => doc::concat([
            doc_of_path(atom_mode, path_base),
            text("."),
            doc_of_atom(atom_mode, atom),
        ]),
    }
}

// == Arguments

fn doc_of_arg(atom_mode: AtomMode, arg: &Arg) -> Doc {
    match &arg.node {
        ArgKind::Exp(exp) => doc_of_exp(atom_mode, exp),
        ArgKind::Def(id_def) => doc::cat(text("def "), doc_of_defid(id_def)),
    }
}

fn doc_of_args(atom_mode: AtomMode, args: &[Arg]) -> Doc {
    doc_of_comma_list(4, "(", ")", |arg| doc_of_arg(atom_mode, arg), args)
}

// == Parameters

fn doc_of_param(atom_mode: AtomMode, param: &Param) -> Doc {
    match &param.node {
        ParamKind::Exp(plain_typ) => doc_of_plain_typ(atom_mode, plain_typ),
        ParamKind::Def(id_def, tparams, params, plain_typ) => doc::group(doc::concat([
            text("def "),
            doc_of_defid(id_def),
            doc_of_tparams(tparams),
            doc_of_params(atom_mode, params),
            text(" : "),
            doc_of_plain_typ(atom_mode, plain_typ),
        ])),
    }
}

fn doc_of_params(atom_mode: AtomMode, params: &[Param]) -> Doc {
    doc_of_optional_comma_list(4, "(", ")", |param| doc_of_param(atom_mode, param), params)
}

// == Type parameters

fn doc_of_tparams(tparams: &[TParam]) -> Doc {
    doc_of_optional_comma_list(2, "<", ">", doc_of_id, tparams)
}

// == Premises

fn doc_of_rel_prem(atom_mode: AtomMode, marker: &str, id_rel: &Id, exp: &Exp) -> Doc {
    doc::group(doc::concat([
        doc_of_id(id_rel),
        text(marker),
        doc::nest(4, doc::cat(doc::break_(" "), doc_of_exp(atom_mode, exp))),
    ]))
}

fn doc_of_prem(atom_mode: AtomMode, prem: &Prem) -> Doc {
    match &prem.node {
        PremKind::Var(VarPrem { id, plain_typ }) => {
            doc::concat([doc_of_id(id), text(" : "), doc_of_plain_typ(atom_mode, plain_typ)])
        }
        PremKind::Rule(RulePrem { id, exp }) => doc_of_rel_prem(atom_mode, ":", id, exp),
        PremKind::RuleNot(RuleNotPrem { id, exp }) => doc_of_rel_prem(atom_mode, ":/", id, exp),
        PremKind::If(IfPrem { exp }) => doc::cat(text("if "), doc_of_exp(atom_mode, exp)),
        PremKind::Else => text("otherwise"),
        PremKind::Iter(IterPrem { prem: prem_inner, iter })
            if matches!(prem_inner.node, PremKind::Iter(_)) =>
        {
            doc::cat(doc_of_prem(atom_mode, prem_inner), doc_of_iter(*iter))
        }
        PremKind::Iter(IterPrem { prem: prem_inner, iter }) => doc::concat([
            text("("),
            doc_of_prem(atom_mode, prem_inner),
            text(")"),
            doc_of_iter(*iter),
        ]),
        PremKind::Debug(DebugPrem { exp }) => doc::cat(text("debug "), doc_of_exp(atom_mode, exp)),
    }
}

fn doc_of_prems(atom_mode: AtomMode, prems: &[Prem]) -> Doc {
    doc::concat(
        prems
            .iter()
            .map(|prem| doc::concat([doc::line(), text("-- "), doc_of_prem(atom_mode, prem)])),
    )
}

// == Rules

/// Places the conclusion and premises below the grouped rule heading.
fn doc_of_rule(atom_mode: AtomMode, rule: &Rule) -> Doc {
    doc::cat(
        doc::group(doc::concat([
            text("rule"),
            doc::nest(
                2,
                doc::concat([
                    doc::break_(" "),
                    doc_of_id(&rule.node.id_rel),
                    doc_of_rule_suffix(&rule.node.id_rule),
                ]),
            ),
            text(":"),
        ])),
        doc::nest(
            2,
            doc::concat([
                doc::line(),
                doc_of_exp(atom_mode, &rule.node.exp),
                doc_of_prems(atom_mode, &rule.node.prems),
            ]),
        ),
    )
}

// == Tables

/// Keeps the pattern and result together when the row fits.
fn doc_of_table_row(atom_mode: AtomMode, row: &TableRow) -> Doc {
    doc::group(doc::concat([
        text("| "),
        doc_of_exp(atom_mode, &row.node.exp_pattern),
        doc::nest(
            4,
            doc::concat([doc::break_(" "), text("=> "), doc_of_exp(atom_mode, &row.node.exp_body)]),
        ),
    ]))
}

// == Functions

/// Groups a source signature with its breakable result type.
fn doc_of_func_dec(
    prefix: &str,
    id_def: &Id,
    tparams: &[TParam],
    params: &[Param],
    plain_typ: &PlainTyp,
) -> Doc {
    doc::group(doc::concat([
        text(prefix),
        doc_of_defid(id_def),
        doc_of_tparams(tparams),
        doc_of_params(AtomMode::Source, params),
        doc::nest(
            2,
            doc::concat([
                doc::break_(" "),
                text(": "),
                doc_of_plain_typ(AtomMode::Source, plain_typ),
            ]),
        ),
    ]))
}

// == Definitions

/// Builds the width-sensitive source block for one EL definition.
fn doc_of_def(def: &Def) -> Doc {
    match &def.node {
        DefKind::ExternSyntax(ExternSyntaxDef { id, .. }) => {
            doc::cat(text("extern syntax "), doc_of_id(id))
        }
        DefKind::Syntax(SyntaxDef { entries }) => doc::cat(
            text("syntax "),
            doc::group(doc::join(
                doc::cat(text(","), doc::break_(" ")),
                entries
                    .iter()
                    .map(|entry| doc::cat(doc_of_id(&entry.id), doc_of_tparams(&entry.tparams))),
            )),
        ),
        DefKind::Typ(TypDef { id, tparams, def_typ, .. }) => doc::concat([
            doc_of_id(id),
            doc_of_tparams(tparams),
            doc_of_def_typ(AtomMode::Display, def_typ),
        ]),
        DefKind::Var(VarDef { id, plain_typ, .. }) => doc::group(doc::concat([
            text("var "),
            doc_of_id(id),
            doc::nest(
                2,
                doc::concat([
                    doc::break_(" "),
                    text(": "),
                    doc_of_plain_typ(AtomMode::Source, plain_typ),
                ]),
            ),
        ])),
        DefKind::ExternRel(ExternRelDef { id, not_typ, .. }) => doc::concat([
            text("extern relation "),
            doc_of_id(id),
            text(":"),
            doc::nest(2, doc::cat(doc::line(), doc_of_not_typ(AtomMode::Source, not_typ))),
        ]),
        DefKind::Rel(RelDef { id, not_typ, .. }) => doc::concat([
            text("relation "),
            doc_of_id(id),
            text(":"),
            doc::nest(2, doc::cat(doc::line(), doc_of_not_typ(AtomMode::Source, not_typ))),
        ]),
        DefKind::RuleGroup(RuleGroupDef { rules, .. }) if rules.len() == 1 => {
            doc_of_rule(AtomMode::Source, &rules[0])
        }
        DefKind::RuleGroup(RuleGroupDef { relid, groupid, rules }) => doc::concat([
            text("rulegroup "),
            doc_of_id(relid),
            doc_of_rule_suffix(groupid),
            text(" {"),
            doc::concat(rules.iter().map(|rule| {
                doc::cat(
                    doc::line(),
                    doc::nest(2, doc::cat(doc::line(), doc_of_rule(AtomMode::Source, rule))),
                )
            })),
            doc::line(),
            doc::line(),
            text("}"),
        ]),
        DefKind::ExternDec(ExternDecDef { id, tparams, params, plain_typ, .. }) => {
            doc_of_func_dec("extern dec ", id, tparams, params, plain_typ)
        }
        DefKind::BuiltinDec(BuiltinDecDef { id, tparams, params, plain_typ, .. }) => {
            doc_of_func_dec("builtin dec ", id, tparams, params, plain_typ)
        }
        DefKind::TableDec(TableDecDef { id, params, plain_typ, .. }) => {
            doc_of_func_dec("tbl dec ", id, &[], params, plain_typ)
        }
        DefKind::FuncDec(FuncDecDef { id, tparams, params, plain_typ, .. }) => {
            doc_of_func_dec("dec ", id, tparams, params, plain_typ)
        }
        DefKind::TableDef(TableDef { id, rows }) => doc::concat([
            text("tbl def "),
            doc_of_defid(id),
            text(" ="),
            doc::nest(
                2,
                doc::concat(
                    rows.iter()
                        .map(|row| doc::cat(doc::line(), doc_of_table_row(AtomMode::Source, row))),
                ),
            ),
        ]),
        DefKind::FuncDef(FuncDef { id, tparams, args, exp, prems }) => doc::cat(
            doc::group(doc::concat([
                text("def "),
                doc_of_defid(id),
                doc_of_tparams(tparams),
                doc_of_args(AtomMode::Source, args),
                doc::nest(
                    2,
                    doc::concat([doc::break_(" "), text("= "), doc_of_exp(AtomMode::Source, exp)]),
                ),
            ])),
            doc::nest(2, doc_of_prems(AtomMode::Source, prems)),
        ),
        DefKind::Sep => doc::cat(doc::line(), doc::line()),
    }
}

// == Rendering

/// Renders one elaboration-language definition as an AsciiDoc source block.
pub fn render_def(def: &Def) -> String {
    doc::render(WIDTH, &doc_of_def(def))
}

// == Text escaping

/// Escapes quotes, backslashes, control bytes, and non-ASCII text bytes.
fn escaped(text_value: &str) -> String {
    text_value
        .bytes()
        .map(|byte| match byte {
            b'"' => "\\\"".to_owned(),
            b'\\' => "\\\\".to_owned(),
            8 => "\\b".to_owned(),
            9 => "\\t".to_owned(),
            10 => "\\n".to_owned(),
            13 => "\\r".to_owned(),
            32..=126 => char::from(byte).to_string(),
            _ => format!("\\{byte:03}"),
        })
        .collect()
}
