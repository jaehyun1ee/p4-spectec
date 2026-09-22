//! EL syntax translated to semantic TeX documents
//!
//! `tex_of_def` renders a single definition;
//! `tex_of_defs` groups consecutive function clauses before rendering.
//! Expressions retain their precedence until nested in a parent document.
//! Definition layouts resolve at width 80 before serialization.

use num_traits::Signed;

use crate::lang::{
    common::{
        Iter,
        notation::atom::Atom as AtomKind,
        prim::{bool, num},
    },
    el::ast::*,
};

use super::{
    error::{Error, Result},
    precedence::{self, Assoc, Category, Side},
    render::Anchors,
    tex::{
        doc::{self, Alignment, Block, Delimiter, Doc, GridRow, Style, Symbol},
        layout, link, width,
    },
};

const WIDTH_LAYOUT: usize = 80;

// == Lexical rendering

fn tex_of_typid(id: &Id) -> Doc {
    Doc::Styled(Style::Mathsf, id.node.clone())
}

fn tex_of_defid(id: &Id) -> Doc {
    Doc::Styled(Style::Mathrm, id.node.clone())
}

/// Keeps signed integers visibly distinct from natural literals.
fn tex_of_number(op: NumOp, num: &Num) -> Doc {
    let int = num::to_int(num);
    let tex_sign = match num {
        num::Number::Nat(_) => Doc::Empty,
        num::Number::Int(_) if int.is_negative() => Doc::Fixed(Symbol::Minus),
        num::Number::Int(_) => Doc::Fixed(Symbol::Plus),
    };
    let int_abs = int.abs();
    let tex_abs = match op {
        NumOp::Dec => Doc::Decimal(int_abs),
        NumOp::Hex => Doc::Hexadecimal(int_abs),
    };
    doc::concat(vec![tex_sign, tex_abs])
}

fn tex_of_atom(atom: &Atom) -> Doc {
    use AtomKind as A;
    use Symbol as S;
    match &atom.node {
        A::Keyword(text) => Doc::Styled(Style::Mathsf, text.clone()),
        A::Tag(text) => Doc::Subscript(
            Box::new(Doc::ThinSpace),
            Box::new(Doc::Styled(Style::Mathsf, text.clone())),
        ),
        A::Operator(text) => Doc::Mathbin(Box::new(Doc::Styled(Style::Mathtt, text.clone()))),
        A::Sub => {
            Doc::Mathrel(Box::new(doc::concat(vec![Doc::Fixed(S::Less), Doc::Fixed(S::Colon)])))
        }
        A::Sup => {
            Doc::Mathrel(Box::new(doc::concat(vec![Doc::Fixed(S::Colon), Doc::Fixed(S::Greater)])))
        }
        A::Turnstile => Doc::Mathrel(Box::new(Doc::Fixed(S::Turnstile))),
        A::Tilesturn => Doc::Mathrel(Box::new(Doc::Fixed(S::Tilesturn))),
        A::Arrow | A::ArrowSub => Doc::Fixed(S::To),
        A::DoubleArrowSub => Doc::Fixed(S::Rightarrow),
        A::DoubleArrowLong => Doc::Fixed(S::Longrightarrow),
        A::SqArrow => Doc::Fixed(S::Hookrightarrow),
        A::SqArrowStar => {
            Doc::Superscript(Box::new(Doc::Fixed(S::Hookrightarrow)), Box::new(Doc::Fixed(S::Ast)))
        }
        A::Dot => Doc::Group(Box::new(Doc::Fixed(S::Dot))),
        A::Dot2 => Doc::Fixed(S::Dot2),
        A::Dot3 => Doc::Fixed(S::Ellipsis),
        A::Semicolon => Doc::Fixed(S::Semicolon),
        A::Colon => Doc::Fixed(S::Colon),
        A::ColonEq => {
            Doc::Mathrel(Box::new(doc::concat(vec![Doc::Fixed(S::Colon), Doc::Fixed(S::Equal)])))
        }
        A::Tilde2 => Doc::Fixed(S::Sim),
        A::Backslash => Doc::Fixed(S::Setminus),
        A::LAngle => Doc::Fixed(S::Less),
        A::RAngle => Doc::Fixed(S::Greater),
        A::LParen => Doc::Fixed(S::LeftParen),
        A::RParen => Doc::Fixed(S::RightParen),
        A::LBrack => Doc::Fixed(S::LeftBracket),
        A::RBrack => Doc::Fixed(S::RightBracket),
        A::LBrace => Doc::Fixed(S::LeftBrace),
        A::RBrace => Doc::Fixed(S::RightBrace),
    }
}

/// Pairs recognized bracket atoms and otherwise retains their literal notation.
fn tex_of_bracket(atom_l: &Atom, tex_body: Doc, atom_r: &Atom) -> Doc {
    let delimiter = match (&atom_l.node, &atom_r.node) {
        (AtomKind::LParen, AtomKind::RParen) => Delimiter::Paren,
        (AtomKind::LBrack, AtomKind::RBrack) => Delimiter::Bracket,
        (AtomKind::LBrace, AtomKind::RBrace) => Delimiter::Brace,
        (AtomKind::LAngle, AtomKind::RAngle) => Delimiter::Angle,
        _ => return doc::concat(vec![tex_of_atom(atom_l), tex_body, tex_of_atom(atom_r)]),
    };
    Doc::Delimited(delimiter, Box::new(tex_body))
}

fn tex_of_iter(iter: Iter) -> Doc {
    Doc::Fixed(match iter {
        Iter::Opt => Symbol::Question,
        Iter::List => Symbol::Ast,
    })
}

// == Layout and links

