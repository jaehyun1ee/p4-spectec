//! AsciiDoc rendering for prose-language definitions
//!
//! Expressions have code and readable-prose views. Instruction rendering
//! threads fallthrough targets through nested blocks, while one renderer owns
//! all backtracking counters for a complete document. Definition and fragment
//! entry points serialize the resulting document model with caller anchors.

use crate::lang::{
    common::{Iter, notation::mixfix::Mixfix},
    hints::{alter, input},
    pl::{annot::Hints, ast as pl},
    sl,
    traits::print::Print,
};

use super::{
    document::{self, Block, Code, ItemKind, Link, Prose, Subject},
    fallthrough::{self, Anchors, Context},
    utils::{ADOC_WIDTH_SHORT, adoc_subscript, adoc_superscript, reindent_lines, unindent_lines},
};

// == Documents

fn text(text_body: impl Into<String>) -> Prose {
    Prose::Text(text_body.into())
}

fn token(text_body: impl Into<String>) -> Code {
    Code::Token(text_body.into())
}

fn prose(proses: impl IntoIterator<Item = Prose>) -> Prose {
    Prose::Seq(proses.into_iter().collect())
}

fn code(codes: impl IntoIterator<Item = Code>) -> Code {
    Code::Seq(codes.into_iter().collect())
}

fn code_prose(code_body: Code) -> Prose {
    Prose::Code(code_body)
}

fn inline(prose_body: Prose) -> Block {
    Block::Inline(prose_body)
}

fn raw(text_body: impl Into<String>) -> Block {
    Block::Raw(text_body.into())
}

fn concat(blocks_body: impl IntoIterator<Item = Block>) -> Block {
    Block::Concat(blocks_body.into_iter().collect())
}

fn seq(blocks_body: impl IntoIterator<Item = Block>) -> Block {
    Block::Seq(blocks_body.into_iter().collect())
}

fn ordered(level: usize, prose_head: Prose) -> Block {
    Block::Item(level, ItemKind::Ordered(None), prose_head, Box::new(Block::Empty))
}

fn ordered_body(
    level: usize,
    anchor: Option<String>,
    prose_head: Prose,
    block_body: Block,
) -> Block {
    Block::Item(level, ItemKind::Ordered(anchor), prose_head, Box::new(block_body))
}

fn unordered(level: usize, prose_head: Prose) -> Block {
    Block::Item(level, ItemKind::Unordered, prose_head, Box::new(Block::Empty))
}

fn subject_link(subject: Subject, prose_body: Prose) -> Prose {
    Prose::Link(Link::Subject(subject), Box::new(prose_body))
}

fn direct_link(target: impl Into<String>, prose_body: Prose) -> Prose {
    Prose::Link(Link::Direct(target.into()), Box::new(prose_body))
}

fn subject_code(subject: Subject, code_body: Code) -> Code {
    Code::Link(Link::Subject(subject), Box::new(code_body))
}

/// Joins prose with an Oxford comma.
fn prose_of_list(proses: Vec<Prose>) -> Prose {
    match proses.as_slice() {
        [] => Prose::Empty,
        [prose_head] => prose_head.clone(),
        [prose_l, prose_r] => prose([prose_l.clone(), text(" and "), prose_r.clone()]),
        _ => {
            // Separate the final item with an Oxford comma
            let num_proses = proses.len();
            prose(
                proses
                    .into_iter()
                    .enumerate()
                    .flat_map(move |(idx, prose_item)| {
                        if idx == 0 {
                            vec![prose_item]
                        } else if idx + 1 == num_proses {
                            vec![text(", and "), prose_item]
                        } else {
                            vec![text(", "), prose_item]
                        }
                    }),
            )
        }
    }
}

// == Alteration hints

struct AlterRenderer<'a, Item> {
    base_text: &'a dyn Fn(&str) -> String,
    render_item: &'a dyn Fn(&Item) -> Prose,
}

impl<Item> alter::Renderer<Item> for AlterRenderer<'_, Item> {
    type Output = Prose;

    fn empty(&self) -> Self::Output {
        Prose::Empty
    }

    fn text(&self, text_hint: &str) -> Option<Self::Output> {
        (!text_hint.is_empty()).then(|| text((self.base_text)(text_hint)))
    }

    fn atom(&self, atom: &pl::Atom) -> Self::Output {
        code_prose(token(string_of_atom(atom)))
    }

    fn join(&self, proses: Vec<Self::Output>) -> Self::Output {
        prose(
            proses
                .into_iter()
                .enumerate()
                .flat_map(
                    |(idx, prose_item)| {
                        if idx == 0 { vec![prose_item] } else { vec![text(" "), prose_item] }
                    },
                ),
        )
    }

    fn fuse(&self, output_l: Self::Output, output_r: Self::Output) -> Self::Output {
        prose([output_l, output_r])
    }

    fn other(&self, exp: &crate::lang::el::ast::Exp) -> Self::Output {
        text(Print::to_string(exp))
    }

    fn item(&self, item: &Item) -> Self::Output {
        (self.render_item)(item)
    }
}

/// Applies an alteration hint to prose items.
fn alternate<Item>(
    hint: &alter::AlterationHint,
    base_text: &dyn Fn(&str) -> String,
    render_item: &dyn Fn(&Item) -> Prose,
    items: &[Item],
    caps: bool,
) -> Prose {
    let prose_alternated = alter::alternate(hint, items, &AlterRenderer { base_text, render_item })
        .expect("prosify validates alteration hints");
    if caps { document::capitalize_first_prose(prose_alternated) } else { prose_alternated }
}

// == Atoms and identifiers

/// Escapes an atom for an AsciiDoc code span.
fn string_of_atom(atom: &pl::Atom) -> String {
    use crate::lang::common::notation::atom::Atom;

    match &atom.node {
        Atom::Tag(id) => format!("{{nbsp}}{}", adoc_subscript(id)),
        atom_kind => {
            let text_atom = Print::to_string(atom_kind);
            if text_atom.contains('+') {
                text_atom.replace('+', "{plus}").replace('\'', "{apos}")
            } else {
                format!("+{text_atom}+")
            }
        }
    }
}

/// Renders an identifier with its suffix as a subscript.
fn code_of_id(id: &pl::Id) -> Code {
    // Preserve the anonymous identifier literally
    if id.node.starts_with('_') {
        token("++_++")
    } else {
        // Render the base before any underscore suffix
        let mut parts = id.node.split('_');
        let base = parts.next().unwrap_or_default();
        let subscript = parts.collect::<Vec<_>>().join("_");
        token(if subscript.is_empty() {
            base.to_owned()
        } else {
            format!("{base}{}", adoc_subscript(&subscript))
        })
    }
}

fn code_of_var(var: &pl::Var) -> Code {
    code(
        std::iter::once(code_of_id(&var.id))
            .chain(var.iters.iter().map(|iter| token(code_of_iter(*iter)))),
    )
}

fn code_of_iter(iter: Iter) -> String {
    adoc_superscript(match iter {
        Iter::List => "{asterisk}",
        Iter::Opt => "?",
    })
}

fn string_of_iter(iter: Iter) -> &'static str {
    match iter {
        Iter::List => "list",
        Iter::Opt => "option",
    }
}

fn code_of_typ(typ: &pl::Typ) -> Code {
    token(Print::to_string(typ))
}

fn string_of_defid(id: &pl::Id) -> String {
    format!("${}", id.node)
}

/// Escapes a text literal with the language printer's byte rules.
fn escape_text(text_value: &str) -> String {
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

// == Operators

fn string_of_binop(op: pl::BinOp) -> String {
    use crate::lang::common::prim::bool::BinOp;
    match op {
        pl::BinOp::Bool(BinOp::And) => "and".to_owned(),
        pl::BinOp::Bool(BinOp::Or) => "or".to_owned(),
        pl::BinOp::Bool(BinOp::Impl) => "implies".to_owned(),
        pl::BinOp::Bool(BinOp::Equiv) => "is equivalent to".to_owned(),
        _ => Print::to_string(&op),
    }
}

fn string_of_cmpop(op: pl::CmpOp) -> &'static str {
    use crate::lang::common::prim::{bool::CmpOp as BoolCmp, num::CmpOp as NumCmp};
    match op {
        pl::CmpOp::Bool(BoolCmp::Eq) => "is equal to",
        pl::CmpOp::Bool(BoolCmp::Ne) => "is not equal to",
        pl::CmpOp::Num(NumCmp::Lt) => "is less than",
        pl::CmpOp::Num(NumCmp::Gt) => "is greater than",
        pl::CmpOp::Num(NumCmp::Le) => "is less than or equal to",
        pl::CmpOp::Num(NumCmp::Ge) => "is greater than or equal to",
    }
}

// == Expressions as code

/// Renders a mixfix tree with caller-rendered arguments.
fn code_of_mixfix<T>(mixfix: &Mixfix<T>, render_arg: &dyn Fn(&T) -> Code) -> Code {
    match mixfix {
        Mixfix::Arg(arg) => render_arg(arg),
        Mixfix::Atom(atom) => token(string_of_atom(atom)),
        Mixfix::Brack(atom_l, inner, atom_r) => code([
            token(string_of_atom(atom_l)),
            token(" "),
            code_of_mixfix(inner, render_arg),
            token(" "),
            token(string_of_atom(atom_r)),
        ]),
        Mixfix::Infix(mixfix_l, atom, mixfix_r) => code([
            code_of_mixfix(mixfix_l, render_arg),
            token(" "),
            token(string_of_atom(atom)),
            token(" "),
            code_of_mixfix(mixfix_r, render_arg),
        ]),
        Mixfix::Seq(mixfixes) => code(mixfixes.iter().enumerate().flat_map(|(idx, mixfix)| {
            if idx == 0 {
                vec![code_of_mixfix(mixfix, render_arg)]
            } else {
                vec![token(" "), code_of_mixfix(mixfix, render_arg)]
            }
        })),
    }
}

fn code_of_exps(exps: &[pl::Exp], separator: &str) -> Code {
    code(exps.iter().enumerate().flat_map(|(idx, exp)| {
        if idx == 0 { vec![code_of_exp(exp)] } else { vec![token(separator), code_of_exp(exp)] }
    }))
}

/// Renders a pattern in its prose-backend notation.
fn code_of_pattern(pattern: &pl::Pattern) -> Code {
    use crate::lang::il::ast::{ListPattern, OptPattern, Pattern};
    match pattern {
        Pattern::Case(mixop) => code_of_mixfix(mixop, &|()| token("%")),
        Pattern::List(ListPattern::Cons) => token("_ :: _"),
        Pattern::List(ListPattern::Fixed(num_elems)) => token(format!("[ _/{num_elems} ]")),
        Pattern::List(ListPattern::Nil) => token("[]"),
        Pattern::Opt(OptPattern::Some) => token("(_)"),
        Pattern::Opt(OptPattern::None) => token("()"),
    }
}

