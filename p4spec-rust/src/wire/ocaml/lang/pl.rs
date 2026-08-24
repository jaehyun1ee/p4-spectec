use serde_json::{Value, json};

use crate::{
    lang::{
        hints::alter,
        pl::{
            annot,
            ast::{self, ArgKind, ExpKind, PathKind},
        },
    },
    wire::ocaml::{atom::AtomPhraseCodec, mixfix, source},
};

use super::{
    super::{DecodeError, array, boolean, field, object, on_codec_stack, string, variant},
    el, il, xl,
};

pub struct ExpCodec;

impl ExpCodec {
    pub fn decode(value: &Value) -> Result<ast::Exp, DecodeError> {
        on_codec_stack(|| decode_exp(value))
    }

    pub fn encode(exp: &ast::Exp) -> Value {
        on_codec_stack(|| encode_exp(exp))
    }
}

pub struct PathCodec;

impl PathCodec {
    pub fn decode(value: &Value) -> Result<ast::Path, DecodeError> {
        on_codec_stack(|| decode_path(value))
    }

    pub fn encode(path: &ast::Path) -> Value {
        on_codec_stack(|| encode_path(path))
    }
}

fn decode_option<T>(
    value: &Value,
    decode: impl FnOnce(&Value) -> Result<T, DecodeError>,
) -> Result<Option<T>, DecodeError> {
    if value.is_null() {
        Ok(None)
    } else {
        Ok(Some(decode(value)?))
    }
}

fn encode_option<T>(value: Option<&T>, encode: impl FnOnce(&T) -> Value) -> Value {
    value.map_or(Value::Null, encode)
}