/// Offers an indented break before an operator only when all terms are visible.
fn tex_of_breakable_infix(tex_l: Doc, tex_op: Doc, tex_r: Doc) -> Doc {
    // Missing operands must not leave an empty indented continuation
    if doc::is_empty(&tex_l) || doc::is_empty(&tex_op) || doc::is_empty(&tex_r) {
        return doc::concat_spaced(vec![tex_l, tex_op, tex_r]);
    }

    // Nest only the continuation so the first line retains its original width
    let tex_continuation =
        doc::concat(vec![Doc::SoftBreak(doc::Soft::SoftSpace), tex_op, Doc::Space, tex_r]);
    let tex_continuation = doc::nest(4, tex_continuation);
    doc::layout_group(doc::concat(vec![tex_l, tex_continuation]))
}

fn tex_of_link(anchor: Option<&str>, doc: Doc) -> Result<Doc> {
    match anchor {
        None => Ok(doc),
        Some(anchor) => {
            let target = link::target_of_string(anchor)?;
            Ok(link::link_unowned_doc(&target, doc))
        }
    }
}

fn annotate(doc: Doc, text: &str) -> Doc {
    doc::concat_spaced(vec![doc, Doc::Quad, Doc::Styled(Style::Text, text.to_owned())])
}

// == Types

fn tex_of_typ(typ: &Typ) -> Doc {
    match typ {
        Typ::Plain(plain_typ) => tex_of_plaintyp(plain_typ),
        Typ::Notation(not_typ) => tex_of_nottyp(not_typ),
    }
}

fn tex_of_plaintyp(plain_typ: &PlainTyp) -> Doc {
    match &plain_typ.node {
        PlainTypKind::Bool => Doc::Styled(Style::Mathbb, "B".to_owned()),
        PlainTypKind::Num(num::Typ::Nat) => Doc::Styled(Style::Mathbb, "N".to_owned()),
        PlainTypKind::Num(num::Typ::Int) => Doc::Styled(Style::Mathbb, "Z".to_owned()),
        PlainTypKind::Text => Doc::Styled(Style::Mathbb, "T".to_owned()),
        PlainTypKind::Var(id, targs) => doc::concat(vec![tex_of_typid(id), tex_of_targs(targs)]),
        PlainTypKind::Paren(plain_typ) => {
            Doc::Delimited(Delimiter::Paren, Box::new(tex_of_plaintyp(plain_typ)))
        }
        PlainTypKind::Tuple(plain_typs) => {
            let docs = plain_typs.iter().map(tex_of_plaintyp).collect();
            let doc = doc::layout_group_soft_comma_separated(docs);
            Doc::Delimited(Delimiter::Paren, Box::new(doc))
        }
        PlainTypKind::Iter(plain_typ, iter) => {
            Doc::Superscript(Box::new(tex_of_plaintyp(plain_typ)), Box::new(tex_of_iter(*iter)))
        }
    }
}

fn tex_of_nottyp(not_typ: &NotTyp) -> Doc {
    match &not_typ.node {
        NotTypKind::Atom(atom) => tex_of_atom(atom),
        NotTypKind::Seq(typs) => tex_of_typs(typs),
        NotTypKind::Infix(typ_l, atom, typ_r) => tex_of_infix_typ(typ_l, atom, typ_r),
        NotTypKind::Brack(atom_l, typ, atom_r) => tex_of_bracket(atom_l, tex_of_typ(typ), atom_r),
    }
}

fn tex_of_typs(typs: &[Typ]) -> Doc {
    doc::concat_juxtaposed(typs.iter().map(tex_of_typ).collect())
}

/// Consumes the first right-hand type as an arrow subscript when present.
fn tex_of_infix_typ(typ_l: &Typ, atom: &Atom, typ_r: &Typ) -> Doc {
    let tex_l = tex_of_typ(typ_l);
    let tex_op = tex_of_atom(atom);

    // Plain infix notation retains its complete right operand
    if !matches!(atom.node, AtomKind::ArrowSub | AtomKind::DoubleArrowSub) {
        return doc::concat_spaced(vec![tex_l, tex_op, tex_of_typ(typ_r)]);
    }

    // Subscripted arrows consume one type from a sequence or the whole operand
    let (tex_sub, tex_r) = match typ_r {
        // A sequence tail remains to the right of the arrow
        Typ::Notation(NotTyp { node: NotTypKind::Seq(typs), .. }) => {
            let Some((typ_sub, typs)) = typs.split_first() else {
                return doc::concat_spaced(vec![tex_l, tex_op]);
            };
            (tex_of_typ(typ_sub), tex_of_typs(typs))
        }
        // A single operand supplies only the subscript
        typ_sub => (tex_of_typ(typ_sub), Doc::Empty),
    };
    let tex_op = Doc::Subscript(Box::new(tex_op), Box::new(tex_sub));
    doc::concat_spaced(vec![tex_l, tex_op, tex_r])
}

fn tex_of_targs(targs: &[Targ]) -> Doc {
    if targs.is_empty() {
        return Doc::Empty;
    }
    let docs = targs.iter().map(tex_of_plaintyp).collect();
    let doc = doc::layout_group_soft_comma_separated(docs);
    Doc::Delimited(Delimiter::Angle, Box::new(doc))
}

fn tex_of_deftyp(def_typ: &DefTyp) -> Doc {
    match &def_typ.node {
        DefTypKind::Plain(plain_typ) => tex_of_plaintyp(plain_typ),
        DefTypKind::Struct(typ_fields) => {
            let docs = typ_fields
                .iter()
                .map(|typ_field| {
                    let tex_atom = tex_of_atom(&typ_field.atom);
                    let tex_typ = tex_of_plaintyp(&typ_field.typ);
                    doc::concat_spaced(vec![tex_atom, tex_typ])
                })
                .collect();
            let doc = doc::layout_group_soft_comma_separated(docs);
            Doc::Delimited(Delimiter::Brace, Box::new(doc))
        }
        DefTypKind::Variant(typ_cases) => {
            let blocks = typ_cases
                .iter()
                .map(|typ_case| Block::Line(tex_of_typ(&typ_case.typ)))
                .collect();
            doc::gathered(blocks)
        }
    }
}

// - Type definitions