/// Renders an update path.
fn code_of_path(path: &pl::Path) -> Code {
    match &path.node {
        pl::PathKind::Root => Code::Empty,
        pl::PathKind::Idx(path_base, exp_idx) => {
            code([code_of_path(path_base), token("["), code_of_exp(exp_idx), token("]")])
        }
        pl::PathKind::Slice(path_base, exp_idx, exp_len) => code([
            code_of_path(path_base),
            token("["),
            code_of_exp(exp_idx),
            token(" : "),
            code_of_exp(exp_len),
            token("]"),
        ]),
        pl::PathKind::Dot(path_base, atom) if matches!(path_base.node, pl::PathKind::Root) => {
            token(string_of_atom(atom))
        }
        pl::PathKind::Dot(path_base, atom) => {
            code([code_of_path(path_base), token("."), token(string_of_atom(atom))])
        }
    }
}

fn code_of_arg(arg: &pl::Arg) -> Code {
    match &arg.node {
        pl::ArgKind::Exp(exp) => code_of_exp(exp),
        pl::ArgKind::Def(id) => token(string_of_defid(id)),
    }
}

/// Renders a nonempty argument list with parentheses.
fn code_of_args(args: &[pl::Arg]) -> Code {
    if args.is_empty() {
        Code::Empty
    } else {
        code(
            std::iter::once(token("("))
                .chain(args.iter().enumerate().flat_map(|(idx, arg)| {
                    let arg = code_of_arg(arg);
                    if idx == 0 { vec![arg] } else { vec![token(", "), arg] }
                }))
                .chain(std::iter::once(token(")"))),
        )
    }
}

