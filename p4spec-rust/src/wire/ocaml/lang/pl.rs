use serde_json::{Value, json};

use crate::{
    lang::{
        hints::alter,
        pl::{
            annot,
            ast::{
                self, ArgKind, DefKind, ExpKind, Fallthrough, Guard, HoldCase, InstrDispatch,
                InstrGroup, InstrKind, ParamKind, PathKind,
            },
        },
    },
    wire::ocaml::{atom::AtomPhraseCodec, mixfix, source},
};

use super::{
    super::{
        DecodeError, EncodeError, array, boolean, field, integer, object, on_codec_stack, string,
        variant,
    },
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

pub struct SpecCodec;

impl SpecCodec {
    pub fn decode(value: &Value) -> Result<ast::Spec, DecodeError> {
        on_codec_stack(|| il::decode_list(value, decode_def))
    }

    pub fn encode(spec: &ast::Spec) -> Result<Value, EncodeError> {
        on_codec_stack(|| Ok(il::encode_list(spec, encode_def)))
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
    Ok(annot::Annotated {
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

fn decode_param(value: &Value) -> Result<ast::Param, DecodeError> {
    source::decode_phrase(value, |value| {
        let (tag, fields) = variant(value)?;
        match (tag, fields) {
            ("ExpP", [typ, exp]) => Ok(ParamKind::ExpP(il::decode_typ(typ)?, decode_exp(exp)?)),
            ("DefP", [id, tparams, params, typ]) => Ok(ParamKind::DefP(
                il::decode_id(id)?,
                il::decode_list(tparams, il::decode_tparam)?,
                il::decode_list(params, decode_param)?,
                il::decode_typ(typ)?,
            )),
            ("ExpP" | "DefP", _) => Err(DecodeError::Expected("valid PL parameter arity")),
            (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
        }
    })
}

fn encode_param(param: &ast::Param) -> Value {
    source::encode_phrase(param, |param| match param {
        ParamKind::ExpP(typ, exp) => json!(["ExpP", il::encode_typ(typ), encode_exp(exp)]),
        ParamKind::DefP(id, tparams, params, typ) => json!([
            "DefP",
            il::encode_id(id),
            il::encode_list(tparams, il::encode_tparam),
            il::encode_list(params, encode_param),
            il::encode_typ(typ)
        ]),
    })
}

fn decode_fallthrough(value: &Value) -> Result<Fallthrough, DecodeError> {
    let (tag, fields) = variant(value)?;
    match (tag, fields) {
        ("FallGroup", [id]) => Ok(Fallthrough::FallGroup(il::decode_id(id)?)),
        ("FallNext", []) => Ok(Fallthrough::FallNext),
        ("FallElse", []) => Ok(Fallthrough::FallElse),
        ("FallFail", []) => Ok(Fallthrough::FallFail),
        ("FallGroup" | "FallNext" | "FallElse" | "FallFail", _) => {
            Err(DecodeError::Expected("valid PL fallthrough arity"))
        }
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

fn encode_fallthrough(value: &Fallthrough) -> Value {
    match value {
        Fallthrough::FallGroup(id) => json!(["FallGroup", il::encode_id(id)]),
        Fallthrough::FallNext => json!(["FallNext"]),
        Fallthrough::FallElse => json!(["FallElse"]),
        Fallthrough::FallFail => json!(["FallFail"]),
    }
}

fn decode_inote(value: &Value) -> Result<(i64, Option<Fallthrough>), DecodeError> {
    let value = object(value)?;
    Ok((
        integer(field(value, "iid")?)?,
        decode_option(field(value, "fallthrough")?, decode_fallthrough)?,
    ))
}

fn encode_inote(iid: i64, fallthrough: Option<&Fallthrough>) -> Value {
    json!({"iid": iid, "fallthrough": encode_option(fallthrough, encode_fallthrough)})
}

fn decode_guard(value: &Value) -> Result<Guard, DecodeError> {
    let (tag, fields) = variant(value)?;
    match (tag, fields) {
        ("BoolG", [value]) => Ok(Guard::BoolG(boolean(value)?)),
        ("CmpG", [op, typ, exp]) => Ok(Guard::CmpG(
            il::decode_cmp_op(op)?,
            il::decode_op_typ(typ)?,
            decode_exp(exp)?,
        )),
        ("SubG", [typ, subcheck]) => Ok(Guard::SubG(
            il::decode_typ(typ)?,
            Box::new(il::decode_subcheck(subcheck)?),
        )),
        ("MatchG", [pattern]) => Ok(Guard::MatchG(il::decode_pattern(pattern)?)),
        ("MemG", [exp]) => Ok(Guard::MemG(decode_exp(exp)?)),
        ("CheckLetSubG", [typ, subcheck, exp]) => Ok(Guard::CheckLetSubG(
            il::decode_typ(typ)?,
            Box::new(il::decode_subcheck(subcheck)?),
            decode_exp(exp)?,
        )),
        ("CheckLetMatchG", [pattern, exp]) => Ok(Guard::CheckLetMatchG(
            il::decode_pattern(pattern)?,
            decode_exp(exp)?,
        )),
        ("BoolG" | "CmpG" | "SubG" | "MatchG" | "MemG" | "CheckLetSubG" | "CheckLetMatchG", _) => {
            Err(DecodeError::Expected("valid PL guard arity"))
        }
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

fn encode_guard(guard: &Guard) -> Value {
    match guard {
        Guard::BoolG(value) => json!(["BoolG", value]),
        Guard::CmpG(op, typ, exp) => json!([
            "CmpG",
            il::encode_cmp_op(*op),
            il::encode_op_typ(*typ),
            encode_exp(exp)
        ]),
        Guard::SubG(typ, subcheck) => {
            json!(["SubG", il::encode_typ(typ), il::encode_subcheck(subcheck)])
        }
        Guard::MatchG(pattern) => json!(["MatchG", il::encode_pattern(pattern)]),
        Guard::MemG(exp) => json!(["MemG", encode_exp(exp)]),
        Guard::CheckLetSubG(typ, subcheck, exp) => json!([
            "CheckLetSubG",
            il::encode_typ(typ),
            il::encode_subcheck(subcheck),
            encode_exp(exp)
        ]),
        Guard::CheckLetMatchG(pattern, exp) => json!([
            "CheckLetMatchG",
            il::encode_pattern(pattern),
            encode_exp(exp)
        ]),
    }
}

fn decode_instr<T>(
    value: &Value,
    decode_tier: fn(&Value) -> Result<T, DecodeError>,
) -> Result<ast::Instr<T>, DecodeError> {
    let value = object(value)?;
    let (kind, (iid, fallthrough), span) = source::decode_annotated(
        field(value, "node")?,
        |value| decode_instr_kind(value, decode_tier),
        decode_inote,
    )?;
    Ok(annot::Annotated {
        node: ast::InstrNode {
            kind,
            iid,
            fallthrough,
            span,
        },
        hints: decode_hints(field(value, "hints")?)?,
    })
}

fn encode_instr<T>(instr: &ast::Instr<T>, encode_tier: fn(&T) -> Value) -> Value {
    json!({
        "node": source::encode_annotated(&instr.node.kind, &(instr.node.iid, instr.node.fallthrough.as_ref()), &instr.node.span, |kind| encode_instr_kind(kind, encode_tier), |(iid, fallthrough)| encode_inote(*iid, *fallthrough)),
        "hints": encode_hints(&instr.hints)
    })
}

fn decode_block<T>(
    value: &Value,
    decode_tier: fn(&Value) -> Result<T, DecodeError>,
) -> Result<ast::Block<T>, DecodeError> {
    il::decode_list(value, |value| decode_instr(value, decode_tier))
}

fn encode_block<T>(block: &ast::Block<T>, encode_tier: fn(&T) -> Value) -> Value {
    il::encode_list(block, |instr| encode_instr(instr, encode_tier))
}

fn decode_hold_case<T>(
    value: &Value,
    decode_tier: fn(&Value) -> Result<T, DecodeError>,
) -> Result<HoldCase<T>, DecodeError> {
    let (tag, fields) = variant(value)?;
    match (tag, fields) {
        ("BothH", [left, right]) => Ok(HoldCase::BothH(
            decode_block(left, decode_tier)?,
            decode_block(right, decode_tier)?,
        )),
        ("HoldH", [block, dangle]) => Ok(HoldCase::HoldH(
            decode_block(block, decode_tier)?,
            boolean(dangle)?,
        )),
        ("NotHoldH", [block, dangle]) => Ok(HoldCase::NotHoldH(
            decode_block(block, decode_tier)?,
            boolean(dangle)?,
        )),
        ("BothH" | "HoldH" | "NotHoldH", _) => {
            Err(DecodeError::Expected("valid PL hold case arity"))
        }
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

fn encode_hold_case<T>(case: &HoldCase<T>, encode_tier: fn(&T) -> Value) -> Value {
    match case {
        HoldCase::BothH(left, right) => json!([
            "BothH",
            encode_block(left, encode_tier),
            encode_block(right, encode_tier)
        ]),
        HoldCase::HoldH(block, dangle) => {
            json!(["HoldH", encode_block(block, encode_tier), dangle])
        }
        HoldCase::NotHoldH(block, dangle) => {
            json!(["NotHoldH", encode_block(block, encode_tier), dangle])
        }
    }
}

fn decode_instr_kind<T>(
    value: &Value,
    decode_tier: fn(&Value) -> Result<T, DecodeError>,
) -> Result<InstrKind<T>, DecodeError> {
    let (tag, fields) = variant(value)?;
    match (tag, fields) {
        ("IfI", [exp, iters, block, dangle]) => Ok(InstrKind::IfI(
            decode_exp(exp)?,
            il::decode_list(iters, il::decode_iter_exp)?,
            decode_block(block, decode_tier)?,
            boolean(dangle)?,
        )),
        ("HoldI", [id, exp, iters, case]) => Ok(InstrKind::HoldI(
            il::decode_id(id)?,
            mixfix::decode(exp, decode_exp)?,
            il::decode_list(iters, il::decode_iter_exp)?,
            decode_hold_case(case, decode_tier)?,
        )),
        ("CaseI", [exp, cases, dangle]) => Ok(InstrKind::CaseI(
            decode_exp(exp)?,
            il::decode_list(cases, |value| match array(value)? {
                [guard, block] => Ok((decode_guard(guard)?, decode_block(block, decode_tier)?)),
                _ => Err(DecodeError::Expected("PL case pair")),
            })?,
            boolean(dangle)?,
        )),
        ("LetI", [left, right, iters]) => Ok(InstrKind::LetI(
            decode_exp(left)?,
            decode_exp(right)?,
            il::decode_list(iters, il::decode_iter_prem)?,
        )),
        ("DebugI", [exp]) => Ok(InstrKind::DebugI(decode_exp(exp)?)),
        ("DestructI", [bindings, exp]) => Ok(InstrKind::DestructI(
            il::decode_list(bindings, |value| match array(value)? {
                [name, exp] => Ok((
                    decode_option(name, |name| Ok(string(name)?.to_owned()))?,
                    decode_exp(exp)?,
                )),
                _ => Err(DecodeError::Expected("PL destruct binding pair")),
            })?,
            decode_exp(exp)?,
        )),
        ("CheckLetSubI", [typ, subcheck, left, right, block]) => Ok(InstrKind::CheckLetSubI(
            il::decode_typ(typ)?,
            Box::new(il::decode_subcheck(subcheck)?),
            decode_exp(left)?,
            decode_exp(right)?,
            decode_block(block, decode_tier)?,
        )),
        ("CheckLetMatchI", [pattern, left, right, block]) => Ok(InstrKind::CheckLetMatchI(
            il::decode_pattern(pattern)?,
            decode_exp(left)?,
            decode_exp(right)?,
            decode_block(block, decode_tier)?,
        )),
        ("OptionGetI", [left, right, block]) => Ok(InstrKind::OptionGetI(
            decode_exp(left)?,
            decode_exp(right)?,
            decode_block(block, decode_tier)?,
        )),
        ("TierI", [tier]) => Ok(InstrKind::TierI(decode_tier(tier)?)),
        (
            "IfI" | "HoldI" | "CaseI" | "LetI" | "DebugI" | "DestructI" | "CheckLetSubI"
            | "CheckLetMatchI" | "OptionGetI" | "TierI",
            _,
        ) => Err(DecodeError::Expected("valid PL instruction arity")),
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

fn encode_instr_kind<T>(instr: &InstrKind<T>, encode_tier: fn(&T) -> Value) -> Value {
    match instr {
        InstrKind::IfI(exp, iters, block, dangle) => json!([
            "IfI",
            encode_exp(exp),
            il::encode_list(iters, il::encode_iter_exp),
            encode_block(block, encode_tier),
            dangle
        ]),
        InstrKind::HoldI(id, exp, iters, case) => json!([
            "HoldI",
            il::encode_id(id),
            mixfix::encode(exp, encode_exp),
            il::encode_list(iters, il::encode_iter_exp),
            encode_hold_case(case, encode_tier)
        ]),
        InstrKind::CaseI(exp, cases, dangle) => json!([
            "CaseI",
            encode_exp(exp),
            cases
                .iter()
                .map(|(guard, block)| json!([
                    encode_guard(guard),
                    encode_block(block, encode_tier)
                ]))
                .collect::<Vec<_>>(),
            dangle
        ]),
        InstrKind::LetI(left, right, iters) => json!([
            "LetI",
            encode_exp(left),
            encode_exp(right),
            il::encode_list(iters, il::encode_iter_prem)
        ]),
        InstrKind::DebugI(exp) => json!(["DebugI", encode_exp(exp)]),
        InstrKind::DestructI(bindings, exp) => json!([
            "DestructI",
            bindings
                .iter()
                .map(|(name, exp)| json!([
                    encode_option(name.as_ref(), |name| json!(name)),
                    encode_exp(exp)
                ]))
                .collect::<Vec<_>>(),
            encode_exp(exp)
        ]),
        InstrKind::CheckLetSubI(typ, subcheck, left, right, block) => json!([
            "CheckLetSubI",
            il::encode_typ(typ),
            il::encode_subcheck(subcheck),
            encode_exp(left),
            encode_exp(right),
            encode_block(block, encode_tier)
        ]),
        InstrKind::CheckLetMatchI(pattern, left, right, block) => json!([
            "CheckLetMatchI",
            il::encode_pattern(pattern),
            encode_exp(left),
            encode_exp(right),
            encode_block(block, encode_tier)
        ]),
        InstrKind::OptionGetI(left, right, block) => json!([
            "OptionGetI",
            encode_exp(left),
            encode_exp(right),
            encode_block(block, encode_tier)
        ]),
        InstrKind::TierI(tier) => json!(["TierI", encode_tier(tier)]),
    }
}

fn decode_rel_signature(value: &Value) -> Result<ast::RelSignature, DecodeError> {
    match array(value)? {
        [typ, input] => Ok((il::decode_not_typ(typ)?, il::decode_input_hint(input)?)),
        _ => Err(DecodeError::Expected("PL relation signature pair")),
    }
}

fn encode_rel_signature((typ, input): &ast::RelSignature) -> Value {
    json!([il::encode_not_typ(typ), il::encode_input_hint(input)])
}

fn decode_instr_group(value: &Value) -> Result<InstrGroup, DecodeError> {
    let (tag, fields) = variant(value)?;
    match (tag, fields) {
        ("ResultI", [signature, exps]) => Ok(InstrGroup::ResultI(
            decode_rel_signature(signature)?,
            il::decode_list(exps, decode_exp)?,
        )),
        ("ReturnI", [exp]) => Ok(InstrGroup::ReturnI(decode_exp(exp)?)),
        ("RuleI", [id, exp, input, iters]) => Ok(InstrGroup::RuleI(
            il::decode_id(id)?,
            mixfix::decode(exp, decode_exp)?,
            il::decode_input_hint(input)?,
            il::decode_list(iters, il::decode_iter_prem)?,
        )),
        ("BacktrackI", [arms]) => Ok(InstrGroup::BacktrackI(il::decode_list(arms, |block| {
            decode_block(block, decode_instr_group)
        })?)),
        ("ResultI" | "ReturnI" | "RuleI" | "BacktrackI", _) => {
            Err(DecodeError::Expected("valid PL group instruction arity"))
        }
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

fn encode_instr_group(instr: &InstrGroup) -> Value {
    match instr {
        InstrGroup::ResultI(signature, exps) => json!([
            "ResultI",
            encode_rel_signature(signature),
            il::encode_list(exps, encode_exp)
        ]),
        InstrGroup::ReturnI(exp) => json!(["ReturnI", encode_exp(exp)]),
        InstrGroup::RuleI(id, exp, input, iters) => json!([
            "RuleI",
            il::encode_id(id),
            mixfix::encode(exp, encode_exp),
            il::encode_input_hint(input),
            il::encode_list(iters, il::encode_iter_prem)
        ]),
        InstrGroup::BacktrackI(arms) => json!([
            "BacktrackI",
            arms.iter()
                .map(|block| encode_block(block, encode_instr_group))
                .collect::<Vec<_>>()
        ]),
    }
}

fn decode_instr_dispatch(value: &Value) -> Result<InstrDispatch, DecodeError> {
    let (tag, fields) = variant(value)?;
    match (tag, fields) {
        ("GroupI", [group, rel, signature, exps, block]) => Ok(InstrDispatch::GroupI(
            il::decode_id(group)?,
            il::decode_id(rel)?,
            decode_rel_signature(signature)?,
            il::decode_list(exps, decode_exp)?,
            decode_block(block, decode_instr_group)?,
        )),
        ("RouteI", [arms]) => Ok(InstrDispatch::RouteI(il::decode_list(arms, |block| {
            decode_block(block, decode_instr_dispatch)
        })?)),
        ("GroupI" | "RouteI", _) => {
            Err(DecodeError::Expected("valid PL dispatch instruction arity"))
        }
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

fn encode_instr_dispatch(instr: &InstrDispatch) -> Value {
    match instr {
        InstrDispatch::GroupI(group, rel, signature, exps, block) => json!([
            "GroupI",
            il::encode_id(group),
            il::encode_id(rel),
            encode_rel_signature(signature),
            il::encode_list(exps, encode_exp),
            encode_block(block, encode_instr_group)
        ]),
        InstrDispatch::RouteI(arms) => json!([
            "RouteI",
            arms.iter()
                .map(|block| encode_block(block, encode_instr_dispatch))
                .collect::<Vec<_>>()
        ]),
    }
}

fn decode_extern_rel(value: &Value) -> Result<ast::ExternRel, DecodeError> {
    match array(value)? {
        [id, signature, exps] => Ok((
            il::decode_id(id)?,
            decode_rel_signature(signature)?,
            il::decode_list(exps, decode_exp)?,
        )),
        _ => Err(DecodeError::Expected("PL external relation triple")),
    }
}
fn encode_extern_rel((id, signature, exps): &ast::ExternRel) -> Value {
    json!([
        il::encode_id(id),
        encode_rel_signature(signature),
        il::encode_list(exps, encode_exp)
    ])
}

fn decode_rel(value: &Value) -> Result<ast::Rel, DecodeError> {
    match array(value)? {
        [id, signature, exps, block, else_block] => Ok((
            il::decode_id(id)?,
            decode_rel_signature(signature)?,
            il::decode_list(exps, decode_exp)?,
            decode_block(block, decode_instr_dispatch)?,
            decode_option(else_block, |block| {
                decode_block(block, decode_instr_dispatch)
            })?,
        )),
        _ => Err(DecodeError::Expected("PL relation quintuple")),
    }
}
fn encode_rel((id, signature, exps, block, else_block): &ast::Rel) -> Value {
    json!([
        il::encode_id(id),
        encode_rel_signature(signature),
        il::encode_list(exps, encode_exp),
        encode_block(block, encode_instr_dispatch),
        encode_option(else_block.as_ref(), |block| encode_block(
            block,
            encode_instr_dispatch
        ))
    ])
}

fn decode_extern_func(value: &Value) -> Result<ast::ExternFunc, DecodeError> {
    match array(value)? {
        [id, tparams, params, typ] => Ok((
            il::decode_id(id)?,
            il::decode_list(tparams, il::decode_tparam)?,
            il::decode_list(params, decode_param)?,
            il::decode_typ(typ)?,
        )),
        _ => Err(DecodeError::Expected("PL function quadruple")),
    }
}
fn encode_extern_func((id, tparams, params, typ): &ast::ExternFunc) -> Value {
    json!([
        il::encode_id(id),
        il::encode_list(tparams, il::encode_tparam),
        il::encode_list(params, encode_param),
        il::encode_typ(typ)
    ])
}

fn decode_table_row(value: &Value) -> Result<ast::TableRow, DecodeError> {
    match array(value)? {
        [exps, exp, block] => Ok((
            il::decode_list(exps, decode_exp)?,
            decode_exp(exp)?,
            decode_block(block, decode_instr_group)?,
        )),
        _ => Err(DecodeError::Expected("PL table row triple")),
    }
}
fn encode_table_row((exps, exp, block): &ast::TableRow) -> Value {
    json!([
        il::encode_list(exps, encode_exp),
        encode_exp(exp),
        encode_block(block, encode_instr_group)
    ])
}

fn decode_table_func(value: &Value) -> Result<ast::TableFunc, DecodeError> {
    match array(value)? {
        [id, params, typ, rows] => Ok((
            il::decode_id(id)?,
            il::decode_list(params, decode_param)?,
            il::decode_typ(typ)?,
            il::decode_list(rows, decode_table_row)?,
        )),
        _ => Err(DecodeError::Expected("PL table function quadruple")),
    }
}
fn encode_table_func((id, params, typ, rows): &ast::TableFunc) -> Value {
    json!([
        il::encode_id(id),
        il::encode_list(params, encode_param),
        il::encode_typ(typ),
        il::encode_list(rows, encode_table_row)
    ])
}

fn decode_defined_func(value: &Value) -> Result<ast::DefinedFunc, DecodeError> {
    match array(value)? {
        [id, tparams, params, typ, block, else_block] => Ok((
            il::decode_id(id)?,
            il::decode_list(tparams, il::decode_tparam)?,
            il::decode_list(params, decode_param)?,
            il::decode_typ(typ)?,
            decode_block(block, decode_instr_group)?,
            decode_option(else_block, |block| decode_block(block, decode_instr_group))?,
        )),
        _ => Err(DecodeError::Expected("PL defined function sextuple")),
    }
}
fn encode_defined_func((id, tparams, params, typ, block, else_block): &ast::DefinedFunc) -> Value {
    json!([
        il::encode_id(id),
        il::encode_list(tparams, il::encode_tparam),
        il::encode_list(params, encode_param),
        il::encode_typ(typ),
        encode_block(block, encode_instr_group),
        encode_option(else_block.as_ref(), |block| encode_block(
            block,
            encode_instr_group
        ))
    ])
}

fn decode_def(value: &Value) -> Result<ast::Def, DecodeError> {
    let value = object(value)?;
    let def = source::decode_phrase(field(value, "node")?, |value| {
        let (tag, fields) = variant(value)?;
        match (tag, fields) {
            ("ExternTypD", [id]) => Ok(DefKind::ExternTypD(il::decode_id(id)?)),
            ("TypD", [id, tparams, typ]) => Ok(DefKind::TypD(
                il::decode_id(id)?,
                il::decode_list(tparams, il::decode_tparam)?,
                il::decode_def_typ(typ)?,
            )),
            ("VarD", [id, typ]) => Ok(DefKind::VarD(il::decode_id(id)?, il::decode_typ(typ)?)),
            ("ExternRelD", [rel]) => Ok(DefKind::ExternRelD(decode_extern_rel(rel)?)),
            ("RelD", [rel]) => Ok(DefKind::RelD(decode_rel(rel)?)),
            ("ExternDecD", [func]) => Ok(DefKind::ExternDecD(decode_extern_func(func)?)),
            ("BuiltinDecD", [func]) => Ok(DefKind::BuiltinDecD(decode_extern_func(func)?)),
            ("TableDecD", [func]) => Ok(DefKind::TableDecD(decode_table_func(func)?)),
            ("FuncDecD", [func]) => Ok(DefKind::FuncDecD(decode_defined_func(func)?)),
            (
                "ExternTypD" | "TypD" | "VarD" | "ExternRelD" | "RelD" | "ExternDecD"
                | "BuiltinDecD" | "TableDecD" | "FuncDecD",
                _,
            ) => Err(DecodeError::Expected("valid PL definition arity")),
            (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
        }
    })?;
    Ok(annot::Annotated {
        node: ast::DefNode {
            kind: def.node,
            span: def.span,
        },
        hints: decode_hints(field(value, "hints")?)?,
    })
}

fn encode_def(def: &ast::Def) -> Value {
    let node = source::encode_phrase(
        &crate::domain::source::Spanned::new(def.node.kind.clone(), def.node.span.clone()),
        |def| match def {
            DefKind::ExternTypD(id) => json!(["ExternTypD", il::encode_id(id)]),
            DefKind::TypD(id, tparams, typ) => json!([
                "TypD",
                il::encode_id(id),
                il::encode_list(tparams, il::encode_tparam),
                il::encode_def_typ(typ)
            ]),
            DefKind::VarD(id, typ) => json!(["VarD", il::encode_id(id), il::encode_typ(typ)]),
            DefKind::ExternRelD(rel) => json!(["ExternRelD", encode_extern_rel(rel)]),
            DefKind::RelD(rel) => json!(["RelD", encode_rel(rel)]),
            DefKind::ExternDecD(func) => json!(["ExternDecD", encode_extern_func(func)]),
            DefKind::BuiltinDecD(func) => json!(["BuiltinDecD", encode_extern_func(func)]),
            DefKind::TableDecD(func) => json!(["TableDecD", encode_table_func(func)]),
            DefKind::FuncDecD(func) => json!(["FuncDecD", encode_defined_func(func)]),
        },
    );
    json!({"node": node, "hints": encode_hints(&def.hints)})
}