/// Aligns variant alternatives beneath their production operator.
fn tex_of_typ_def(id: &Id, tparams: &[TParam], def_typ: &DefTyp) -> Doc {
    let tex_l = doc::concat(vec![tex_of_typid(id), tex_of_tparams(tparams)]);
    let tex_production = Doc::Mathrel(Box::new(Doc::Fixed(Symbol::Production)));

    // Non-variant definitions keep their body on the production line
    let DefTypKind::Variant(typ_cases) = &def_typ.node else {
        return doc::concat_spaced(vec![tex_l, tex_production, tex_of_deftyp(def_typ)]);
    };

    // An empty variant denotes the empty set
    let Some((typ_case, typ_cases)) = typ_cases.split_first() else {
        return doc::concat_spaced(vec![tex_l, tex_production, Doc::Fixed(Symbol::EmptySet)]);
    };

    // Continue each alternative in the operator and body columns
    let mut rows = vec![vec![tex_l, tex_production, tex_of_typ(&typ_case.typ)]];
    for typ_case in typ_cases {
        let tex_alternative = Doc::Mathrel(Box::new(Doc::Fixed(Symbol::VerticalBar)));
        rows.push(vec![Doc::Empty, tex_alternative, tex_of_typ(&typ_case.typ)]);
    }
    Doc::Aligned(rows)
}

// == Meta-variables

/// Renders underscore-prefixed variables as `_` and `TC_0` as a subscript.
fn tex_of_varid(id: &Id) -> Doc {
    // Underscore-prefixed identifiers all denote an anonymous variable
    if id.node.starts_with('_') {
        return Doc::Styled(Style::Mathsf, "_".to_owned());
    }
    // Keep every suffix after the first underscore in one subscript
    match id.node.split_once('_') {
        // A plain identifier needs no subscript
        None => Doc::Styled(Style::Mathsf, id.node.clone()),
        // The remaining underscores belong to the subscript text
        Some((var, subscript)) => Doc::Subscript(
            Box::new(Doc::Styled(Style::Mathsf, var.to_owned())),
            Box::new(Doc::Styled(Style::Mathsf, subscript.to_owned())),
        ),
    }
}

// == Expressions

fn tex_of_exp(exp: &Exp, anchors: Option<&Anchors<'_>>) -> Result<Doc> {
    let (doc, _) = render_exp(exp, anchors)?;
    Ok(doc)
}