fn decode_alter(value: &Value) -> Result<alter::T, DecodeError> {
    let (tag, fields) = variant(value)?;
    match (tag, fields) {
        ("TextH", [text]) => Ok(alter::T::TextH(string(text)?.to_owned())),
        ("AtomH", [atom]) => Ok(alter::T::AtomH(AtomPhraseCodec::decode(atom)?)),
        ("SeqH", [hints]) => Ok(alter::T::SeqH(il::decode_list(hints, decode_alter)?)),
        ("BrackH", [left, hint, right]) => Ok(alter::T::BrackH(
            AtomPhraseCodec::decode(left)?,
            Box::new(decode_alter(hint)?),
            AtomPhraseCodec::decode(right)?,
        )),
        ("HoleH", [hole]) => {
            let (tag, fields) = variant(hole)?;
            match (tag, fields) {
                ("Next", []) => Ok(alter::T::HoleH(alter::Hole::Next)),
                ("Num", [index]) => Ok(alter::T::HoleH(alter::Hole::Num(super::super::integer(
                    index,
                )?))),
                ("Next" | "Num", _) => Err(DecodeError::Expected("valid PL alter hole arity")),
                (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
            }
        }
        ("FuseH", [left, right]) => Ok(alter::T::FuseH(
            Box::new(decode_alter(left)?),
            Box::new(decode_alter(right)?),
        )),
        ("OtherH", [exp]) => Ok(alter::T::OtherH(el::decode_exp(exp)?)),
        ("TextH" | "AtomH" | "SeqH" | "BrackH" | "HoleH" | "FuseH" | "OtherH", _) => {
            Err(DecodeError::Expected("valid PL alter hint arity"))
        }
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

fn encode_alter(hint: &alter::T) -> Value {
    match hint {
        alter::T::TextH(text) => json!(["TextH", text]),
        alter::T::AtomH(atom) => json!(["AtomH", AtomPhraseCodec::encode(atom)]),
        alter::T::SeqH(hints) => json!(["SeqH", il::encode_list(hints, encode_alter)]),
        alter::T::BrackH(left, hint, right) => json!([
            "BrackH",
            AtomPhraseCodec::encode(left),
            encode_alter(hint),
            AtomPhraseCodec::encode(right)
        ]),
        alter::T::HoleH(alter::Hole::Next) => json!(["HoleH", ["Next"]]),
        alter::T::HoleH(alter::Hole::Num(index)) => json!(["HoleH", ["Num", index]]),
        alter::T::FuseH(left, right) => json!(["FuseH", encode_alter(left), encode_alter(right)]),
        alter::T::OtherH(exp) => json!(["OtherH", el::encode_exp(exp)]),
    }
}

fn decode_hints(value: &Value) -> Result<annot::Hints, DecodeError> {
    let value = object(value)?;
    Ok(annot::Hints {
        prose: decode_option(field(value, "prose")?, decode_alter)?,
        prose_in: decode_option(field(value, "prose_in")?, decode_alter)?,
        prose_out: decode_option(field(value, "prose_out")?, decode_alter)?,
        prose_true: decode_option(field(value, "prose_true")?, decode_alter)?,
        prose_false: decode_option(field(value, "prose_false")?, decode_alter)?,
        prose_fields: decode_option(field(value, "prose_fields")?, |value| {
            il::decode_list(value, |value| Ok(string(value)?.to_owned()))
        })?,
        prose_input_exps: decode_option(field(value, "prose_input_exps")?, |value| {
            il::decode_list(value, il::decode_exp)
        })?,
        prose_output_exps: decode_option(field(value, "prose_output_exps")?, |value| {
            il::decode_list(value, il::decode_exp)
        })?,
    })
}

fn encode_hints(hints: &annot::Hints) -> Value {
    json!({
        "prose": encode_option(hints.prose.as_ref(), encode_alter),
        "prose_in": encode_option(hints.prose_in.as_ref(), encode_alter),
        "prose_out": encode_option(hints.prose_out.as_ref(), encode_alter),
        "prose_true": encode_option(hints.prose_true.as_ref(), encode_alter),
        "prose_false": encode_option(hints.prose_false.as_ref(), encode_alter),
        "prose_fields": encode_option(hints.prose_fields.as_ref(), |fields| json!(fields)),
        "prose_input_exps": encode_option(hints.prose_input_exps.as_ref(), |exps| il::encode_list(exps, il::encode_exp)),
        "prose_output_exps": encode_option(hints.prose_output_exps.as_ref(), |exps| il::encode_list(exps, il::encode_exp)),
    })
}

fn decode_exp(value: &Value) -> Result<ast::Exp, DecodeError> {
    let value = object(value)?;
    Ok(annot::T {
        node: decode_exp_node(field(value, "node")?)?,
        hints: decode_hints(field(value, "hints")?)?,
    })
}

fn encode_exp(exp: &ast::Exp) -> Value {
    json!({"node": encode_exp_node(&exp.node), "hints": encode_hints(&exp.hints)})
}

fn decode_exp_node(value: &Value) -> Result<ast::ExpNode, DecodeError> {
    let (kind, ty, span) = source::decode_annotated(value, decode_exp_kind, il::decode_typ_kind)?;
    Ok(ast::ExpNode { kind, ty, span })
}

fn encode_exp_node(exp: &ast::ExpNode) -> Value {
    source::encode_annotated(
        &exp.kind,
        &exp.ty,
        &exp.span,
        encode_exp_kind,
        il::encode_typ_kind,
    )
}

fn decode_exp_kind(value: &Value) -> Result<ExpKind, DecodeError> {
    let (tag, fields) = variant(value)?;
    match (tag, fields) {
        ("BoolE", [value]) => Ok(ExpKind::BoolE(boolean(value)?)),
        ("NumE", [num]) => Ok(ExpKind::NumE(xl::decode_num(num)?)),
        ("TextE", [text]) => Ok(ExpKind::TextE(string(text)?.to_owned())),
        ("VarE", [id]) => Ok(ExpKind::VarE(il::decode_id(id)?)),
        ("UnE", [op, typ, exp]) => Ok(ExpKind::UnE(
            il::decode_un_op(op)?,
            il::decode_op_typ(typ)?,
            Box::new(decode_exp(exp)?),
        )),
        ("BinE", [op, typ, left, right]) => Ok(ExpKind::BinE(
            il::decode_bin_op(op)?,
            il::decode_op_typ(typ)?,
            Box::new(decode_exp(left)?),
            Box::new(decode_exp(right)?),
        )),
        ("CmpE", [op, typ, left, right]) => Ok(ExpKind::CmpE(
            il::decode_cmp_op(op)?,
            il::decode_op_typ(typ)?,
            Box::new(decode_exp(left)?),
            Box::new(decode_exp(right)?),
        )),
        ("UpCastE", [typ, exp]) => Ok(ExpKind::UpCastE(
            il::decode_typ(typ)?,
            Box::new(decode_exp(exp)?),
        )),
        ("DownCastE", [typ, exp]) => Ok(ExpKind::DownCastE(
            il::decode_typ(typ)?,
            Box::new(decode_exp(exp)?),
        )),
        ("SubE", [exp, typ, subcheck]) => Ok(ExpKind::SubE(
            Box::new(decode_exp(exp)?),
            il::decode_typ(typ)?,
            Box::new(il::decode_subcheck(subcheck)?),
        )),
        ("MatchE", [exp, pattern]) => Ok(ExpKind::MatchE(
            Box::new(decode_exp(exp)?),
            il::decode_pattern(pattern)?,
        )),
        ("TupleE", [exps]) => Ok(ExpKind::TupleE(il::decode_list(exps, decode_exp)?)),
        ("CaseE", [exp]) => Ok(ExpKind::CaseE(Box::new(mixfix::decode(exp, decode_exp)?))),
        ("StrE", [fields]) => Ok(ExpKind::StrE(il::decode_list(
            fields,
            |value| match array(value)? {
                [atom, exp] => Ok((AtomPhraseCodec::decode(atom)?, decode_exp(exp)?)),
                _ => Err(DecodeError::Expected("PL structure field pair")),
            },
        )?)),
        ("OptE", [exp]) => Ok(ExpKind::OptE(decode_option(exp, |exp| {
            Ok(Box::new(decode_exp(exp)?))
        })?)),
        ("ListE", [exps]) => Ok(ExpKind::ListE(il::decode_list(exps, decode_exp)?)),
        ("ConsE", [left, right]) => Ok(ExpKind::ConsE(
            Box::new(decode_exp(left)?),
            Box::new(decode_exp(right)?),
        )),
        ("CatE", [left, right]) => Ok(ExpKind::CatE(
            Box::new(decode_exp(left)?),
            Box::new(decode_exp(right)?),
        )),
        ("MemE", [left, right]) => Ok(ExpKind::MemE(
            Box::new(decode_exp(left)?),
            Box::new(decode_exp(right)?),
        )),
        ("LenE", [exp]) => Ok(ExpKind::LenE(Box::new(decode_exp(exp)?))),
        ("DotE", [exp, atom]) => Ok(ExpKind::DotE(
            Box::new(decode_exp(exp)?),
            AtomPhraseCodec::decode(atom)?,
        )),
        ("IdxE", [base, index]) => Ok(ExpKind::IdxE(
            Box::new(decode_exp(base)?),
            Box::new(decode_exp(index)?),
        )),
        ("SliceE", [base, left, right]) => Ok(ExpKind::SliceE(
            Box::new(decode_exp(base)?),
            Box::new(decode_exp(left)?),
            Box::new(decode_exp(right)?),
        )),
        ("UpdE", [base, path, exp]) => Ok(ExpKind::UpdE(
            Box::new(decode_exp(base)?),
            Box::new(decode_path(path)?),
            Box::new(decode_exp(exp)?),
        )),
        ("CallE", [id, targs, args]) => Ok(ExpKind::CallE(
            il::decode_id(id)?,
            il::decode_list(targs, il::decode_targ)?,
            il::decode_list(args, decode_arg)?,
        )),
        ("IterE", [exp, iter]) => Ok(ExpKind::IterE(
            Box::new(decode_exp(exp)?),
            il::decode_iter_exp(iter)?,
        )),
        (
            "BoolE" | "NumE" | "TextE" | "VarE" | "UnE" | "BinE" | "CmpE" | "UpCastE" | "DownCastE"
            | "SubE" | "MatchE" | "TupleE" | "CaseE" | "StrE" | "OptE" | "ListE" | "ConsE" | "CatE"
            | "MemE" | "LenE" | "DotE" | "IdxE" | "SliceE" | "UpdE" | "CallE" | "IterE",
            _,
        ) => Err(DecodeError::Expected("valid PL expression arity")),
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

fn encode_exp_kind(exp: &ExpKind) -> Value {
    match exp {
        ExpKind::BoolE(value) => json!(["BoolE", value]),
        ExpKind::NumE(num) => json!(["NumE", xl::encode_num(num)]),
        ExpKind::TextE(text) => json!(["TextE", text]),
        ExpKind::VarE(id) => json!(["VarE", il::encode_id(id)]),
        ExpKind::UnE(op, typ, exp) => json!([
            "UnE",
            il::encode_un_op(*op),
            il::encode_op_typ(*typ),
            encode_exp(exp)
        ]),
        ExpKind::BinE(op, typ, left, right) => json!([
            "BinE",
            il::encode_bin_op(*op),
            il::encode_op_typ(*typ),
            encode_exp(left),
            encode_exp(right)
        ]),
        ExpKind::CmpE(op, typ, left, right) => json!([
            "CmpE",
            il::encode_cmp_op(*op),
            il::encode_op_typ(*typ),
            encode_exp(left),
            encode_exp(right)
        ]),
        ExpKind::UpCastE(typ, exp) => json!(["UpCastE", il::encode_typ(typ), encode_exp(exp)]),
        ExpKind::DownCastE(typ, exp) => json!(["DownCastE", il::encode_typ(typ), encode_exp(exp)]),
        ExpKind::SubE(exp, typ, subcheck) => json!([
            "SubE",
            encode_exp(exp),
            il::encode_typ(typ),
            il::encode_subcheck(subcheck)
        ]),
        ExpKind::MatchE(exp, pattern) => {
            json!(["MatchE", encode_exp(exp), il::encode_pattern(pattern)])
        }
        ExpKind::TupleE(exps) => json!(["TupleE", il::encode_list(exps, encode_exp)]),
        ExpKind::CaseE(exp) => json!(["CaseE", mixfix::encode(exp, encode_exp)]),
        ExpKind::StrE(fields) => json!([
            "StrE",
            fields
                .iter()
                .map(|(atom, exp)| json!([AtomPhraseCodec::encode(atom), encode_exp(exp)]))
                .collect::<Vec<_>>()
        ]),
        ExpKind::OptE(exp) => json!(["OptE", encode_option(exp.as_deref(), encode_exp)]),
        ExpKind::ListE(exps) => json!(["ListE", il::encode_list(exps, encode_exp)]),
        ExpKind::ConsE(left, right) => json!(["ConsE", encode_exp(left), encode_exp(right)]),
        ExpKind::CatE(left, right) => json!(["CatE", encode_exp(left), encode_exp(right)]),
        ExpKind::MemE(left, right) => json!(["MemE", encode_exp(left), encode_exp(right)]),
        ExpKind::LenE(exp) => json!(["LenE", encode_exp(exp)]),
        ExpKind::DotE(exp, atom) => json!(["DotE", encode_exp(exp), AtomPhraseCodec::encode(atom)]),
        ExpKind::IdxE(base, index) => json!(["IdxE", encode_exp(base), encode_exp(index)]),
        ExpKind::SliceE(base, left, right) => json!([
            "SliceE",
            encode_exp(base),
            encode_exp(left),
            encode_exp(right)
        ]),
        ExpKind::UpdE(base, path, exp) => {
            json!(["UpdE", encode_exp(base), encode_path(path), encode_exp(exp)])
        }
        ExpKind::CallE(id, targs, args) => json!([
            "CallE",
            il::encode_id(id),
            il::encode_list(targs, il::encode_targ),
            il::encode_list(args, encode_arg)
        ]),
        ExpKind::IterE(exp, iter) => json!(["IterE", encode_exp(exp), il::encode_iter_exp(iter)]),
    }
}

fn decode_path(value: &Value) -> Result<ast::Path, DecodeError> {
    let (kind, ty, span) = source::decode_annotated(value, decode_path_kind, il::decode_typ_kind)?;
    Ok(ast::Path { kind, ty, span })
}

fn encode_path(path: &ast::Path) -> Value {
    source::encode_annotated(
        &path.kind,
        &path.ty,
        &path.span,
        encode_path_kind,
        il::encode_typ_kind,
    )
}

fn decode_path_kind(value: &Value) -> Result<PathKind, DecodeError> {
    let (tag, fields) = variant(value)?;
    match (tag, fields) {
        ("RootP", []) => Ok(PathKind::RootP),
        ("IdxP", [path, exp]) => Ok(PathKind::IdxP(
            Box::new(decode_path(path)?),
            Box::new(decode_exp(exp)?),
        )),
        ("SliceP", [path, left, right]) => Ok(PathKind::SliceP(
            Box::new(decode_path(path)?),
            Box::new(decode_exp(left)?),
            Box::new(decode_exp(right)?),
        )),
        ("DotP", [path, atom]) => Ok(PathKind::DotP(
            Box::new(decode_path(path)?),
            AtomPhraseCodec::decode(atom)?,
        )),
        ("RootP" | "IdxP" | "SliceP" | "DotP", _) => {
            Err(DecodeError::Expected("valid PL path arity"))
        }
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

fn encode_path_kind(path: &PathKind) -> Value {
    match path {
        PathKind::RootP => json!(["RootP"]),
        PathKind::IdxP(path, exp) => json!(["IdxP", encode_path(path), encode_exp(exp)]),
        PathKind::SliceP(path, left, right) => json!([
            "SliceP",
            encode_path(path),
            encode_exp(left),
            encode_exp(right)
        ]),
        PathKind::DotP(path, atom) => {
            json!(["DotP", encode_path(path), AtomPhraseCodec::encode(atom)])
        }
    }
}

fn decode_arg(value: &Value) -> Result<ast::Arg, DecodeError> {
    source::decode_phrase(value, |value| {
        let (tag, fields) = variant(value)?;
        match (tag, fields) {
            ("ExpA", [exp]) => Ok(ArgKind::ExpA(decode_exp(exp)?)),
            ("DefA", [id]) => Ok(ArgKind::DefA(il::decode_id(id)?)),
            ("ExpA" | "DefA", _) => Err(DecodeError::Expected("valid PL argument arity")),
            (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
        }
    })
}

fn encode_arg(arg: &ast::Arg) -> Value {
    source::encode_phrase(arg, |arg| match arg {
        ArgKind::ExpA(exp) => json!(["ExpA", encode_exp(exp)]),
        ArgKind::DefA(id) => json!(["DefA", il::encode_id(id)]),
    })
}