fn string_of_targs(targs: &[pl::Targ]) -> String {
    if targs.is_empty() {
        String::new()
    } else {
        format!(
            "<{}>",
            targs
                .iter()
                .map(Print::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

/// Renders an expression in compact code form.
fn code_of_exp(exp: &pl::Exp) -> Code {
    use pl::ExpKind;
    match &exp.node.node {
        ExpKind::Bool(value) => token(value.to_string()),
        ExpKind::Num(num) => token(Print::to_string(num)),
        ExpKind::Text(text_value) => token(format!("\"{}\"", escape_text(text_value))),
        ExpKind::Id(id) => code_of_id(id),
        ExpKind::Un(op, _, exp_inner) => {
            code([token(Print::to_string(op)), code_of_exp(exp_inner)])
        }
        ExpKind::Bin(op, _, exp_l, exp_r) => code([
            code_of_exp(exp_l),
            token(format!(" {} ", Print::to_string(op).replace('+', "{plus}"))),
            code_of_exp(exp_r),
        ]),
        ExpKind::Cmp(op, _, exp_l, exp_r) => code([
            code_of_exp(exp_l),
            token(format!(" {} ", Print::to_string(op))),
            code_of_exp(exp_r),
        ]),
        ExpKind::UpCast(_, exp_inner) | ExpKind::DownCast(_, exp_inner) => code_of_exp(exp_inner),
        ExpKind::Sub(exp_inner, typ, _) => {
            code([code_of_exp(exp_inner), token(" has type "), code_of_typ(typ)])
        }
        ExpKind::Match(exp_inner, pattern) => code_of_match(exp_inner, pattern),
        ExpKind::Tuple(exps) => code([token("( "), code_of_exps(exps, ", "), token(" )")]),
        ExpKind::Case(not_exp) => code_of_mixfix(not_exp, &code_of_exp),
        ExpKind::Str(fields) => code(
            std::iter::once(token("+{+"))
                .chain(
                    fields
                        .iter()
                        .enumerate()
                        .flat_map(|(idx, (atom, exp_field))| {
                            let code_field = code([
                                token(string_of_atom(atom)),
                                token(" "),
                                code_of_exp(exp_field),
                            ]);
                            if idx == 0 { vec![code_field] } else { vec![token(", "), code_field] }
                        }),
                )
                .chain(std::iter::once(token("+}+"))),
        ),
        ExpKind::Opt(None) => token("·"),
        ExpKind::Opt(Some(exp_inner)) => code_of_exp(exp_inner),
        ExpKind::List(exps) if exps.is_empty() => token("·"),
        ExpKind::List(exps) if exps.len() == 1 => code_of_exp(&exps[0]),
        ExpKind::List(exps) => code([token("+[+ "), code_of_exps(exps, ", "), token(" +]+")]),
        ExpKind::Cons(exp_head, exp_tail) => {
            code([code_of_exp(exp_head), token(" {two-colons} "), code_of_exp(exp_tail)])
        }
        ExpKind::Cat(exp_l, exp_r) => {
            code([code_of_exp(exp_l), token(" {pp} "), code_of_exp(exp_r)])
        }
        ExpKind::Mem(exp_elem, exp_set) => {
            code([code_of_exp(exp_elem), token(" is in "), code_of_exp(exp_set)])
        }
        ExpKind::Len(exp_inner) => code([token("the length of "), code_of_exp(exp_inner)]),
        ExpKind::Dot(exp_base, atom) => {
            code([code_of_exp(exp_base), token("."), token(string_of_atom(atom))])
        }
        ExpKind::Idx(exp_base, exp_idx) => {
            code([code_of_exp(exp_base), token("["), code_of_exp(exp_idx), token("]")])
        }
        ExpKind::Slice(exp_base, exp_idx, exp_len) => code([
            code_of_exp(exp_base),
            token("["),
            code_of_exp(exp_idx),
            token(" : "),
            code_of_exp(exp_len),
            token("]"),
        ]),
        ExpKind::Upd(exp_base, path, exp_field) => code([
            code_of_exp(exp_base),
            token("["),
            code_of_path(path),
            token(" = "),
            code_of_exp(exp_field),
            token("]"),
        ]),
        ExpKind::Call(id, targs, args) => subject_code(
            Subject::Function(id.node.clone()),
            code([token(string_of_defid(id)), token(string_of_targs(targs)), code_of_args(args)]),
        ),
        ExpKind::Iter(exp_inner, iter_exp) if iter_exp.vars.is_empty() => code_of_exp(exp_inner),
        ExpKind::Iter(exp_inner, iter_exp) => {
            let inner = code_of_exp(exp_inner);
            let needs_parens = !matches!(exp_inner.node.node, ExpKind::Id(_) | ExpKind::Tuple(_))
                && document::ser_code(&inner).contains(' ');
            if needs_parens {
                code([token("( "), inner, token(" )"), token(code_of_iter(iter_exp.iter))])
            } else {
                code([inner, token(code_of_iter(iter_exp.iter))])
            }
        }
    }
}

fn code_of_match(exp: &pl::Exp, pattern: &pl::Pattern) -> Code {
    use crate::lang::il::ast::{ListPattern, OptPattern, Pattern};
    let scrutinee = code_of_exp(exp);
    match pattern {
        Pattern::Case(mixop) if mixop.arity() == 0 => {
            code([scrutinee, token(" is "), code_of_pattern(pattern)])
        }
        Pattern::List(ListPattern::Nil) => code([scrutinee, token(" is an empty list")]),
        Pattern::List(ListPattern::Cons) => code([scrutinee, token(" is a non-empty list")]),
        Pattern::List(ListPattern::Fixed(num_elems)) => {
            code([scrutinee, token(format!(" is a list of length {num_elems}"))])
        }
        Pattern::Opt(OptPattern::None) => code([scrutinee, token(" is none")]),
        Pattern::Opt(OptPattern::Some) => code([scrutinee, token(" is defined")]),
        _ => code([scrutinee, token(" matches pattern "), code_of_pattern(pattern)]),
    }
}

// == Expressions as prose

fn prose_of_exps(exps: &[pl::Exp]) -> Prose {
    prose_of_list(exps.iter().map(prose_of_exp).collect())
}

/// Describes a pattern check using its specialized list and option wording.
fn prose_of_match(exp: &pl::Exp, pattern: &pl::Pattern) -> Prose {
    use crate::lang::il::ast::{ListPattern, OptPattern, Pattern};
    let scrutinee = prose_of_exp(exp);
    match pattern {
        Pattern::Case(mixop) if mixop.arity() == 0 => {
            prose([scrutinee, text(" is "), code_prose(code_of_pattern(pattern))])
        }
        Pattern::List(ListPattern::Nil) => prose([scrutinee, text(" is an empty list")]),
        Pattern::List(ListPattern::Cons) => prose([scrutinee, text(" is a non-empty list")]),
        Pattern::List(ListPattern::Fixed(num_elems)) => {
            prose([scrutinee, text(format!(" is a list of length {num_elems}"))])
        }
        Pattern::Opt(OptPattern::None) => prose([scrutinee, text(" is none")]),
        Pattern::Opt(OptPattern::Some) => prose([scrutinee, text(" is defined")]),
        _ => prose([scrutinee, text(" matches pattern "), code_prose(code_of_pattern(pattern))]),
    }
}

/// Describes the readable negation of a partial check when available.
fn prose_of_negated_exp(exp: &pl::Exp) -> Option<Prose> {
    match &exp.node.node {
        pl::ExpKind::Match(exp_elem, pattern) => Some(prose([
            prose_of_exp(exp_elem),
            text(" does not match pattern "),
            code_prose(code_of_pattern(pattern)),
        ])),
        pl::ExpKind::Sub(exp_elem, typ, _) => Some(prose([
            code_prose(code_of_exp(exp_elem)),
            text(" does not have type "),
            code_prose(code_of_typ(typ)),
        ])),
        pl::ExpKind::Mem(exp_elem, exp_set) => Some(prose([
            code_prose(code_of_exp(exp_elem)),
            text(" is not in "),
            code_prose(code_of_exp(exp_set)),
        ])),
        pl::ExpKind::Call(id, _, args) => exp
            .hints
            .prose_false
            .as_ref()
            .map(|hint| {
                subject_link(
                    Subject::Function(id.node.clone()),
                    alternate(
                        hint,
                        &|text_body| reindent_lines(0, text_body),
                        &prose_of_arg,
                        args,
                        false,
                    ),
                )
            })
            .or_else(|| Some(code_prose(code([token("~"), code_of_exp(exp)])))),
        _ => None,
    }
}

/// Renders an expression in readable prose form.
fn prose_of_exp(exp: &pl::Exp) -> Prose {
    use crate::lang::common::prim::bool::{BinOp as BoolBin, UnOp as BoolUn};
    use pl::ExpKind;
    match &exp.node.node {
        ExpKind::Un(pl::UnOp::Bool(BoolUn::Not), _, exp_inner) => {
            prose_of_negated_exp(exp_inner).unwrap_or_else(|| code_prose(code_of_exp(exp)))
        }
        ExpKind::Bin(pl::BinOp::Bool(BoolBin::Impl), _, exp_l, exp_r) => {
            prose([text("if "), prose_of_exp(exp_l), text(", then "), prose_of_exp(exp_r)])
        }
        ExpKind::Bin(op @ pl::BinOp::Bool(_), _, exp_l, exp_r) => prose([
            prose_of_exp(exp_l),
            text(format!(" {} ", string_of_binop(*op))),
            prose_of_exp(exp_r),
        ]),
        ExpKind::Cmp(op, _, exp_l, exp_r) => prose([
            prose_of_exp(exp_l),
            text(format!(" {} ", string_of_cmpop(*op))),
            prose_of_exp(exp_r),
        ]),
        ExpKind::UpCast(_, exp_inner) | ExpKind::DownCast(_, exp_inner) => {
            code_prose(code_of_exp(exp_inner))
        }
        ExpKind::Sub(exp_inner, typ, _) => prose([
            code_prose(code_of_exp(exp_inner)),
            text(" has type "),
            code_prose(code_of_typ(typ)),
        ]),
        ExpKind::Match(exp_inner, pattern) => prose_of_match(exp_inner, pattern),
        ExpKind::Tuple(exps) => prose([
            text("( "),
            prose(exps.iter().enumerate().flat_map(|(idx, exp)| {
                if idx == 0 { vec![prose_of_exp(exp)] } else { vec![text(", "), prose_of_exp(exp)] }
            })),
            text(" )"),
        ]),
        ExpKind::Case(not_exp) => {
            if let (Some(hint), pl::TypKind::Var(id_typ, _)) = (&exp.hints.prose, &exp.node.note) {
                let exps = not_exp.args().into_iter().cloned().collect::<Vec<_>>();
                direct_link(
                    id_typ.node.clone(),
                    alternate(
                        hint,
                        &|text_body| reindent_lines(0, text_body),
                        &prose_of_exp,
                        &exps,
                        false,
                    ),
                )
            } else {
                code_prose(code_of_mixfix(not_exp, &code_of_exp))
            }
        }
        ExpKind::Str(fields) => prose(
            std::iter::once(text("+{+"))
                .chain(
                    fields
                        .iter()
                        .enumerate()
                        .flat_map(|(idx, (atom, exp_field))| {
                            let prose_field = prose([
                                text(string_of_atom(atom)),
                                text(" "),
                                prose_of_exp(exp_field),
                            ]);
                            if idx == 0 { vec![prose_field] } else { vec![text(", "), prose_field] }
                        }),
                )
                .chain(std::iter::once(text("+}+"))),
        ),
        ExpKind::Opt(Some(exp_inner)) => prose_of_exp(exp_inner),
        ExpKind::Cat(exp_l, exp_r) => {
            prose([prose_of_exp(exp_l), text(" concatenated with "), prose_of_exp(exp_r)])
        }
        ExpKind::Mem(exp_elem, exp_set) => {
            prose([prose_of_exp(exp_elem), text(" is in "), prose_of_exp(exp_set)])
        }
        ExpKind::Len(exp_inner) => prose([text("the length of "), prose_of_exp(exp_inner)]),
        ExpKind::Upd(exp_base, path, exp_field) => prose([
            code_prose(code_of_exp(exp_base)),
            text(" with "),
            code_prose(code_of_path(path)),
            text(" set to "),
            code_prose(code_of_exp(exp_field)),
        ]),
        ExpKind::Call(id, _, args) => {
            let hint_opt = exp
                .hints
                .prose_in
                .as_ref()
                .or(exp.hints.prose_true.as_ref());
            hint_opt.map_or_else(
                || code_prose(code_of_exp(exp)),
                |hint| {
                    subject_link(
                        Subject::Function(id.node.clone()),
                        alternate(
                            hint,
                            &|text_body| reindent_lines(0, text_body),
                            &prose_of_arg,
                            args,
                            false,
                        ),
                    )
                },
            )
        }
        ExpKind::Iter(exp_inner, iter_exp) if iter_exp.vars.is_empty() => prose_of_exp(exp_inner),
        ExpKind::Bool(_)
        | ExpKind::Num(_)
        | ExpKind::Text(_)
        | ExpKind::Id(_)
        | ExpKind::Un(..)
        | ExpKind::Bin(..)
        | ExpKind::Opt(None)
        | ExpKind::List(_)
        | ExpKind::Cons(..)
        | ExpKind::Dot(..)
        | ExpKind::Idx(..)
        | ExpKind::Slice(..)
        | ExpKind::Iter(..) => code_prose(code_of_exp(exp)),
    }
}

fn code_of_param(param: &pl::Param) -> Code {
    match &param.node {
        pl::ParamKind::Exp(_, exp) => code_of_exp(exp),
        pl::ParamKind::Def(id, _, _, _) => token(string_of_defid(id)),
    }
}

fn code_of_params(params: &[pl::Param]) -> Code {
    if params.is_empty() {
        Code::Empty
    } else {
        code(
            std::iter::once(token("("))
                .chain(params.iter().enumerate().flat_map(|(idx, param)| {
                    let param = code_of_param(param);
                    if idx == 0 { vec![param] } else { vec![token(", "), param] }
                }))
                .chain(std::iter::once(token(")"))),
        )
    }
}

fn prose_of_param(param: &pl::Param) -> Prose {
    match &param.node {
        pl::ParamKind::Exp(_, exp) => prose_of_exp(exp),
        pl::ParamKind::Def(id, _, _, _) => code_prose(token(string_of_defid(id))),
    }
}

fn prose_of_params(params: &[pl::Param]) -> Prose {
    if params.is_empty() {
        Prose::Empty
    } else {
        prose(
            std::iter::once(text("("))
                .chain(params.iter().enumerate().flat_map(|(idx, param)| {
                    let param = prose_of_param(param);
                    if idx == 0 { vec![param] } else { vec![text(", "), param] }
                }))
                .chain(std::iter::once(text(")"))),
        )
    }
}

fn prose_of_arg(arg: &pl::Arg) -> Prose {
    match &arg.node {
        pl::ArgKind::Exp(exp) => prose_of_exp(exp),
        pl::ArgKind::Def(id) => code_prose(token(string_of_defid(id))),
    }
}

// == Rendering context

/// Per-invocation state shared by every rendered fragment in one document.
struct Renderer<'a> {
    anchors: Anchors,
    anchor: &'a dyn Fn(&Subject) -> Option<String>,
}

impl<'a> Renderer<'a> {
    fn new(anchor: &'a dyn Fn(&Subject) -> Option<String>) -> Self {
        Self { anchors: Anchors::default(), anchor }
    }
}

// == Guards

/// Describes a case guard against its scrutinee.
fn prose_of_guard(exp_scrut: &pl::Exp, guard: &pl::Guard) -> Prose {
    match guard {
        pl::Guard::Bool(true) => prose_of_exp(exp_scrut),
        pl::Guard::Bool(false) => prose_of_negated_exp(exp_scrut)
            .unwrap_or_else(|| code_prose(code([token("~"), code_of_exp(exp_scrut)]))),
        pl::Guard::Cmp(op, _, exp) => prose([
            prose_of_exp(exp_scrut),
            text(format!(" {} ", string_of_cmpop(*op))),
            prose_of_exp(exp),
        ]),
        pl::Guard::Sub(typ, _) => prose([
            code_prose(code_of_exp(exp_scrut)),
            text(" has type "),
            code_prose(code_of_typ(typ)),
        ]),
        pl::Guard::Match(pattern) => prose([
            prose_of_exp(exp_scrut),
            text(" matches pattern "),
            code_prose(code_of_pattern(pattern)),
        ]),
        pl::Guard::Mem(exp) => prose([prose_of_exp(exp_scrut), text(" is in "), prose_of_exp(exp)]),
        pl::Guard::CheckLetSub(_, _, exp_target) | pl::Guard::CheckLetMatch(_, exp_target) => {
            prose([
                text("let "),
                code_prose(code_of_exp(exp_target)),
                text(" be "),
                prose_of_exp(exp_scrut),
            ])
        }
    }
}

// == Partial constructs

/// Reports whether evaluation of a path can fail.
fn is_partial_path(path: &pl::Path) -> bool {
    match &path.node {
        pl::PathKind::Root => false,
        pl::PathKind::Idx(path_base, exp_idx) => {
            is_partial_path(path_base) || is_partial_exp(exp_idx)
        }
        pl::PathKind::Slice(path_base, exp_idx, exp_len) => {
            is_partial_path(path_base) || is_partial_exp(exp_idx) || is_partial_exp(exp_len)
        }
        pl::PathKind::Dot(path_base, _) => is_partial_path(path_base),
    }
}

/// Reports whether evaluation of an expression can fail.
fn is_partial_exp(exp: &pl::Exp) -> bool {
    use pl::ExpKind;
    match &exp.node.node {
        ExpKind::Bool(_) | ExpKind::Num(_) | ExpKind::Text(_) | ExpKind::Id(_) => false,
        ExpKind::Un(_, _, exp_inner)
        | ExpKind::UpCast(_, exp_inner)
        | ExpKind::DownCast(_, exp_inner)
        | ExpKind::Sub(exp_inner, _, _)
        | ExpKind::Match(exp_inner, _)
        | ExpKind::Len(exp_inner)
        | ExpKind::Dot(exp_inner, _)
        | ExpKind::Iter(exp_inner, _) => is_partial_exp(exp_inner),
        ExpKind::Bin(_, _, exp_l, exp_r)
        | ExpKind::Cmp(_, _, exp_l, exp_r)
        | ExpKind::Cons(exp_l, exp_r)
        | ExpKind::Cat(exp_l, exp_r)
        | ExpKind::Mem(exp_l, exp_r)
        | ExpKind::Idx(exp_l, exp_r) => is_partial_exp(exp_l) || is_partial_exp(exp_r),
        ExpKind::Tuple(exps) | ExpKind::List(exps) => exps.iter().any(is_partial_exp),
        ExpKind::Case(not_exp) => not_exp.args().into_iter().any(is_partial_exp),
        ExpKind::Str(fields) => fields.iter().any(|(_, exp)| is_partial_exp(exp)),
        ExpKind::Opt(exp_opt) => exp_opt.as_deref().is_some_and(is_partial_exp),
        ExpKind::Slice(exp_base, exp_idx, exp_len) => {
            is_partial_exp(exp_base) || is_partial_exp(exp_idx) || is_partial_exp(exp_len)
        }
        ExpKind::Upd(exp_base, path, exp_field) => {
            is_partial_exp(exp_base) || is_partial_path(path) || is_partial_exp(exp_field)
        }
        ExpKind::Call(..) => true,
    }
}

fn is_partial_guard(guard: &pl::Guard) -> bool {
    match guard {
        pl::Guard::Bool(_) | pl::Guard::Sub(..) | pl::Guard::Match(_) | pl::Guard::Mem(_) => false,
        pl::Guard::Cmp(_, _, exp)
        | pl::Guard::CheckLetSub(_, _, exp)
        | pl::Guard::CheckLetMatch(_, exp) => is_partial_exp(exp),
    }
}

// == Instructions

/// A tier instruction ready to fold inline or nest below its enclosing head.
enum Rendered {
    Inline(Prose),
    InlineGoto(Prose),
    Nested(Block),
}

/// Renders one tier payload while preserving the shared renderer state.
type RenderTier<Tier> =
    fn(&mut Renderer<'_>, usize, &Context, bool, &pl::Instr<Tier>, &Tier) -> Rendered;

/// Composes a tier result with the enclosing instruction head.
fn compose(block_head: Option<Block>, singleton: bool, rendered: Rendered) -> Block {
    match rendered {
        Rendered::Inline(prose_tail) => match block_head {
            Some(block_head) => concat([block_head, inline(prose_tail)]),
            None => inline(prose_tail),
        },
        Rendered::InlineGoto(prose_tail) => document::capitalize_first_block(match block_head {
            Some(block_head) => concat([block_head, inline(prose_tail)]),
            None => inline(prose_tail),
        }),
        Rendered::Nested(block) if !singleton => block,
        Rendered::Nested(block) => match block_head {
            Some(block_head) => seq([block_head, block]),
            None => concat([raw("\n"), seq([block])]),
        },
    }
}

/// Renders one shared or tier-specific instruction.
fn render_instr<Tier>(
    renderer: &mut Renderer<'_>,
    level: usize,
    ctx: &Context,
    render_tier: RenderTier<Tier>,
    instr: &pl::Instr<Tier>,
) -> Block {
    match &instr.node.node {
        pl::InstrKind::If(if_instr) => {
            render_if_instr(renderer, level, ctx, render_tier, instr, if_instr)
        }
        pl::InstrKind::Hold(hold_instr) => {
            render_hold_instr(renderer, level, ctx, render_tier, instr, hold_instr)
        }
        pl::InstrKind::Case(case_instr) => {
            render_case_instr(renderer, level, ctx, render_tier, instr, case_instr)
        }
        pl::InstrKind::Let(let_instr) => render_let_instr(level, ctx, instr, let_instr),
        pl::InstrKind::Debug(debug_instr) => render_debug_instr(level, ctx, instr, debug_instr),
        pl::InstrKind::Destruct(destruct_instr) => {
            render_destruct_instr(level, ctx, instr, destruct_instr)
        }
        pl::InstrKind::CheckLetSub(check_instr) => render_check_let_instr(
            renderer,
            level,
            ctx,
            render_tier,
            instr,
            (&check_instr.exp_l, &check_instr.exp_r, &check_instr.block),
        ),
        pl::InstrKind::CheckLetMatch(check_instr) => render_check_let_instr(
            renderer,
            level,
            ctx,
            render_tier,
            instr,
            (&check_instr.exp_l, &check_instr.exp_r, &check_instr.block),
        ),
        pl::InstrKind::OptionGet(option_instr) => {
            render_option_get_instr(renderer, level, ctx, render_tier, instr, option_instr)
        }
        pl::InstrKind::Tier(tier_instr) => {
            compose(None, false, render_tier(renderer, level, ctx, false, instr, &tier_instr.tier))
        }
    }
}

/// Renders instructions under an optional heading.
fn render_instrs<Tier>(
    renderer: &mut Renderer<'_>,
    level: usize,
    block_head: Option<Block>,
    ctx: &Context,
    render_tier: RenderTier<Tier>,
    instrs: &[pl::Instr<Tier>],
) -> Block {
    // Fold a lone tier instruction into the enclosing heading
    if let [instr] = instrs
        && let pl::InstrKind::Tier(tier_instr) = &instr.node.node
    {
        return compose(
            block_head,
            true,
            render_tier(renderer, level, ctx, true, instr, &tier_instr.tier),
        );
    }

    // Render general blocks as a sequence below the optional heading
    let blocks_rendered = instrs
        .iter()
        .map(|instr| render_instr(renderer, level, ctx, render_tier, instr))
        .collect::<Vec<_>>();
    match block_head {
        Some(block_head) => seq(std::iter::once(block_head).chain(blocks_rendered)),
        None => concat([raw("\n"), seq(blocks_rendered)]),
    }
}

/// Serializes an otherwise block with its optional anchor.
fn render_elseblock<Tier>(
    renderer: &mut Renderer<'_>,
    anchor_else: Option<&str>,
    ctx: &Context,
    render_tier: RenderTier<Tier>,
    block_opt: Option<&[pl::Instr<Tier>]>,
) -> String {
    // Omit absent and empty otherwise blocks
    let Some(block) = block_opt.filter(|block| !block.is_empty()) else {
        return String::new();
    };
    // Prefix the visible heading with its optional destination anchor
    let text_anchor = anchor_else
        .map(|anchor| format!("+++<span id=\"{anchor}\"></span>+++"))
        .unwrap_or_default();
    format!(
        "\n\n. {text_anchor}Otherwise:{}",
        document::ser_block_with_anchor(
            &render_instrs(renderer, 1, None, ctx, render_tier, block),
            renderer.anchor,
        )
    )
}

// - Iterations

fn prose_of_in_itervar(iter: Iter, var: &pl::Var) -> Prose {
    prose([
        code_prose(code_of_var(var)),
        text(" in "),
        code_prose(code([code_of_var(var), token(code_of_iter(iter))])),
    ])
}

fn prose_of_in_itervars(iter: Iter, vars: &[pl::Var]) -> Prose {
    prose_of_list(
        vars.iter()
            .map(|var| prose_of_in_itervar(iter, var))
            .collect(),
    )
}

fn prose_of_out_itervars(iter: Iter, vars: &[pl::Var]) -> Prose {
    prose_of_list(
        vars.iter()
            .filter(|var| !var.id.node.starts_with('_'))
            .map(|var| code_prose(code([code_of_var(var), token(code_of_iter(iter))])))
            .collect(),
    )
}

fn prose_of_iterexp_suffix(iter_exps: &[pl::ExpIter]) -> Prose {
    // Collect every expression iterator binding in source order
    let proses = iter_exps
        .iter()
        .flat_map(|iter_exp| {
            iter_exp
                .vars
                .iter()
                .map(|var| prose_of_in_itervar(iter_exp.iter, var))
        })
        .collect::<Vec<_>>();
    // Omit the quantifier when no variables are bound
    if proses.is_empty() {
        Prose::Empty
    } else {
        prose([text(", for all "), prose_of_list(proses)])
    }
}

fn prose_of_iterinstr_suffix(iter_instrs: &[pl::InstrIter]) -> Prose {
    // Collect every instruction iterator binding in source order
    let proses = iter_instrs
        .iter()
        .flat_map(|iter_instr| {
            iter_instr
                .vars_bound
                .iter()
                .map(|var| prose_of_in_itervar(iter_instr.iter, var))
        })
        .collect::<Vec<_>>();
    // Omit the quantifier when no variables are bound
    if proses.is_empty() {
        Prose::Empty
    } else {
        prose([text(", for each "), prose_of_list(proses)])
    }
}

/// Wraps a body in nested iteration blocks and binds visible outputs.
fn render_iterinstrs(
    level: usize,
    prose_fallthrough: Prose,
    iter_instrs: &[pl::InstrIter],
    render_body: &dyn Fn(usize) -> Block,
) -> Block {
    fn go(
        level: usize,
        outermost: bool,
        prose_fallthrough: &Prose,
        iter_instrs: &[pl::InstrIter],
        render_body: &dyn Fn(usize) -> Block,
    ) -> Block {
        // Finish recursion at the caller-supplied body
        let Some((iter_instr, iter_instrs_tail)) = iter_instrs.split_last() else {
            return render_body(level);
        };
        // Keep only outputs that appear in the rendered binding
        let vars_output = iter_instr
            .vars_bind
            .iter()
            .filter(|var| !var.id.node.starts_with('_'))
            .cloned()
            .collect::<Vec<_>>();
        // Render inner iteration levels before their enclosing open block
        let block_inner = go(level + 1, false, prose_fallthrough, iter_instrs_tail, render_body);
        let prose_head = prose([
            text("For each "),
            prose_of_in_itervars(iter_instr.iter, &iter_instr.vars_bound),
            text(":"),
        ]);
        let fallthrough = if outermost { prose_fallthrough.clone() } else { Prose::Empty };
        // Bind visible outputs after closing the iteration body
        let block_body = if vars_output.is_empty() {
            concat([raw("+\n--\n"), block_inner, raw("\n--\n")])
        } else {
            let text_noun = string_of_iter(iter_instr.iter);
            let text_suffix = if vars_output.len() > 1 { "s" } else { "" };
            concat([
                raw("+\n--\n"),
                block_inner,
                raw("\n--\n+\n"),
                inline(prose([
                    text("Let "),
                    prose_of_out_itervars(iter_instr.iter, &vars_output),
                    text(format!(" be the resulting {text_noun}{text_suffix}.")),
                    fallthrough,
                ])),
            ])
        };
        ordered_body(level, None, prose_head, block_body)
    }

    go(level, true, &prose_fallthrough, iter_instrs, render_body)
}

// - Shared instruction forms

/// Renders a conditional check and its continuation.
fn render_if_instr<Tier>(
    renderer: &mut Renderer<'_>,
    level: usize,
    ctx: &Context,
    render_tier: RenderTier<Tier>,
    instr: &pl::Instr<Tier>,
    if_instr: &pl::IfInstr<Tier>,
) -> Block {
    // Build the check heading with its failure continuation
    let block_head = ordered(
        level,
        prose([
            text("Check that "),
            prose_of_exp(&if_instr.exp),
            prose_of_iterexp_suffix(&if_instr.iter_exps),
            text("."),
            fallthrough::prose_of_link(ctx, instr),
        ]),
    );
    // Append a nonempty continuation below the heading
    if if_instr.block.is_empty() {
        block_head
    } else {
        seq(std::iter::once(block_head).chain(
            if_instr
                .block
                .iter()
                .map(|instr| render_instr(renderer, level, ctx, render_tier, instr)),
        ))
    }
}

/// Renders positive, negative, or two-sided relation holding branches.
fn render_hold_instr<Tier>(
    renderer: &mut Renderer<'_>,
    level: usize,
    ctx: &Context,
    render_tier: RenderTier<Tier>,
    instr: &pl::Instr<Tier>,
    hold_instr: &pl::HoldInstr<Tier>,
) -> Block {
    // Extract relation arguments once for hinted prose
    let exps = hold_instr
        .not_exp
        .args()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let iter_suffix = prose_of_iterexp_suffix(&hold_instr.iter_exps);
    // Build either the positive or negative branch heading
    let make_head = |hold: bool| {
        let hint_opt = if hold { &instr.hints.prose_true } else { &instr.hints.prose_false };
        let prose_cond = hint_opt.as_ref().map_or_else(
            || {
                prose([
                    subject_link(
                        Subject::Relation(hold_instr.id.node.clone()),
                        code_prose(code_of_mixfix(&hold_instr.not_exp, &code_of_exp)),
                    ),
                    text(if hold { " holds" } else { " does not hold" }),
                ])
            },
            |hint| {
                subject_link(
                    Subject::Relation(hold_instr.id.node.clone()),
                    alternate(
                        hint,
                        &|text_body| reindent_lines(0, text_body),
                        &prose_of_exp,
                        &exps,
                        false,
                    ),
                )
            },
        );
        ordered(
            level,
            prose([
                text("If "),
                prose_cond,
                iter_suffix.clone(),
                text(":"),
                fallthrough::prose_of_link(ctx, instr),
            ]),
        )
    };
    // Render the selected one-sided or two-sided branch structure
    match &hold_instr.hold_case {
        pl::HoldCase::Hold(block, _) => {
            render_instrs(renderer, level + 1, Some(make_head(true)), ctx, render_tier, block)
        }
        pl::HoldCase::NotHold(block, _) => {
            render_instrs(renderer, level + 1, Some(make_head(false)), ctx, render_tier, block)
        }
        pl::HoldCase::Both(block_hold, block_not_hold) => seq([
            render_instrs(renderer, level + 1, Some(make_head(true)), ctx, render_tier, block_hold),
            render_instrs(
                renderer,
                level + 1,
                Some(ordered(level, text("Else:"))),
                ctx,
                render_tier,
                block_not_hold,
            ),
        ]),
    }
}

/// Renders a check or an if/else-if/else case ladder.
fn render_case_instr<Tier>(
    renderer: &mut Renderer<'_>,
    level: usize,
    ctx: &Context,
    render_tier: RenderTier<Tier>,
    instr: &pl::Instr<Tier>,
    case_instr: &pl::CaseInstr<Tier>,
) -> Block {
    let fallthrough = fallthrough::prose_of_link(ctx, instr);
    // Render a single arm as a check without an if ladder
    if let [case] = case_instr.cases.as_slice() {
        let block_head = ordered(
            level,
            prose([
                text("Check that "),
                prose_of_guard(&case_instr.exp, &case.guard),
                text("."),
                fallthrough,
            ]),
        );
        return if case.block.is_empty() {
            block_head
        } else {
            seq(std::iter::once(block_head).chain(
                case.block
                    .iter()
                    .map(|instr| render_instr(renderer, level, ctx, render_tier, instr)),
            ))
        };
    }

    let num_cases = case_instr.cases.len();
    let mut blocks_case = Vec::with_capacity(num_cases);
    for (idx, case) in case_instr.cases.iter().enumerate() {
        // Turn the final total arm into an otherwise branch
        if idx + 1 == num_cases && !case_instr.dangle {
            let block_else = ordered(level, text("Else:"));
            if matches!(case.guard, pl::Guard::CheckLetSub(..) | pl::Guard::CheckLetMatch(..)) {
                let block_bind = ordered(
                    level + 1,
                    prose([
                        document::capitalize_first_prose(prose_of_guard(
                            &case_instr.exp,
                            &case.guard,
                        )),
                        text("."),
                    ]),
                );
                blocks_case.push(seq(std::iter::once(block_else)
                    .chain(std::iter::once(block_bind))
                    .chain(
                        case.block.iter().map(|instr| {
                            render_instr(renderer, level + 1, ctx, render_tier, instr)
                        }),
                    )));
            } else {
                blocks_case.push(render_instrs(
                    renderer,
                    level + 1,
                    Some(block_else),
                    ctx,
                    render_tier,
                    &case.block,
                ));
            }
        } else {
            // Attach fallthrough only where evaluating the condition can fail
            let label =
                if is_partial_guard(&case.guard) || (idx == 0 && is_partial_exp(&case_instr.exp)) {
                    fallthrough.clone()
                } else {
                    Prose::Empty
                };
            let keyword = if idx == 0 { "If " } else { "Else if " };
            // Nest the selected case body below its condition
            let block_head = ordered(
                level,
                prose([
                    text(keyword),
                    prose_of_guard(&case_instr.exp, &case.guard),
                    text(":"),
                    label,
                ]),
            );
            blocks_case.push(render_instrs(
                renderer,
                level + 1,
                Some(block_head),
                ctx,
                render_tier,
                &case.block,
            ));
        }
    }
    seq(blocks_case)
}

/// Renders a binding and any instruction iterations around it.
fn render_let_instr<Tier>(
    level: usize,
    ctx: &Context,
    instr: &pl::Instr<Tier>,
    let_instr: &pl::LetInstr,
) -> Block {
    let fallthrough = fallthrough::prose_of_link(ctx, instr);
    // Detect outputs that require explicit iteration blocks
    let has_output = let_instr
        .iter_instrs
        .iter()
        .flat_map(|iter_instr| &iter_instr.vars_bind)
        .any(|var| !var.id.node.starts_with('_'));
    // Keep output-free bindings inline with their iterator suffix
    if !has_output {
        ordered(
            level,
            prose([
                text("Let "),
                code_prose(code_of_exp(&let_instr.exp_l)),
                text(" be "),
                prose_of_exp(&let_instr.exp_r),
                prose_of_iterinstr_suffix(&let_instr.iter_instrs),
                text("."),
                fallthrough,
            ]),
        )
    } else {
        // Nest output-producing bindings under their iteration scopes
        render_iterinstrs(level, fallthrough, &let_instr.iter_instrs, &|level| {
            unordered(
                level,
                prose([
                    text("Let "),
                    code_prose(code_of_exp(&let_instr.exp_l)),
                    text(" be "),
                    prose_of_exp(&let_instr.exp_r),
                    text("."),
                ]),
            )
        })
    }
}

fn render_debug_instr<Tier>(
    level: usize,
    ctx: &Context,
    instr: &pl::Instr<Tier>,
    debug_instr: &pl::DebugInstr,
) -> Block {
    ordered(
        level,
        prose([
            text("(debug: "),
            prose_of_exp(&debug_instr.exp),
            text(")"),
            fallthrough::prose_of_link(ctx, instr),
        ]),
    )
}

/// Renders named destructuring projections.
fn render_destruct_instr<Tier>(
    level: usize,
    ctx: &Context,
    instr: &pl::Instr<Tier>,
    destruct_instr: &pl::DestructInstr,
) -> Block {
    // Discard unnamed projections from the prose binding
    let projections = destruct_instr
        .bindings
        .iter()
        .filter_map(|(name, exp)| name.as_ref().map(|name| (name, exp)))
        .collect::<Vec<_>>();
    let fallthrough = fallthrough::prose_of_link(ctx, instr);
    // Use dedicated singular prose for one named projection
    if let [(name, exp_target)] = projections.as_slice() {
        ordered(
            level,
            prose([
                text("Let "),
                prose_of_exp(exp_target),
                text(format!(" be the {name} of ")),
                prose_of_exp(&destruct_instr.exp),
                text("."),
                fallthrough,
            ]),
        )
    } else {
        // Pair multiple targets with their projection names
        ordered(
            level,
            prose([
                text("Let "),
                prose_of_list(
                    projections
                        .iter()
                        .map(|(_, exp)| prose_of_exp(exp))
                        .collect(),
                ),
                text(" be "),
                prose_of_list(
                    projections
                        .iter()
                        .map(|(name, _)| text(format!("the {name}")))
                        .collect(),
                ),
                text(" of "),
                prose_of_exp(&destruct_instr.exp),
                text("."),
                fallthrough,
            ]),
        )
    }
}

/// Renders a partial subtype or pattern binding.
fn render_check_let_instr<Tier>(
    renderer: &mut Renderer<'_>,
    level: usize,
    ctx: &Context,
    render_tier: RenderTier<Tier>,
    instr: &pl::Instr<Tier>,
    binding: (&pl::Exp, &pl::Exp, &[pl::Instr<Tier>]),
) -> Block {
    let (exp_l, exp_r, block) = binding;
    // Build the partial binding heading with its failure continuation
    let block_head = ordered(
        level,
        prose([
            text("Let!~type~ "),
            code_prose(code_of_exp(exp_l)),
            text(" be "),
            prose_of_exp(exp_r),
            text("."),
            fallthrough::prose_of_link(ctx, instr),
        ]),
    );
    // Append a nonempty successful continuation below the heading
    if block.is_empty() {
        block_head
    } else {
        seq(std::iter::once(block_head).chain(
            block
                .iter()
                .map(|instr| render_instr(renderer, level, ctx, render_tier, instr)),
        ))
    }
}

/// Renders a forced option binding.
fn render_option_get_instr<Tier>(
    renderer: &mut Renderer<'_>,
    level: usize,
    ctx: &Context,
    render_tier: RenderTier<Tier>,
    instr: &pl::Instr<Tier>,
    option_instr: &pl::OptionGetInstr<Tier>,
) -> Block {
    // Build the forced binding heading with its failure continuation
    let block_head = ordered(
        level,
        prose([
            text("Let "),
            code_prose(code_of_exp(&option_instr.exp_l)),
            text(" be "),
            text(super::utils::adoc_link("option_get", "*!*")),
            text(" "),
            prose_of_exp(&option_instr.exp_r),
            text("."),
            fallthrough::prose_of_link(ctx, instr),
        ]),
    );
    // Append a nonempty successful continuation below the heading
    if option_instr.block.is_empty() {
        block_head
    } else {
        seq(std::iter::once(block_head).chain(
            option_instr
                .block
                .iter()
                .map(|instr| render_instr(renderer, level, ctx, render_tier, instr)),
        ))
    }
}

// == Relations

/// Lifts a synthesized SL output expression into an unhinted PL expression.
fn lift_synthesized_exp(exp_sl: &sl::ast::Exp) -> pl::Exp {
    let exp_kind = match &exp_sl.node {
        sl::ast::ExpKind::Id(id) => pl::ExpKind::Id(id.clone()),
        sl::ast::ExpKind::Iter(exp_inner, iter_exp) => {
            pl::ExpKind::Iter(Box::new(lift_synthesized_exp(exp_inner)), iter_exp.clone())
        }
        _ => panic!("relation title outputs are synthesized variables"),
    };
    crate::annotated_note_phrase! {
        node: exp_kind,
        note: exp_sl.note.as_ref().clone(),
        span: exp_sl.span.clone(),
    }
}

/// Fills relation inputs and leaves output positions as percent holes.
fn prose_of_rel_title_math(signature: &pl::RelSignature, exps: &[pl::Exp]) -> Prose {
    let mixop = signature.not_typ.node.to_mixop();
    let num_outputs = mixop.arity() - exps.len();
    let codes_args = input::combine(
        &signature.input_hint,
        exps.iter().map(code_of_exp).collect(),
        (0..num_outputs).map(|_| token("%")).collect(),
    )
    .expect("validated relation input hint");
    let not_exp = pl::Mixop::fill(&mixop, codes_args).expect("relation title fills its notation");
    code_prose(code_of_mixfix(&not_exp, &Clone::clone))
}

/// Builds a relation title from input, output, truth, or notation prose.
fn render_rel_title_block(
    hints: &Hints,
    id_rel: &pl::Id,
    signature: &pl::RelSignature,
    exps: &[pl::Exp],
) -> Block {
    // Prefer synthesized inputs when prosification changed title bindings
    let exps_input = hints
        .prose_input_exps
        .as_ref()
        .map(|exps| exps.iter().map(lift_synthesized_exp).collect::<Vec<_>>());
    let exps_input = exps_input.as_deref().unwrap_or(exps);
    // Build the shared linked heading before selecting the title form
    let prose_title =
        subject_link(Subject::Relation(id_rel.node.clone()), text(id_rel.node.clone()));
    let block_header = concat([inline(prose([prose_title, text(":")])), raw("\n\n")]);
    // Select paired, input-only, truth, or notation prose
    match (&hints.prose_in, &hints.prose_out, &hints.prose_output_exps, &hints.prose_true) {
        // Reject incomplete paired output hints
        (Some(_), Some(_), None, _) => panic!("prose_out title requires synthesized outputs"),
        (Some(hint_input), Some(hint_output), Some(exps_output_sl), _) => {
            // Render paired input and synthesized output prose
            let exps_output = exps_output_sl
                .iter()
                .map(lift_synthesized_exp)
                .collect::<Vec<_>>();
            concat([
                block_header,
                unordered(
                    0,
                    alternate(
                        hint_input,
                        &|text_body| reindent_lines(1, text_body),
                        &prose_of_exp,
                        exps_input,
                        true,
                    ),
                ),
                raw(":\n"),
                unordered(
                    0,
                    prose([
                        text("The result is "),
                        alternate(
                            hint_output,
                            &|text_body| reindent_lines(1, text_body),
                            &prose_of_exp,
                            &exps_output,
                            false,
                        ),
                    ]),
                ),
                raw("."),
            ])
        }
        // Render input prose without a synthesized result
        (Some(hint_input), _, _, _) => concat([
            block_header,
            unordered(
                0,
                alternate(
                    hint_input,
                    &|text_body| reindent_lines(1, text_body),
                    &prose_of_exp,
                    exps_input,
                    true,
                ),
            ),
            raw("."),
        ]),
        // Render a direct truth description
        (_, _, _, Some(hint_true)) => concat([
            block_header,
            unordered(
                0,
                alternate(
                    hint_true,
                    &|text_body| reindent_lines(0, text_body),
                    &prose_of_exp,
                    exps,
                    true,
                ),
            ),
        ]),
        // Fall back to the filled relation notation
        _ => inline(subject_link(
            Subject::Relation(id_rel.node.clone()),
            prose([text(format!("{}: ", id_rel.node)), prose_of_rel_title_math(signature, exps)]),
        )),
    }
}

/// Renders a relation title using caller-supplied anchors.
pub fn render_rel_title_with_anchor(
    hints: &Hints,
    id_rel: &pl::Id,
    signature: &pl::RelSignature,
    exps: &[pl::Exp],
    anchor: &dyn Fn(&Subject) -> Option<String>,
) -> String {
    document::ser_block_with_anchor(&render_rel_title_block(hints, id_rel, signature, exps), anchor)
}

/// Renders a relation title with definition-name anchors.
pub fn render_rel_title(
    hints: &Hints,
    id_rel: &pl::Id,
    signature: &pl::RelSignature,
    exps: &[pl::Exp],
) -> String {
    render_rel_title_with_anchor(hints, id_rel, signature, exps, &document::subject_name)
}

// == Tier renderers

/// Renders backtracking arms with derived next-arm targets.
fn render_block_arms<Arm>(
    renderer: &mut Renderer<'_>,
    level: usize,
    ctx: &Context,
    arms: &[Arm],
    render_arm: &dyn Fn(&mut Renderer<'_>, &Context, &Arm) -> Block,
) -> Block {
    // Allocate one shared namespace for every arm target
    let anchor_block = renderer.anchors.fresh_block(&ctx.namespace);
    let num_arms = arms.len();
    let blocks_rendered = arms.iter().enumerate().map(|(idx, arm)| {
        // Point each arm at its successor or the enclosing destination
        let anchor_next_opt = if idx + 1 < num_arms {
            Some(fallthrough::anchor_of_arm(&anchor_block, idx + 1))
        } else {
            ctx.next.clone()
        };
        let ctx_arm = Context { namespace: ctx.namespace.clone(), next: anchor_next_opt };
        // Anchor the arm before rendering its body with the derived context
        ordered_body(
            level,
            Some(fallthrough::anchor_of_arm(&anchor_block, idx)),
            text(if idx == 0 { "Try:" } else { "Then, try:" }),
            render_arm(renderer, &ctx_arm, arm),
        )
    });
    seq(blocks_rendered)
}

/// Describes a relation result according to its output shape and hints.
fn prose_of_result(hints: &Hints, signature: &pl::RelSignature, exps: &[pl::Exp]) -> Prose {
    let typs = signature.not_typ.node.args();
    if input::is_conditional(&signature.input_hint, &typs).expect("validated relation input hint") {
        text("then, the relation holds.")
    } else if let Some(hint) = &hints.prose_out {
        prose([
            text("the result is "),
            alternate(hint, &|text_body| reindent_lines(0, text_body), &prose_of_exp, exps, false),
            text("."),
        ])
    } else if exps.is_empty() {
        text("the relation holds.")
    } else {
        prose([text("the result is "), prose_of_exps(exps), text(".")])
    }
}

/// Renders a relation application and its bound outputs.
fn render_rule_instr(
    level: usize,
    ctx: &Context,
    instr: &pl::Instr<pl::GroupInstr>,
    rule_instr: &pl::RuleInstr,
) -> Block {
    // Split the notation into input and output expressions
    let exps = rule_instr
        .not_exp
        .args()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let (exps_input, exps_output) =
        input::split(&rule_instr.input_hint, exps).expect("validated rule input hint");
    let fallthrough = fallthrough::prose_of_link(ctx, instr);
    // Detect outputs collected by an enclosing iteration
    let has_output = rule_instr
        .iter_instrs
        .iter()
        .flat_map(|iter_instr| &iter_instr.vars_bind)
        .any(|var| !var.id.node.starts_with('_'));
    // Apply paired relation hints when both sides are available
    let rule_body = if let (Some(hint_input), Some(hint_output)) =
        (&instr.hints.prose_in, &instr.hints.prose_out)
    {
        let prose_output =
            alternate(hint_output, &unindent_lines, &prose_of_exp, &exps_output, false);
        prose([
            text("Let "),
            text(document::ser_prose_in_link(&prose_output)),
            text(" be the result of "),
            subject_link(
                Subject::Relation(rule_instr.id.node.clone()),
                alternate(hint_input, &unindent_lines, &prose_of_exp, &exps_input, false),
            ),
        ])
    } else {
        prose([
            text("Let "),
            subject_link(
                Subject::Relation(rule_instr.id.node.clone()),
                code_prose(code_of_mixfix(&rule_instr.not_exp, &code_of_exp)),
            ),
        ])
    };
    // Wrap bindings that produce iterated outputs in open blocks
    if !has_output {
        ordered(
            level,
            prose([
                rule_body,
                prose_of_iterinstr_suffix(&rule_instr.iter_instrs),
                text("."),
                fallthrough,
            ]),
        )
    } else {
        render_iterinstrs(level, fallthrough, &rule_instr.iter_instrs, &|level| {
            unordered(level, prose([rule_body.clone(), text(".")]))
        })
    }
}

/// Renders a group-tier instruction.
fn render_instr_group(
    renderer: &mut Renderer<'_>,
    level: usize,
    ctx: &Context,
    singleton: bool,
    instr: &pl::Instr<pl::GroupInstr>,
    tier: &pl::GroupInstr,
) -> Rendered {
    match tier {
        pl::GroupInstr::Return(return_instr)
            if singleton
                && document::width_prose(&prose_of_exp(&return_instr.exp)) <= ADOC_WIDTH_SHORT =>
        {
            Rendered::Inline(prose([
                text(" return "),
                prose_of_exp(&return_instr.exp),
                text("."),
                fallthrough::prose_of_link(ctx, instr),
            ]))
        }
        pl::GroupInstr::Result(result_instr)
            if singleton
                && document::width_prose(&prose_of_result(
                    &instr.hints,
                    &result_instr.rel_signature,
                    &result_instr.exps_output,
                )) <= ADOC_WIDTH_SHORT =>
        {
            Rendered::Inline(prose([
                text(" "),
                prose_of_result(
                    &instr.hints,
                    &result_instr.rel_signature,
                    &result_instr.exps_output,
                ),
                fallthrough::prose_of_link(ctx, instr),
            ]))
        }
        pl::GroupInstr::Return(return_instr) => Rendered::Nested(ordered(
            level,
            prose([
                text("Return "),
                prose_of_exp(&return_instr.exp),
                text("."),
                fallthrough::prose_of_link(ctx, instr),
            ]),
        )),
        pl::GroupInstr::Result(result_instr) => Rendered::Nested(ordered(
            level,
            prose([
                document::capitalize_first_prose(prose_of_result(
                    &instr.hints,
                    &result_instr.rel_signature,
                    &result_instr.exps_output,
                )),
                fallthrough::prose_of_link(ctx, instr),
            ]),
        )),
        pl::GroupInstr::Rule(rule_instr) => {
            Rendered::Nested(render_rule_instr(level, ctx, instr, rule_instr))
        }
        pl::GroupInstr::Backtrack(backtrack_instr) => {
            let level_body = level + 1;
            Rendered::Nested(render_block_arms(
                renderer,
                level,
                ctx,
                &backtrack_instr.blocks,
                &|renderer, ctx_arm, arm| {
                    seq(arm.iter().map(|instr| {
                        render_instr(renderer, level_body, ctx_arm, render_instr_group, instr)
                    }))
                },
            ))
        }
    }
}

fn prose_of_group_dispatch(id_rel: &pl::Id, id_group: &pl::Id) -> Prose {
    prose([
        text("goto "),
        direct_link(
            fallthrough::anchor_of_group(&id_rel.node, &id_group.node),
            text(id_group.node.clone()),
        ),
    ])
}

/// Renders a dispatch-tier instruction.
fn render_instr_dispatch(
    renderer: &mut Renderer<'_>,
    level: usize,
    ctx: &Context,
    singleton: bool,
    _instr: &pl::Instr<pl::DispatchInstr>,
    tier: &pl::DispatchInstr,
) -> Rendered {
    match tier {
        pl::DispatchInstr::Group(group_instr) if singleton => Rendered::InlineGoto(prose([
            text(" "),
            prose_of_group_dispatch(&group_instr.id_rel, &group_instr.id_group),
        ])),
        pl::DispatchInstr::Group(group_instr) => Rendered::Nested(ordered(
            level,
            document::capitalize_first_prose(prose_of_group_dispatch(
                &group_instr.id_rel,
                &group_instr.id_group,
            )),
        )),
        pl::DispatchInstr::Route(route_instr) => {
            let level_body = level + 1;
            Rendered::Nested(render_block_arms(
                renderer,
                level,
                ctx,
                &route_instr.blocks,
                &|renderer, ctx_arm, arm| {
                    seq(arm.iter().map(|instr| {
                        render_instr(renderer, level_body, ctx_arm, render_instr_dispatch, instr)
                    }))
                },
            ))
        }
    }
}

/// Renders an otherwise dispatch group with its body inline.
fn render_dispatch_inline(
    renderer: &mut Renderer<'_>,
    level: usize,
    ctx: &Context,
    singleton: bool,
    instr: &pl::Instr<pl::DispatchInstr>,
    tier: &pl::DispatchInstr,
) -> Rendered {
    match tier {
        pl::DispatchInstr::Group(group_instr) => {
            // Select hinted prose or filled relation notation for the title
            let hint_opt = instr
                .hints
                .prose_in
                .as_ref()
                .or(instr.hints.prose_true.as_ref());
            let prose_title = hint_opt.map_or_else(
                || {
                    subject_link(
                        Subject::Relation(group_instr.id_rel.node.clone()),
                        prose_of_rel_title_math(
                            &group_instr.rel_signature,
                            &group_instr.exps_input,
                        ),
                    )
                },
                |hint| {
                    subject_link(
                        Subject::Relation(group_instr.id_rel.node.clone()),
                        alternate(
                            hint,
                            &|text_body| reindent_lines(0, text_body),
                            &prose_of_exp,
                            &group_instr.exps_input,
                            true,
                        ),
                    )
                },
            );
            // Render the group body below its linked title
            let block_head = ordered(level, prose([prose_title, text(":")]));
            Rendered::Nested(render_instrs(
                renderer,
                level + 1,
                Some(block_head),
                ctx,
                render_instr_group,
                &group_instr.block,
            ))
        }
        pl::DispatchInstr::Route(_) => {
            render_instr_dispatch(renderer, level, ctx, singleton, instr, tier)
        }
    }
}

// == Relation definitions

struct GroupRef<'a> {
    hints: &'a Hints,
    group: &'a pl::RuleGroupInstr,
}

/// Collects rule groups from a dispatch tree in document order.
fn collect_groups<'a>(block: &'a pl::DispatchBlock, groups: &mut Vec<GroupRef<'a>>) {
    for instr in block {
        match &instr.node.node {
            pl::InstrKind::If(if_instr) => collect_groups(&if_instr.block, groups),
            pl::InstrKind::Hold(hold_instr) => match &hold_instr.hold_case {
                pl::HoldCase::Both(block_l, block_r) => {
                    collect_groups(block_l, groups);
                    collect_groups(block_r, groups);
                }
                pl::HoldCase::Hold(block, _) | pl::HoldCase::NotHold(block, _) => {
                    collect_groups(block, groups);
                }
            },
            pl::InstrKind::Case(case_instr) => {
                for case in &case_instr.cases {
                    collect_groups(&case.block, groups);
                }
            }
            pl::InstrKind::CheckLetSub(check_instr) => collect_groups(&check_instr.block, groups),
            pl::InstrKind::CheckLetMatch(check_instr) => collect_groups(&check_instr.block, groups),
            pl::InstrKind::OptionGet(option_instr) => collect_groups(&option_instr.block, groups),
            pl::InstrKind::Tier(tier_instr) => match &tier_instr.tier {
                pl::DispatchInstr::Route(route_instr) => {
                    for arm in &route_instr.blocks {
                        collect_groups(arm, groups);
                    }
                }
                pl::DispatchInstr::Group(group) => {
                    groups.push(GroupRef { hints: &instr.hints, group });
                }
            },
            pl::InstrKind::Let(_) | pl::InstrKind::Debug(_) | pl::InstrKind::Destruct(_) => {}
        }
    }
}

/// Renders a rule group while sharing document counters.
fn render_rulegroup_inner(
    renderer: &mut Renderer<'_>,
    hints: &Hints,
    id_rel: &pl::Id,
    signature: &pl::RelSignature,
    exps: &[pl::Exp],
    block: &pl::GroupBlock,
) -> String {
    // Select hinted prose or filled relation notation for the title
    let hint_opt = hints.prose_in.as_ref().or(hints.prose_true.as_ref());
    let prose_title = hint_opt.map_or_else(
        || {
            subject_link(
                Subject::Relation(id_rel.node.clone()),
                prose_of_rel_title_math(signature, exps),
            )
        },
        |hint| {
            subject_link(
                Subject::Relation(id_rel.node.clone()),
                alternate(
                    hint,
                    &|text_body| reindent_lines(0, text_body),
                    &prose_of_exp,
                    exps,
                    true,
                ),
            )
        },
    );
    // Render the body with counters shared by the enclosing document
    let ctx = Context { namespace: id_rel.node.clone(), next: None };
    let block_body = render_instrs(renderer, 0, None, &ctx, render_instr_group, block);
    // Serialize the linked title and body as one fragment
    format!(
        "{}:\n{}",
        document::ser_prose_with_anchor(&prose_title, renderer.anchor),
        document::ser_block_with_anchor(&block_body, renderer.anchor),
    )
}

/// Renders one rule-group fragment using caller-supplied anchors.
pub fn render_rulegroup_with_anchor(
    hints: &Hints,
    _id_group: &pl::Id,
    id_rel: &pl::Id,
    signature: &pl::RelSignature,
    exps: &[pl::Exp],
    block: &pl::GroupBlock,
    anchor: &dyn Fn(&Subject) -> Option<String>,
) -> String {
    render_rulegroup_inner(&mut Renderer::new(anchor), hints, id_rel, signature, exps, block)
}

/// Renders one rule-group fragment with definition-name anchors.
pub fn render_rulegroup(
    hints: &Hints,
    id_group: &pl::Id,
    id_rel: &pl::Id,
    signature: &pl::RelSignature,
    exps: &[pl::Exp],
    block: &pl::GroupBlock,
) -> String {
    render_rulegroup_with_anchor(
        hints,
        id_group,
        id_rel,
        signature,
        exps,
        block,
        &document::subject_name,
    )
}

/// Renders dispatch while sharing the relation's counter state.
fn render_defined_rel_dispatch_inner(renderer: &mut Renderer<'_>, rel: &pl::DefinedRel) -> String {
    let ctx = Context { namespace: rel.id.node.clone(), next: None };
    format!(
        "{} dispatch:\n{}",
        rel.id.node,
        document::ser_block_with_anchor(
            &render_instrs(renderer, 0, None, &ctx, render_instr_dispatch, &rel.block),
            renderer.anchor,
        )
    )
}

/// Renders a relation dispatch fragment using caller-supplied anchors.
pub fn render_defined_rel_def_dispatch_with_anchor(
    rel: &pl::DefinedRel,
    anchor: &dyn Fn(&Subject) -> Option<String>,
) -> String {
    render_defined_rel_dispatch_inner(&mut Renderer::new(anchor), rel)
}

/// Renders a relation dispatch fragment with definition-name anchors.
pub fn render_defined_rel_def_dispatch(rel: &pl::DefinedRel) -> String {
    render_defined_rel_def_dispatch_with_anchor(rel, &document::subject_name)
}

/// Builds a complete relation block with source-compatible counter order.
fn render_defined_rel_block(
    renderer: &mut Renderer<'_>,
    hints: &Hints,
    rel: &pl::DefinedRel,
) -> Block {
    // Reserve an otherwise anchor only for a visible block
    let has_else = rel
        .block_else_opt
        .as_ref()
        .is_some_and(|block| !block.is_empty());
    let anchor_else = has_else.then(|| fallthrough::anchor_of_else(&rel.id.node));
    // Allocate counter-bearing fragments in OCaml right-to-left evaluation order
    let dispatch_text = render_defined_rel_dispatch_inner(renderer, rel);
    let ctx = Context { namespace: rel.id.node.clone(), next: None };
    let else_text = render_elseblock(
        renderer,
        anchor_else.as_deref(),
        &ctx,
        render_dispatch_inline,
        rel.block_else_opt.as_deref(),
    );
    // Extract each rule group from the dispatch tree in document order
    let mut groups = Vec::new();
    collect_groups(&rel.block, &mut groups);
    let groups_text = groups
        .into_iter()
        .map(|group_ref| {
            render_rulegroup_inner(
                renderer,
                group_ref.hints,
                &group_ref.group.id_rel,
                &group_ref.group.rel_signature,
                &group_ref.group.exps_input,
                &group_ref.group.block,
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    // Assemble fragments in their displayed order
    concat([
        render_rel_title_block(hints, &rel.id, &rel.rel_signature, &rel.exps_input),
        raw("\n\n"),
        raw(groups_text),
        raw(else_text),
        raw(format!("\n\n{dispatch_text}")),
    ])
}

/// Renders a defined relation using caller-supplied anchors.
pub fn render_defined_rel_def_with_anchor(
    hints: &Hints,
    rel: &pl::DefinedRel,
    anchor: &dyn Fn(&Subject) -> Option<String>,
) -> String {
    let mut renderer = Renderer::new(anchor);
    document::ser_block_with_anchor(&render_defined_rel_block(&mut renderer, hints, rel), anchor)
}

/// Renders a defined relation with definition-name anchors.
pub fn render_defined_rel_def(hints: &Hints, rel: &pl::DefinedRel) -> String {
    render_defined_rel_def_with_anchor(hints, rel, &document::subject_name)
}

/// Renders an external relation using caller-supplied anchors.
pub fn render_extern_rel_def_with_anchor(
    hints: &Hints,
    rel: &pl::ExternRel,
    anchor: &dyn Fn(&Subject) -> Option<String>,
) -> String {
    document::ser_block_with_anchor(
        &render_rel_title_block(hints, &rel.id, &rel.rel_signature, &rel.exps_input),
        anchor,
    )
}

/// Renders an external relation with definition-name anchors.
pub fn render_extern_rel_def(hints: &Hints, rel: &pl::ExternRel) -> String {
    render_extern_rel_def_with_anchor(hints, rel, &document::subject_name)
}

/// Renders an otherwise rule-group fragment using caller-supplied anchors.
pub fn render_rulegroup_else_with_anchor(
    id_rel: &pl::Id,
    block: &pl::DispatchBlock,
    anchor: &dyn Fn(&Subject) -> Option<String>,
) -> String {
    let mut renderer = Renderer::new(anchor);
    let ctx = Context { namespace: id_rel.node.clone(), next: None };
    render_elseblock(
        &mut renderer,
        Some(&fallthrough::anchor_of_else(&id_rel.node)),
        &ctx,
        render_dispatch_inline,
        Some(block),
    )
    .trim()
    .to_owned()
}

/// Renders an otherwise rule-group fragment with definition-name anchors.
pub fn render_rulegroup_else(id_rel: &pl::Id, block: &pl::DispatchBlock) -> String {
    render_rulegroup_else_with_anchor(id_rel, block, &document::subject_name)
}

// == Function definitions

/// Builds the standalone title form of a function.
fn render_func_title_block(
    hints: &Hints,
    id_func: &pl::Id,
    tparams: &[pl::TParam],
    params: &[pl::Param],
) -> Block {
    // Build the linked function name shared by both title forms
    let prose_title =
        subject_link(Subject::Function(id_func.node.clone()), text(string_of_defid(id_func)));
    // Select prose-hinted or signature notation output
    if let Some(hint) = hints.prose_in.as_ref().or(hints.prose_true.as_ref()) {
        concat([
            inline(prose([prose_title, text(":")])),
            raw("\n\n"),
            unordered(
                0,
                alternate(
                    hint,
                    &|text_body| reindent_lines(0, text_body),
                    &prose_of_param,
                    params,
                    true,
                ),
            ),
        ])
    } else {
        concat([
            inline(prose_title),
            raw(if tparams.is_empty() {
                String::new()
            } else {
                format!(
                    "<{}>",
                    tparams
                        .iter()
                        .map(Print::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }),
            raw(document::ser_code(&code_of_params(params))),
        ])
    }
}

/// Builds the linked inline header used before a body or table.
fn render_func_header_block(
    hints: &Hints,
    id_func: &pl::Id,
    tparams: &[pl::TParam],
    params: &[pl::Param],
) -> Block {
    // Serialize the selected title form inside the function link
    let prose_body = if let Some(hint) = hints.prose_in.as_ref().or(hints.prose_true.as_ref()) {
        text(document::ser_prose(&alternate(
            hint,
            &|text_body| reindent_lines(0, text_body),
            &prose_of_param,
            params,
            true,
        )))
    } else {
        text(format!(
            "{}{}{}",
            string_of_defid(id_func),
            if tparams.is_empty() {
                String::new()
            } else {
                format!(
                    "<{}>",
                    tparams
                        .iter()
                        .map(Print::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            },
            document::ser_code(&code_of_params(params)),
        ))
    };
    inline(subject_link(Subject::Function(id_func.node.clone()), prose_body))
}

/// Renders a function title using caller-supplied anchors.
pub fn render_func_title_with_anchor(
    hints: &Hints,
    id_func: &pl::Id,
    tparams: &[pl::TParam],
    params: &[pl::Param],
    anchor: &dyn Fn(&Subject) -> Option<String>,
) -> String {
    document::ser_block_with_anchor(
        &render_func_title_block(hints, id_func, tparams, params),
        anchor,
    )
}

/// Renders a function title with definition-name anchors.
pub fn render_func_title(
    hints: &Hints,
    id_func: &pl::Id,
    tparams: &[pl::TParam],
    params: &[pl::Param],
) -> String {
    render_func_title_with_anchor(hints, id_func, tparams, params, &document::subject_name)
}

/// Renders a function header using caller-supplied anchors.
pub fn render_func_header_with_anchor(
    hints: &Hints,
    id_func: &pl::Id,
    tparams: &[pl::TParam],
    params: &[pl::Param],
    anchor: &dyn Fn(&Subject) -> Option<String>,
) -> String {
    document::ser_block_with_anchor(
        &render_func_header_block(hints, id_func, tparams, params),
        anchor,
    )
}

/// Renders a function header with definition-name anchors.
pub fn render_func_header(
    hints: &Hints,
    id_func: &pl::Id,
    tparams: &[pl::TParam],
    params: &[pl::Param],
) -> String {
    render_func_header_with_anchor(hints, id_func, tparams, params, &document::subject_name)
}

/// Renders an external function using caller-supplied anchors.
pub fn render_extern_func_def_with_anchor(
    hints: &Hints,
    func: &pl::ExternFunc,
    anchor: &dyn Fn(&Subject) -> Option<String>,
) -> String {
    render_func_header_with_anchor(hints, &func.id, &func.tparams, &func.params, anchor)
}

/// Renders an external function with definition-name anchors.
pub fn render_extern_func_def(hints: &Hints, func: &pl::ExternFunc) -> String {
    render_extern_func_def_with_anchor(hints, func, &document::subject_name)
}

/// Renders a builtin function using caller-supplied anchors.
pub fn render_builtin_func_def_with_anchor(
    hints: &Hints,
    func: &pl::BuiltinFunc,
    anchor: &dyn Fn(&Subject) -> Option<String>,
) -> String {
    render_func_header_with_anchor(hints, &func.id, &func.tparams, &func.params, anchor)
}

/// Renders a builtin function with definition-name anchors.
pub fn render_builtin_func_def(hints: &Hints, func: &pl::BuiltinFunc) -> String {
    render_builtin_func_def_with_anchor(hints, func, &document::subject_name)
}

/// Builds a table function with argument and result columns.
fn render_table_func_block(hints: &Hints, func: &pl::TableFunc) -> Block {
    // Serialize each input tuple and result into table cells
    let rows_table = func
        .rows
        .iter()
        .map(|row| {
            vec![
                document::ser_code(&code_of_exps(&row.exps_input, ", ")),
                document::ser_code(&code_of_exp(&row.exp)),
            ]
        })
        .collect();
    // Assemble the linked header and table with one result column
    concat([
        render_func_header_block(hints, &func.id, &[], &func.params),
        raw(":\n"),
        Block::Table(
            func.params.len() + 1,
            vec![prose_of_params(&func.params), text("Result")],
            rows_table,
        ),
    ])
}

/// Renders a table function using caller-supplied anchors.
pub fn render_table_func_def_with_anchor(
    hints: &Hints,
    func: &pl::TableFunc,
    anchor: &dyn Fn(&Subject) -> Option<String>,
) -> String {
    document::ser_block_with_anchor(&render_table_func_block(hints, func), anchor)
}

/// Renders a table function with definition-name anchors.
pub fn render_table_func_def(hints: &Hints, func: &pl::TableFunc) -> String {
    render_table_func_def_with_anchor(hints, func, &document::subject_name)
}

/// Builds a function body and its optional otherwise clause.
fn render_defined_func_block(
    renderer: &mut Renderer<'_>,
    hints: &Hints,
    func: &pl::DefinedFunc,
) -> Block {
    // Reserve an otherwise anchor only for a visible block
    let has_else = func
        .block_else_opt
        .as_ref()
        .is_some_and(|block| !block.is_empty());
    let ctx = Context { namespace: func.id.node.clone(), next: None };
    // Choose the compact boolean, backtracking, or general body form
    let (block_body, anchor_else) = match func.block.as_slice() {
        [instr]
            if let pl::InstrKind::Tier(tier_instr) = &instr.node.node
                && let pl::GroupInstr::Return(return_instr) = &tier_instr.tier
                && matches!(return_instr.exp.node.node, pl::ExpKind::Bool(_)) =>
        {
            // Render a lone boolean return as inline prose
            (
                inline(prose([
                    text(" return "),
                    code_prose(code_of_exp(&return_instr.exp)),
                    text("."),
                ])),
                None,
            )
        }
        [instr]
            if has_else
                && matches!(
                    instr.node.node,
                    pl::InstrKind::Tier(pl::TierInstr { tier: pl::GroupInstr::Backtrack(_) })
                ) =>
        {
            // Preserve an otherwise target for a lone backtrack
            (
                render_instr(renderer, 0, &ctx, render_instr_group, instr),
                Some(fallthrough::anchor_of_else(&func.id.node)),
            )
        }
        _ => {
            // Render the general instruction sequence with an optional target
            (
                seq(func
                    .block
                    .iter()
                    .map(|instr| render_instr(renderer, 0, &ctx, render_instr_group, instr))),
                has_else.then(|| fallthrough::anchor_of_else(&func.id.node)),
            )
        }
    };
    // Append the otherwise clause after the selected body form
    concat([
        render_func_header_block(hints, &func.id, &func.tparams, &func.params),
        raw("\n\n"),
        block_body,
        raw(render_elseblock(
            renderer,
            anchor_else.as_deref(),
            &ctx,
            render_instr_group,
            func.block_else_opt.as_deref(),
        )),
    ])
}

/// Renders a defined function using caller-supplied anchors.
pub fn render_defined_func_def_with_anchor(
    hints: &Hints,
    func: &pl::DefinedFunc,
    anchor: &dyn Fn(&Subject) -> Option<String>,
) -> String {
    let mut renderer = Renderer::new(anchor);
    document::ser_block_with_anchor(&render_defined_func_block(&mut renderer, hints, func), anchor)
}

/// Renders a defined function with definition-name anchors.
pub fn render_defined_func_def(hints: &Hints, func: &pl::DefinedFunc) -> String {
    render_defined_func_def_with_anchor(hints, func, &document::subject_name)
}

// == Definitions

/// Renders one definition while sharing document counters.
fn render_def_inner(renderer: &mut Renderer<'_>, def: &pl::Def) -> Option<String> {
    match &def.node.node {
        pl::DefKind::Typ(_) | pl::DefKind::Var(_) => None,
        pl::DefKind::Rel(pl::RelDef::Extern(rel)) => Some(document::ser_block_with_anchor(
            &render_rel_title_block(&def.hints, &rel.id, &rel.rel_signature, &rel.exps_input),
            renderer.anchor,
        )),
        pl::DefKind::Rel(pl::RelDef::Defined(rel)) => Some(document::ser_block_with_anchor(
            &render_defined_rel_block(renderer, &def.hints, rel),
            renderer.anchor,
        )),
        pl::DefKind::MetaFunc(pl::MetaFuncDef::Extern(func)) => {
            Some(document::ser_block_with_anchor(
                &render_func_header_block(&def.hints, &func.id, &func.tparams, &func.params),
                renderer.anchor,
            ))
        }
        pl::DefKind::MetaFunc(pl::MetaFuncDef::Builtin(func)) => {
            Some(document::ser_block_with_anchor(
                &render_func_header_block(&def.hints, &func.id, &func.tparams, &func.params),
                renderer.anchor,
            ))
        }
        pl::DefKind::MetaFunc(pl::MetaFuncDef::Table(func)) => {
            Some(document::ser_block_with_anchor(
                &render_table_func_block(&def.hints, func),
                renderer.anchor,
            ))
        }
        pl::DefKind::MetaFunc(pl::MetaFuncDef::Defined(func)) => {
            Some(document::ser_block_with_anchor(
                &render_defined_func_block(renderer, &def.hints, func),
                renderer.anchor,
            ))
        }
    }
}

/// Renders one definition using caller-supplied anchors.
pub fn render_def_with_anchor(
    def: &pl::Def,
    anchor: &dyn Fn(&Subject) -> Option<String>,
) -> Option<String> {
    render_def_inner(&mut Renderer::new(anchor), def)
}

/// Renders one prose definition, omitting type and variable declarations.
pub fn render_def(def: &pl::Def) -> Option<String> {
    render_def_with_anchor(def, &document::subject_name)
}

/// Renders definitions with one shared fallthrough counter state.
pub fn render_defs(defs: &[pl::Def]) -> String {
    let mut renderer = Renderer::new(&document::subject_name);
    defs.iter()
        .filter_map(|def| render_def_inner(&mut renderer, def))
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Renders a complete prose specification.
pub fn render_spec(spec: &pl::Spec) -> String {
    render_defs(spec)
}