/// Renders an expression while retaining the category of its outermost operator.
fn render_exp(exp: &Exp, anchors: Option<&Anchors<'_>>) -> Result<(Doc, Category)> {
    use Category as C;
    match &exp.node {
        ExpKind::Bool(value) => Ok((Doc::Styled(Style::Mathsf, value.to_string()), C::Atomic)),
        ExpKind::Num(op, num) => Ok((tex_of_number(*op, num), C::Atomic)),
        ExpKind::Text(text) => Ok((Doc::Styled(Style::Texttt, format!("\"{text}\"")), C::Atomic)),
        ExpKind::Id(id) => Ok((tex_of_varid(id), C::Atomic)),
        ExpKind::Un(op, exp) => {
            let (tex, category) = render_exp(exp, anchors)?;
            let tex_exp = tex_of_nested_exp((C::Unary, Assoc::Right), Side::Right, (tex, category));
            Ok((doc::concat_spaced(vec![tex_of_unop(*op), tex_exp]), C::Unary))
        }
        ExpKind::Bin(exp_l, BinOp::Num(num::BinOp::Pow), exp_r) => {
            let (tex_l, category_l) = render_exp(exp_l, anchors)?;
            let (tex_r, category_r) = render_exp(exp_r, anchors)?;
            let prec = precedence::of_binop(BinOp::Num(num::BinOp::Pow));
            let tex_l = tex_of_nested_exp(prec, Side::Left, (tex_l, category_l));
            let tex_r = tex_of_nested_exp(prec, Side::Right, (tex_r, category_r));
            Ok((Doc::Superscript(Box::new(tex_l), Box::new(tex_r)), C::Power))
        }
        ExpKind::Bin(exp_l, op, exp_r) => {
            render_binary_exp(precedence::of_binop(*op), tex_of_binop(*op), exp_l, exp_r, anchors)
        }
        ExpKind::Cmp(exp_l, op, exp_r) => render_binary_exp(
            (C::Comparison, Assoc::Right),
            tex_of_cmpop(*op),
            exp_l,
            exp_r,
            anchors,
        ),
        ExpKind::Arith(exp) => render_exp(exp, anchors),
        ExpKind::Eps => Ok((Doc::Fixed(Symbol::Epsilon), C::Atomic)),
        ExpKind::List(exps) => {
            let docs = texs_of_exps(exps, anchors)?;
            let doc = doc::layout_group_soft_comma_separated(docs);
            Ok((Doc::Delimited(Delimiter::Bracket, Box::new(doc)), C::Atomic))
        }
        ExpKind::Cons(exp_l, exp_r) => {
            let tex_op = Doc::Mathbin(Box::new(Doc::Fixed(Symbol::DoubleColon)));
            render_binary_exp((C::Cons, Assoc::Right), tex_op, exp_l, exp_r, anchors)
        }
        ExpKind::Cat(exp_l, exp_r) => {
            let tex_op = Doc::Mathbin(Box::new(Doc::Fixed(Symbol::Cat)));
            render_binary_exp((C::Additive, Assoc::Left), tex_op, exp_l, exp_r, anchors)
        }
        ExpKind::Idx(exp_base, exp_idx) => {
            let tex_idx = tex_of_exp(exp_idx, anchors)?;
            let tex_suffix = Doc::Delimited(Delimiter::Bracket, Box::new(tex_idx));
            render_postfix_exp(exp_base, tex_suffix, anchors)
        }
        ExpKind::Slice(exp_base, exp_idx, exp_len) => {
            let tex_idx = tex_of_exp(exp_idx, anchors)?;
            let tex_len = tex_of_exp(exp_len, anchors)?;
            let tex_body = doc::concat_spaced(vec![tex_idx, Doc::Fixed(Symbol::Colon), tex_len]);
            let tex_suffix = Doc::Delimited(Delimiter::Bracket, Box::new(tex_body));
            render_postfix_exp(exp_base, tex_suffix, anchors)
        }
        ExpKind::Len(exp) => {
            let tex_exp = tex_of_exp(exp, anchors)?;
            Ok((Doc::Delimited(Delimiter::Bar, Box::new(tex_exp)), C::Unary))
        }
        ExpKind::Mem(exp_l, exp_r) => render_binary_exp(
            (C::Comparison, Assoc::Right),
            Doc::Fixed(Symbol::In),
            exp_l,
            exp_r,
            anchors,
        ),
        ExpKind::Str(exp_fields) => {
            let docs = exp_fields
                .iter()
                .map(|(atom, exp)| {
                    let tex_atom = tex_of_atom(atom);
                    let tex_exp = tex_of_exp(exp, anchors)?;
                    Ok(doc::concat_spaced(vec![tex_atom, tex_exp]))
                })
                .collect::<Result<Vec<_>>>()?;
            let doc = doc::layout_group_soft_comma_separated(docs);
            Ok((Doc::Delimited(Delimiter::Brace, Box::new(doc)), C::Atomic))
        }
        ExpKind::Dot(exp_base, atom) => {
            let tex_field = tex_of_atom(atom);
            // An invisible field preserves the preceding path without a dot
            if doc::is_empty(&tex_field) {
                return render_exp(exp_base, anchors);
            }
            let (tex_base, category_base) = render_exp(exp_base, anchors)?;
            let tex_base =
                tex_of_nested_exp((C::Postfix, Assoc::Left), Side::Left, (tex_base, category_base));
            Ok((Doc::Subscript(Box::new(tex_base), Box::new(tex_field)), C::Postfix))
        }
        ExpKind::Upd(exp_base, path, exp_field) => {
            let tex_path = tex_of_path(path, anchors)?;
            let tex_field = tex_of_exp(exp_field, anchors)?;
            let tex_body = doc::concat_spaced(vec![tex_path, Doc::Fixed(Symbol::Equal), tex_field]);
            let tex_suffix = Doc::Delimited(Delimiter::Bracket, Box::new(tex_body));
            render_postfix_exp(exp_base, tex_suffix, anchors)
        }
        ExpKind::Paren(exp) => {
            let tex_exp = tex_of_exp(exp, anchors)?;
            Ok((Doc::Delimited(Delimiter::Paren, Box::new(tex_exp)), C::Atomic))
        }
        ExpKind::Tuple(exps) => {
            let docs = texs_of_exps(exps, anchors)?;
            let doc = doc::layout_group_soft_comma_separated(docs);
            Ok((Doc::Delimited(Delimiter::Paren, Box::new(doc)), C::Atomic))
        }
        ExpKind::Call(id, targs, args) => {
            let anchor = anchors.and_then(|anchors| (anchors.func)(&id.node));
            let tex_name = tex_of_link(anchor.as_deref(), tex_of_defid(id))?;
            let tex_targs = tex_of_targs(targs);
            let tex_args = tex_of_args(args, anchors)?;
            Ok((doc::concat(vec![tex_name, tex_targs, tex_args]), C::Atomic))
        }
        ExpKind::Iter(exp, iter) => {
            let (tex, category) = render_exp(exp, anchors)?;
            let tex_base =
                tex_of_nested_exp((C::Postfix, Assoc::Left), Side::Left, (tex, category));
            let tex_iter = tex_of_iter(*iter);
            Ok((Doc::Superscript(Box::new(tex_base), Box::new(tex_iter)), C::Postfix))
        }
        ExpKind::Sub(exp, plain_typ) => {
            let (tex, category) = render_exp(exp, anchors)?;
            let tex_l = tex_of_nested_exp((C::Colon, Assoc::Left), Side::Left, (tex, category));
            let tex_op = Doc::Mathrel(Box::new(doc::concat(vec![
                Doc::Fixed(Symbol::Less),
                Doc::Fixed(Symbol::Colon),
            ])));
            let tex_r = tex_of_plaintyp(plain_typ);
            Ok((tex_of_breakable_infix(tex_l, tex_op, tex_r), C::Colon))
        }
        ExpKind::Atom(atom) => Ok((tex_of_atom(atom), C::Atomic)),
        ExpKind::Seq(exps) => render_seq_exp(exps, anchors),
        ExpKind::Infix(exp_l, atom, exp_r) => render_infix_exp(exp_l, atom, exp_r, anchors),
        ExpKind::Brack(atom_l, exp, atom_r) => {
            let tex_body = tex_of_exp(exp, anchors)?;
            Ok((tex_of_bracket(atom_l, tex_body, atom_r), C::Atomic))
        }
        ExpKind::Hole(_) => Err(Error::Hole(exp.span.clone())),
        ExpKind::Fuse(_, _) => Err(Error::Fuse(exp.span.clone())),
        ExpKind::Unparen(_) => Err(Error::Unparen(exp.span.clone())),
        ExpKind::Latex(_) => Err(Error::RawLatex(exp.span.clone())),
    }
}

fn texs_of_exps(exps: &[Exp], anchors: Option<&Anchors<'_>>) -> Result<Vec<Doc>> {
    exps.iter().map(|exp| tex_of_exp(exp, anchors)).collect()
}

// - Operand precedence

/// Parenthesizes a weaker operand or an equal-precedence associativity conflict.
fn tex_of_nested_exp(prec: (Category, Assoc), side: Side, (tex, category): (Doc, Category)) -> Doc {
    if precedence::needs_parentheses(prec.0, prec.1, side, category) {
        Doc::Delimited(Delimiter::Paren, Box::new(tex))
    } else {
        tex
    }
}

/// Preserves operand grouping and adds a break before the infix operator.
fn render_binary_exp(
    prec: (Category, Assoc),
    tex_op: Doc,
    exp_l: &Exp,
    exp_r: &Exp,
    anchors: Option<&Anchors<'_>>,
) -> Result<(Doc, Category)> {
    let (tex_l, category_l) = render_exp(exp_l, anchors)?;
    let (tex_r, category_r) = render_exp(exp_r, anchors)?;
    let tex_l = tex_of_nested_exp(prec, Side::Left, (tex_l, category_l));
    let tex_r = tex_of_nested_exp(prec, Side::Right, (tex_r, category_r));
    Ok((tex_of_breakable_infix(tex_l, tex_op, tex_r), prec.0))
}

/// Parenthesizes a postfix base before attaching its rendered suffix.
fn render_postfix_exp(
    exp_base: &Exp,
    tex_suffix: Doc,
    anchors: Option<&Anchors<'_>>,
) -> Result<(Doc, Category)> {
    let (tex_base, category_base) = render_exp(exp_base, anchors)?;
    let tex_base =
        tex_of_nested_exp((Category::Postfix, Assoc::Left), Side::Left, (tex_base, category_base));
    Ok((doc::concat(vec![tex_base, tex_suffix]), Category::Postfix))
}

/// Packs notation terms with thin spaces and right-operand parenthesization.
fn render_seq_exp(exps: &[Exp], anchors: Option<&Anchors<'_>>) -> Result<(Doc, Category)> {
    let docs = exps
        .iter()
        .map(|exp| {
            let (tex, category) = render_exp(exp, anchors)?;
            Ok(tex_of_nested_exp((Category::Sequence, Assoc::Left), Side::Right, (tex, category)))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok((doc::fill(0, Doc::ThinSpace, docs), Category::Sequence))
}

/// Renders arrow subscripts separately from the remaining right-hand expression.
fn render_infix_exp(
    exp_l: &Exp,
    atom: &Atom,
    exp_r: &Exp,
    anchors: Option<&Anchors<'_>>,
) -> Result<(Doc, Category)> {
    let prec = precedence::of_infix(&atom.node);
    let (tex_l, category_l) = render_exp(exp_l, anchors)?;
    let tex_op = tex_of_atom(atom);

    // Subscripted arrows consume the first right-hand term
    let ((tex_r, category_r), tex_op) =
        if matches!(atom.node, AtomKind::ArrowSub | AtomKind::DoubleArrowSub) {
            match &exp_r.node {
                // Preserve the sequence category even when its tail is empty
                ExpKind::Seq(exps) => match exps.split_first() {
                    Some((exp_sub, exps)) => {
                        let (tex_r, category_r) = render_seq_exp(exps, anchors)?;
                        let tex_sub = tex_of_exp(exp_sub, anchors)?;
                        ((tex_r, category_r), Doc::Subscript(Box::new(tex_op), Box::new(tex_sub)))
                    }
                    None => ((Doc::Empty, Category::Atomic), tex_op),
                },
                // A singleton right operand becomes only a subscript
                _ => {
                    let tex_sub = tex_of_exp(exp_r, anchors)?;
                    (
                        (Doc::Empty, Category::Atomic),
                        Doc::Subscript(Box::new(tex_op), Box::new(tex_sub)),
                    )
                }
            }
        } else {
            (render_exp(exp_r, anchors)?, tex_op)
        };

    // Apply the original operator precedence to the visible operands
    let tex_l = tex_of_nested_exp(prec, Side::Left, (tex_l, category_l));
    let tex_r = tex_of_nested_exp(prec, Side::Right, (tex_r, category_r));
    Ok((tex_of_breakable_infix(tex_l, tex_op, tex_r), prec.0))
}

// - Operators

fn tex_of_unop(op: UnOp) -> Doc {
    Doc::Fixed(match op {
        UnOp::Bool(bool::UnOp::Not) => Symbol::Neg,
        UnOp::Num(num::UnOp::Plus) => Symbol::Plus,
        UnOp::Num(num::UnOp::Minus) => Symbol::Minus,
    })
}

fn tex_of_binop(op: BinOp) -> Doc {
    Doc::Fixed(match op {
        BinOp::Bool(bool::BinOp::And) => Symbol::Land,
        BinOp::Bool(bool::BinOp::Or) => Symbol::Lor,
        BinOp::Bool(bool::BinOp::Impl) => Symbol::Rightarrow,
        BinOp::Bool(bool::BinOp::Equiv) => Symbol::Leftrightarrow,
        BinOp::Num(num::BinOp::Add) => Symbol::Plus,
        BinOp::Num(num::BinOp::Sub) => Symbol::Minus,
        BinOp::Num(num::BinOp::Mul) => Symbol::Cdot,
        BinOp::Num(num::BinOp::Div) => Symbol::Slash,
        BinOp::Num(num::BinOp::Mod) => Symbol::Bmod,
        BinOp::Num(num::BinOp::Pow) => unreachable!("power uses superscript rendering"),
    })
}

fn tex_of_cmpop(op: CmpOp) -> Doc {
    Doc::Fixed(match op {
        CmpOp::Bool(bool::CmpOp::Eq) => Symbol::Equal,
        CmpOp::Bool(bool::CmpOp::Ne) => Symbol::NotEqual,
        CmpOp::Num(num::CmpOp::Lt) => Symbol::Less,
        CmpOp::Num(num::CmpOp::Gt) => Symbol::Greater,
        CmpOp::Num(num::CmpOp::Le) => Symbol::LessEqual,
        CmpOp::Num(num::CmpOp::Ge) => Symbol::GreaterEqual,
    })
}

// == Paths

/// Renders update paths, omitting the dot before a root field.
fn tex_of_path(path: &Path, anchors: Option<&Anchors<'_>>) -> Result<Doc> {
    match &path.node {
        PathKind::Root => Ok(Doc::Empty),
        PathKind::Idx(path, exp_idx) => {
            let tex_path = tex_of_path(path, anchors)?;
            let tex_idx = tex_of_exp(exp_idx, anchors)?;
            let tex_suffix = Doc::Delimited(Delimiter::Bracket, Box::new(tex_idx));
            Ok(doc::concat(vec![tex_path, tex_suffix]))
        }
        PathKind::Slice(path, exp_idx, exp_len) => {
            let tex_path = tex_of_path(path, anchors)?;
            let tex_idx = tex_of_exp(exp_idx, anchors)?;
            let tex_len = tex_of_exp(exp_len, anchors)?;
            let tex_body = doc::concat_spaced(vec![tex_idx, Doc::Fixed(Symbol::Colon), tex_len]);
            let tex_suffix = Doc::Delimited(Delimiter::Bracket, Box::new(tex_body));
            Ok(doc::concat(vec![tex_path, tex_suffix]))
        }
        PathKind::Dot(path, atom) => {
            let tex_field = tex_of_atom(atom);
            // A root field has no preceding path to separate
            if matches!(path.node, PathKind::Root) {
                return Ok(tex_field);
            }
            let tex_path = tex_of_path(path, anchors)?;
            // An invisible field preserves the preceding path without a dot
            if doc::is_empty(&tex_field) {
                return Ok(tex_path);
            }
            Ok(doc::concat(vec![tex_path, Doc::Fixed(Symbol::Dot), tex_field]))
        }
    }
}

// == Parameters and arguments

fn tex_of_tparams(tparams: &[TParam]) -> Doc {
    if tparams.is_empty() {
        return Doc::Empty;
    }
    let docs = tparams.iter().map(tex_of_typid).collect();
    let doc = doc::layout_group_soft_comma_separated(docs);
    Doc::Delimited(Delimiter::Angle, Box::new(doc))
}

fn tex_of_arg(arg: &Arg, anchors: Option<&Anchors<'_>>) -> Result<Doc> {
    match &arg.node {
        ArgKind::Exp(exp) => tex_of_exp(exp, anchors),
        ArgKind::Def(id) => Ok(tex_of_defid(id)),
    }
}

fn tex_of_args(args: &[Arg], anchors: Option<&Anchors<'_>>) -> Result<Doc> {
    let docs = args
        .iter()
        .map(|arg| tex_of_arg(arg, anchors))
        .collect::<Result<Vec<_>>>()?;
    let doc = doc::layout_group_soft_comma_separated(docs);
    Ok(Doc::Delimited(Delimiter::Paren, Box::new(doc)))
}

fn tex_of_param(param: &Param) -> Doc {
    match &param.node {
        ParamKind::Exp(plain_typ) => tex_of_plaintyp(plain_typ),
        ParamKind::Def(id, tparams, params, plain_typ) => {
            tex_of_func_signature(id, tparams, params, plain_typ, None)
        }
    }
}

fn tex_of_params(params: &[Param]) -> Doc {
    let docs = params.iter().map(tex_of_param).collect();
    let doc = doc::layout_group_soft_comma_separated(docs);
    Doc::Delimited(Delimiter::Paren, Box::new(doc))
}

// == Premises

fn tex_of_prem(prem: &Prem, anchors: Option<&Anchors<'_>>) -> Result<Doc> {
    match &prem.node {
        PremKind::Var(VarPrem { id, plain_typ }) => Ok(doc::concat_spaced(vec![
            tex_of_varid(id),
            Doc::Fixed(Symbol::Colon),
            tex_of_plaintyp(plain_typ),
        ])),
        PremKind::Rule(RulePrem { id, exp }) => {
            let anchor = anchors.and_then(|anchors| (anchors.rel)(&id.node));
            let tex_exp = tex_of_exp(exp, anchors)?;
            tex_of_link(anchor.as_deref(), tex_exp)
        }
        PremKind::RuleNot(RuleNotPrem { id, exp }) => {
            let anchor = anchors.and_then(|anchors| (anchors.rel)(&id.node));
            let (tex, category) = render_exp(exp, anchors)?;
            let tex_exp =
                tex_of_nested_exp((Category::Unary, Assoc::Right), Side::Right, (tex, category));
            let tex_exp = tex_of_link(anchor.as_deref(), tex_exp)?;
            Ok(doc::concat_spaced(vec![Doc::Fixed(Symbol::Neg), tex_exp]))
        }
        PremKind::If(IfPrem { exp }) => tex_of_exp(exp, anchors),
        PremKind::Else => Ok(Doc::Styled(Style::Mathrm, "otherwise".to_owned())),
        PremKind::Iter(IterPrem { prem, iter }) => {
            let tex_prem = tex_of_prem(prem, anchors)?;
            let tex_base = if matches!(prem.node, PremKind::Iter(_)) {
                tex_prem
            } else {
                Doc::Delimited(Delimiter::Paren, Box::new(tex_prem))
            };
            Ok(Doc::Superscript(Box::new(tex_base), Box::new(tex_of_iter(*iter))))
        }
        PremKind::Debug(DebugPrem { exp }) => {
            let tex_exp = tex_of_exp(exp, anchors)?;
            Ok(doc::concat_spaced(vec![Doc::Styled(Style::Mathrm, "debug".to_owned()), tex_exp]))
        }
    }
}

fn texs_of_prems(prems: &[Prem], anchors: Option<&Anchors<'_>>) -> Result<Vec<Doc>> {
    let docs = prems
        .iter()
        .map(|prem| tex_of_prem(prem, anchors))
        .collect::<Result<Vec<_>>>()?;
    Ok(docs.into_iter().filter(|doc| !doc::is_empty(doc)).collect())
}

// == Relations

fn tex_of_rel_signature(id: &Id, not_typ: &NotTyp, annotation: Option<&str>) -> Doc {
    let tex = doc::concat_spaced(vec![
        tex_of_defid(id),
        Doc::Fixed(Symbol::Colon),
        tex_of_nottyp(not_typ),
    ]);
    match annotation {
        None => tex,
        Some(text) => annotate(tex, text),
    }
}

/// Places a rule label above its inference fraction and numbers multiple premises.
fn tex_of_rule(rule: &Rule, anchors: Option<&Anchors<'_>>) -> Result<Doc> {
    let RuleKind { id_rel, id_rule, exp, prems } = &rule.node;

    // A single premise needs no numbered gutter
    let mut docs = texs_of_prems(prems, anchors)?;
    let tex_numerator = if docs.len() == 1 { docs.remove(0) } else { doc::numbered(docs) };

    // Keep the full rule identifier in the badge above the inference rule
    let text = if id_rule.node.is_empty() {
        id_rel.node.clone()
    } else {
        format!("{}-{}", id_rel.node, id_rule.node)
    };
    let tex_conclusion = tex_of_exp(exp, anchors)?;
    let tex_fraction = Doc::Fraction(Box::new(tex_numerator), Box::new(tex_conclusion));
    let tex_fraction = doc::displaystyle(tex_fraction);
    let tex_fraction = layout::resolve(WIDTH_LAYOUT, &tex_fraction)?;
    let tex_badge = doc::badge(text);
    Ok(doc::left_stack(vec![tex_badge, tex_fraction]))
}

/// Separates multiple inference rules while retaining empty-group annotations.
fn tex_of_rulegroup(
    id_rel: &Id,
    id_group: &Id,
    rules: &[Rule],
    anchors: Option<&Anchors<'_>>,
) -> Result<Doc> {
    match rules {
        // An empty rule group remains visible as a named empty set
        [] => {
            let text = if id_group.node.is_empty() {
                id_rel.node.clone()
            } else {
                format!("{}-{}", id_rel.node, id_group.node)
            };
            let tex_name = Doc::Styled(Style::Mathrm, text);
            let tex = doc::concat_spaced(vec![
                tex_name,
                Doc::Fixed(Symbol::Colon),
                Doc::Fixed(Symbol::EmptySet),
            ]);
            Ok(annotate(tex, "rules"))
        }
        // A single rule has no surrounding gathered document
        [rule] => tex_of_rule(rule, anchors),
        // Multiple rules receive one gap between their blocks
        rules => {
            let mut blocks = Vec::new();
            for rule in rules {
                if !blocks.is_empty() {
                    blocks.push(Block::Gap);
                }
                blocks.push(Block::Line(tex_of_rule(rule, anchors)?));
            }
            Ok(doc::gathered(blocks))
        }
    }
}

// == Meta-functions

fn tex_of_func_signature(
    id: &Id,
    tparams: &[TParam],
    params: &[Param],
    plain_typ: &PlainTyp,
    annotation: Option<&str>,
) -> Doc {
    let tex_head =
        doc::concat(vec![tex_of_defid(id), tex_of_tparams(tparams), tex_of_params(params)]);
    let tex =
        doc::concat_spaced(vec![tex_head, Doc::Fixed(Symbol::Colon), tex_of_plaintyp(plain_typ)]);
    match annotation {
        None => tex,
        Some(text) => annotate(tex, text),
    }
}

/// Aligns equations together, with wide conditions spanning beneath each clause.
fn tex_of_funcs(defs: &[&FuncDef], anchors: Option<&Anchors<'_>>) -> Result<Doc> {
    let mut rows = Vec::new();
    let mut has_condition_below = false;

    // Keep each clause's condition next to its equation before adding the gap
    for def in defs {
        if !rows.is_empty() {
            rows.push(GridRow::Gap);
        }
        let (docs, condition) = layout_func(def, anchors)?;
        rows.push(GridRow::Cells(docs));
        if let Some(doc) = condition {
            has_condition_below = true;
            rows.push(GridRow::Spanning(doc));
        }
    }

    // Multiple clauses use a grid; a compact single equation uses aligned
    let doc = if defs.len() > 1 {
        doc::grid(vec![Alignment::Left, Alignment::Center, Alignment::Left], rows)?
    } else if has_condition_below {
        doc::grid(vec![Alignment::Right, Alignment::Center, Alignment::Left], rows)?
    } else {
        let rows = rows
            .into_iter()
            .filter_map(|row| match row {
                GridRow::Cells(docs) => Some(docs),
                _ => None,
            })
            .collect();
        Doc::Aligned(rows)
    };
    layout::resolve(WIDTH_LAYOUT, &doc)
}

/// Moves conditions below equations only when their inline form exceeds width 80.
fn layout_func(def: &FuncDef, anchors: Option<&Anchors<'_>>) -> Result<(Vec<Doc>, Option<Doc>)> {
    let tex_name = tex_of_defid(&def.id);
    let tex_tparams = tex_of_tparams(&def.tparams);
    let tex_args = tex_of_args(&def.args, anchors)?;
    let tex_l = doc::concat(vec![tex_name, tex_tparams, tex_args]);
    let tex_body = tex_of_exp(&def.exp, anchors)?;
    let docs_prem = texs_of_prems(&def.prems, anchors)?;

    // Measure the full inline equation before choosing a continuation layout
    let tex_prem = match docs_prem.as_slice() {
        [] => Doc::Empty,
        [doc] => doc.clone(),
        docs => doc::numbered(docs.to_vec()),
    };
    let tex_condition = if docs_prem.is_empty() {
        Doc::Empty
    } else {
        doc::concat_juxtaposed(vec![Doc::Styled(Style::Text, "if".to_owned()), tex_prem.clone()])
    };
    let tex_r = if doc::is_empty(&tex_condition) {
        tex_body.clone()
    } else {
        doc::concat_spaced(vec![tex_body.clone(), Doc::Quad, tex_condition])
    };
    let docs_inline = vec![tex_l.clone(), Doc::Fixed(Symbol::Equal), tex_r];
    if width::flat(&Doc::Aligned(vec![docs_inline.clone()])) <= WIDTH_LAYOUT {
        return Ok((docs_inline, None));
    }

    // Resolve the two sides before placing a condition on its own spanning row
    let tex_l = layout::resolve(WIDTH_LAYOUT, &tex_l)?;
    let tex_body = layout::resolve(WIDTH_LAYOUT, &tex_body)?;
    let docs = vec![tex_l, Doc::Fixed(Symbol::Equal), tex_body];
    if docs_prem.is_empty() {
        return Ok((docs, None));
    }

    // Reserve the prefix width when breaking the premise block
    let tex_prefix =
        doc::concat(vec![Doc::Quad, Doc::Styled(Style::Text, "if".to_owned()), Doc::ThinSpace]);
    let width_prem = WIDTH_LAYOUT - width::flat(&tex_prefix);
    let tex_prem = layout::resolve(width_prem, &tex_prem)?;
    let tex_condition = doc::layout_group(doc::concat(vec![tex_prefix, tex_prem]));
    Ok((docs, Some(tex_condition)))
}

/// Renders table patterns and results in aligned mapsto rows.
fn tex_of_table(id: &Id, rows: &[TableRow], anchors: Option<&Anchors<'_>>) -> Result<Doc> {
    // Empty tables retain their name and declaration category
    if rows.is_empty() {
        let tex = doc::concat_spaced(vec![
            tex_of_defid(id),
            Doc::Fixed(Symbol::Colon),
            Doc::Fixed(Symbol::EmptySet),
        ]);
        return Ok(annotate(tex, "table"));
    }

    // Every pattern is parenthesized as an argument to the table function
    let rows = rows
        .iter()
        .map(|row| {
            let tex_pattern = tex_of_exp(&row.node.exp_pattern, anchors)?;
            let tex_pattern = Doc::Delimited(Delimiter::Paren, Box::new(tex_pattern));
            let tex_l = doc::concat(vec![tex_of_defid(id), tex_pattern]);
            let tex_r = tex_of_exp(&row.node.exp_body, anchors)?;
            Ok(vec![tex_l, Doc::Fixed(Symbol::Mapsto), tex_r])
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Doc::Aligned(rows))
}

// == Definitions

/// Renders one definition, ignoring presentation hints in canonical output.
pub(super) fn tex_of_def(def: &Def, anchors: Option<&Anchors<'_>>) -> Result<Doc> {
    match &def.node {
        DefKind::ExternSyntax(ExternSyntaxDef { id, .. }) => {
            Ok(annotate(tex_of_typid(id), "external syntax"))
        }
        DefKind::Syntax(SyntaxDef { entries }) => {
            let tex = if entries.is_empty() {
                Doc::Fixed(Symbol::EmptySet)
            } else {
                let docs = entries
                    .iter()
                    .map(|entry| {
                        doc::concat(vec![tex_of_typid(&entry.id), tex_of_tparams(&entry.tparams)])
                    })
                    .collect();
                doc::concat_comma_separated(docs)
            };
            Ok(annotate(tex, "syntax"))
        }
        DefKind::Typ(TypDef { id, tparams, def_typ, .. }) => {
            Ok(tex_of_typ_def(id, tparams, def_typ))
        }
        DefKind::Var(VarDef { id, plain_typ, .. }) => Ok(doc::concat_spaced(vec![
            tex_of_varid(id),
            Doc::Fixed(Symbol::Colon),
            tex_of_plaintyp(plain_typ),
        ])),
        DefKind::ExternRel(ExternRelDef { id, not_typ, .. }) => {
            Ok(tex_of_rel_signature(id, not_typ, Some("external")))
        }
        DefKind::Rel(RelDef { id, not_typ, .. }) => Ok(tex_of_rel_signature(id, not_typ, None)),
        DefKind::RuleGroup(RuleGroupDef { relid, groupid, rules }) => {
            tex_of_rulegroup(relid, groupid, rules, anchors)
        }
        DefKind::ExternDec(ExternDecDef { id, tparams, params, plain_typ, .. }) => {
            Ok(tex_of_func_signature(id, tparams, params, plain_typ, Some("external")))
        }
        DefKind::BuiltinDec(BuiltinDecDef { id, tparams, params, plain_typ, .. }) => {
            Ok(tex_of_func_signature(id, tparams, params, plain_typ, Some("builtin")))
        }
        DefKind::TableDec(TableDecDef { id, params, plain_typ, .. }) => {
            Ok(tex_of_func_signature(id, &[], params, plain_typ, Some("table")))
        }
        DefKind::FuncDec(FuncDecDef { id, tparams, params, plain_typ, .. }) => {
            Ok(tex_of_func_signature(id, tparams, params, plain_typ, None))
        }
        DefKind::FuncDef(def) => tex_of_funcs(&[def], anchors),
        DefKind::TableDef(TableDef { id, rows }) => tex_of_table(id, rows, anchors),
        DefKind::Sep => Ok(Doc::Empty),
    }
}

/// Groups only adjacent clauses of the same function, preserving separators.
pub(super) fn tex_of_defs(mut defs: &[Def], anchors: Option<&Anchors<'_>>) -> Result<Doc> {
    let mut blocks = Vec::new();

    // Consume a contiguous clause group or a single definition at each step
    while let Some((def, defs_rest)) = defs.split_first() {
        match &def.node {
            // Separators interrupt clause grouping even if they render no text
            DefKind::Sep => {
                blocks.push(Block::Gap);
                defs = defs_rest;
            }
            // Share alignment across consecutive clauses with the same identifier
            DefKind::FuncDef(def_func) => {
                let mut defs_func = vec![def_func];
                defs = defs_rest;
                while let Some((def, defs_rest)) = defs.split_first() {
                    let DefKind::FuncDef(def_next) = &def.node else { break };
                    if def_next.id.node != def_func.id.node {
                        break;
                    }
                    defs_func.push(def_next);
                    defs = defs_rest;
                }
                let tex = tex_of_funcs(&defs_func, anchors)?;
                blocks.push(Block::Line(tex));
            }
            // Every other definition keeps its source position in the document
            _ => {
                let tex = tex_of_def(def, anchors)?;
                blocks.push(Block::Line(tex));
                defs = defs_rest;
            }
        }
    }

    // Empty and separator-only specifications produce no gathered wrapper
    if blocks.iter().all(|block| matches!(block, Block::Gap)) {
        Ok(Doc::Empty)
    } else {
        Ok(doc::gathered(blocks))
    }
}
