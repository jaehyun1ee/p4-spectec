use serde_json::{Value, json};

use crate::{
    lang::{
        common::noted::Noted,
        hints::{alter, fields},
        pl::{
            annot,
            ast::{self, *},
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

fn decode_alter(value: &Value) -> Result<alter::AlterationHint, DecodeError> {
    let (tag, fields) = variant(value)?;
    match (tag, fields) {
        ("TextH", [text]) => Ok(alter::AlterationHint::Text(string(text)?.to_owned())),
        ("AtomH", [atom]) => Ok(alter::AlterationHint::Atom(AtomPhraseCodec::decode(atom)?)),
        ("SeqH", [hints]) => Ok(alter::AlterationHint::Seq(il::decode_list(
            hints,
            decode_alter,
        )?)),
        ("BrackH", [left, hint, right]) => Ok(alter::AlterationHint::Brack(
            AtomPhraseCodec::decode(left)?,
            Box::new(decode_alter(hint)?),
            AtomPhraseCodec::decode(right)?,
        )),
        ("HoleH", [hole]) => {
            let (tag, fields) = variant(hole)?;
            match (tag, fields) {
                ("Next", []) => Ok(alter::AlterationHint::Hole(alter::Hole::Next)),
                ("Num", [index]) => Ok(alter::AlterationHint::Hole(alter::Hole::Num(
                    super::super::integer(index)?,
                ))),
                ("Next" | "Num", _) => Err(DecodeError::Expected("valid PL alter hole arity")),
                (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
            }
        }
        ("FuseH", [left, right]) => Ok(alter::AlterationHint::Fuse(
            Box::new(decode_alter(left)?),
            Box::new(decode_alter(right)?),
        )),
        ("OtherH", [exp]) => Ok(alter::AlterationHint::Other(el::decode_exp(exp)?)),
        ("TextH" | "AtomH" | "SeqH" | "BrackH" | "HoleH" | "FuseH" | "OtherH", _) => {
            Err(DecodeError::Expected("valid PL alter hint arity"))
        }
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

fn encode_alter(hint: &alter::AlterationHint) -> Value {
    match hint {
        alter::AlterationHint::Text(text) => json!(["TextH", text]),
        alter::AlterationHint::Atom(atom) => json!(["AtomH", AtomPhraseCodec::encode(atom)]),
        alter::AlterationHint::Seq(hints) => {
            json!(["SeqH", il::encode_list(hints, encode_alter)])
        }
        alter::AlterationHint::Brack(left, hint, right) => json!([
            "BrackH",
            AtomPhraseCodec::encode(left),
            encode_alter(hint),
            AtomPhraseCodec::encode(right)
        ]),
        alter::AlterationHint::Hole(alter::Hole::Next) => json!(["HoleH", ["Next"]]),
        alter::AlterationHint::Hole(alter::Hole::Num(index)) => {
            json!(["HoleH", ["Num", index]])
        }
        alter::AlterationHint::Fuse(left, right) => {
            json!(["FuseH", encode_alter(left), encode_alter(right)])
        }
        alter::AlterationHint::Other(exp) => json!(["OtherH", el::encode_exp(exp)]),
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
            Ok(fields::FieldHint::new(il::decode_list(value, |value| {
                Ok(string(value)?.to_owned())
            })?))
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
        "prose_fields": encode_option(hints.prose_fields.as_ref(), |fields| json!(fields.fields())),
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
    Ok(crate::spanned! {
        node: Noted::new(kind, ty),
        span: span,
    })
}

fn encode_exp_node(exp: &ast::ExpNode) -> Value {
    source::encode_annotated(
        &exp.node.kind,
        &exp.node.note,
        &exp.span,
        encode_exp_kind,
        il::encode_typ_kind,
    )
}

fn decode_exp_kind(value: &Value) -> Result<ExpKind, DecodeError> {
    let (tag, fields) = variant(value)?;
    match (tag, fields) {
        ("BoolE", [value]) => Ok(ExpKind::Bool(boolean(value)?)),
        ("NumE", [num]) => Ok(ExpKind::Num(xl::decode_num(num)?)),
        ("TextE", [text]) => Ok(ExpKind::Text(string(text)?.to_owned())),
        ("VarE", [id]) => Ok(ExpKind::Var(il::decode_id(id)?)),
        ("UnE", [op, typ, exp]) => Ok(ExpKind::Un(
            il::decode_un_op(op)?,
            il::decode_op_typ(typ)?,
            Box::new(decode_exp(exp)?),
        )),
        ("BinE", [op, typ, left, right]) => Ok(ExpKind::Bin(
            il::decode_bin_op(op)?,
            il::decode_op_typ(typ)?,
            Box::new(decode_exp(left)?),
            Box::new(decode_exp(right)?),
        )),
        ("CmpE", [op, typ, left, right]) => Ok(ExpKind::Cmp(
            il::decode_cmp_op(op)?,
            il::decode_op_typ(typ)?,
            Box::new(decode_exp(left)?),
            Box::new(decode_exp(right)?),
        )),
        ("UpCastE", [typ, exp]) => Ok(ExpKind::UpCast(
            il::decode_typ(typ)?,
            Box::new(decode_exp(exp)?),
        )),
        ("DownCastE", [typ, exp]) => Ok(ExpKind::DownCast(
            il::decode_typ(typ)?,
            Box::new(decode_exp(exp)?),
        )),
        ("SubE", [exp, typ, subcheck]) => Ok(ExpKind::Sub(
            Box::new(decode_exp(exp)?),
            il::decode_typ(typ)?,
            Box::new(il::decode_subcheck(subcheck)?),
        )),
        ("MatchE", [exp, pattern]) => Ok(ExpKind::Match(
            Box::new(decode_exp(exp)?),
            il::decode_pattern(pattern)?,
        )),
        ("TupleE", [exps]) => Ok(ExpKind::Tuple(il::decode_list(exps, decode_exp)?)),
        ("CaseE", [exp]) => Ok(ExpKind::Case(Box::new(mixfix::decode(exp, decode_exp)?))),
        ("StrE", [fields]) => Ok(ExpKind::Str(il::decode_list(
            fields,
            |value| match array(value)? {
                [atom, exp] => Ok((AtomPhraseCodec::decode(atom)?, decode_exp(exp)?)),
                _ => Err(DecodeError::Expected("PL structure field pair")),
            },
        )?)),
        ("OptE", [exp]) => Ok(ExpKind::Opt(decode_option(exp, |exp| {
            Ok(Box::new(decode_exp(exp)?))
        })?)),
        ("ListE", [exps]) => Ok(ExpKind::List(il::decode_list(exps, decode_exp)?)),
        ("ConsE", [left, right]) => Ok(ExpKind::Cons(
            Box::new(decode_exp(left)?),
            Box::new(decode_exp(right)?),
        )),
        ("CatE", [left, right]) => Ok(ExpKind::Cat(
            Box::new(decode_exp(left)?),
            Box::new(decode_exp(right)?),
        )),
        ("MemE", [left, right]) => Ok(ExpKind::Mem(
            Box::new(decode_exp(left)?),
            Box::new(decode_exp(right)?),
        )),
        ("LenE", [exp]) => Ok(ExpKind::Len(Box::new(decode_exp(exp)?))),
        ("DotE", [exp, atom]) => Ok(ExpKind::Dot(
            Box::new(decode_exp(exp)?),
            AtomPhraseCodec::decode(atom)?,
        )),
        ("IdxE", [base, index]) => Ok(ExpKind::Idx(
            Box::new(decode_exp(base)?),
            Box::new(decode_exp(index)?),
        )),
        ("SliceE", [base, left, right]) => Ok(ExpKind::Slice(
            Box::new(decode_exp(base)?),
            Box::new(decode_exp(left)?),
            Box::new(decode_exp(right)?),
        )),
        ("UpdE", [base, path, exp]) => Ok(ExpKind::Upd(
            Box::new(decode_exp(base)?),
            Box::new(decode_path(path)?),
            Box::new(decode_exp(exp)?),
        )),
        ("CallE", [id, targs, args]) => Ok(ExpKind::Call(
            il::decode_id(id)?,
            il::decode_list(targs, il::decode_targ)?,
            il::decode_list(args, decode_arg)?,
        )),
        ("IterE", [exp, iter]) => Ok(ExpKind::Iter(
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
        ExpKind::Bool(value) => json!(["BoolE", value]),
        ExpKind::Num(num) => json!(["NumE", xl::encode_num(num)]),
        ExpKind::Text(text) => json!(["TextE", text]),
        ExpKind::Var(id) => json!(["VarE", il::encode_id(id)]),
        ExpKind::Un(op, typ, exp) => json!([
            "UnE",
            il::encode_un_op(*op),
            il::encode_op_typ(*typ),
            encode_exp(exp)
        ]),
        ExpKind::Bin(op, typ, left, right) => json!([
            "BinE",
            il::encode_bin_op(*op),
            il::encode_op_typ(*typ),
            encode_exp(left),
            encode_exp(right)
        ]),
        ExpKind::Cmp(op, typ, left, right) => json!([
            "CmpE",
            il::encode_cmp_op(*op),
            il::encode_op_typ(*typ),
            encode_exp(left),
            encode_exp(right)
        ]),
        ExpKind::UpCast(typ, exp) => json!(["UpCastE", il::encode_typ(typ), encode_exp(exp)]),
        ExpKind::DownCast(typ, exp) => json!(["DownCastE", il::encode_typ(typ), encode_exp(exp)]),
        ExpKind::Sub(exp, typ, subcheck) => json!([
            "SubE",
            encode_exp(exp),
            il::encode_typ(typ),
            il::encode_subcheck(subcheck)
        ]),
        ExpKind::Match(exp, pattern) => {
            json!(["MatchE", encode_exp(exp), il::encode_pattern(pattern)])
        }
        ExpKind::Tuple(exps) => json!(["TupleE", il::encode_list(exps, encode_exp)]),
        ExpKind::Case(exp) => json!(["CaseE", mixfix::encode(exp, encode_exp)]),
        ExpKind::Str(fields) => json!([
            "StrE",
            fields
                .iter()
                .map(|(atom, exp)| json!([AtomPhraseCodec::encode(atom), encode_exp(exp)]))
                .collect::<Vec<_>>()
        ]),
        ExpKind::Opt(exp) => json!(["OptE", encode_option(exp.as_deref(), encode_exp)]),
        ExpKind::List(exps) => json!(["ListE", il::encode_list(exps, encode_exp)]),
        ExpKind::Cons(left, right) => json!(["ConsE", encode_exp(left), encode_exp(right)]),
        ExpKind::Cat(left, right) => json!(["CatE", encode_exp(left), encode_exp(right)]),
        ExpKind::Mem(left, right) => json!(["MemE", encode_exp(left), encode_exp(right)]),
        ExpKind::Len(exp) => json!(["LenE", encode_exp(exp)]),
        ExpKind::Dot(exp, atom) => json!(["DotE", encode_exp(exp), AtomPhraseCodec::encode(atom)]),
        ExpKind::Idx(base, index) => json!(["IdxE", encode_exp(base), encode_exp(index)]),
        ExpKind::Slice(base, left, right) => json!([
            "SliceE",
            encode_exp(base),
            encode_exp(left),
            encode_exp(right)
        ]),
        ExpKind::Upd(base, path, exp) => {
            json!(["UpdE", encode_exp(base), encode_path(path), encode_exp(exp)])
        }
        ExpKind::Call(id, targs, args) => json!([
            "CallE",
            il::encode_id(id),
            il::encode_list(targs, il::encode_targ),
            il::encode_list(args, encode_arg)
        ]),
        ExpKind::Iter(exp, iter) => json!(["IterE", encode_exp(exp), il::encode_iter_exp(iter)]),
    }
}

fn decode_path(value: &Value) -> Result<ast::Path, DecodeError> {
    let (kind, ty, span) = source::decode_annotated(value, decode_path_kind, il::decode_typ_kind)?;
    Ok(crate::spanned! {
        node: Noted::new(kind, ty),
        span: span,
    })
}

fn encode_path(path: &ast::Path) -> Value {
    source::encode_annotated(
        &path.node.kind,
        &path.node.note,
        &path.span,
        encode_path_kind,
        il::encode_typ_kind,
    )
}

fn decode_path_kind(value: &Value) -> Result<PathKind, DecodeError> {
    let (tag, fields) = variant(value)?;
    match (tag, fields) {
        ("RootP", []) => Ok(PathKind::Root),
        ("IdxP", [path, exp]) => Ok(PathKind::Idx(
            Box::new(decode_path(path)?),
            Box::new(decode_exp(exp)?),
        )),
        ("SliceP", [path, left, right]) => Ok(PathKind::Slice(
            Box::new(decode_path(path)?),
            Box::new(decode_exp(left)?),
            Box::new(decode_exp(right)?),
        )),
        ("DotP", [path, atom]) => Ok(PathKind::Dot(
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
        PathKind::Root => json!(["RootP"]),
        PathKind::Idx(path, exp) => json!(["IdxP", encode_path(path), encode_exp(exp)]),
        PathKind::Slice(path, left, right) => json!([
            "SliceP",
            encode_path(path),
            encode_exp(left),
            encode_exp(right)
        ]),
        PathKind::Dot(path, atom) => {
            json!(["DotP", encode_path(path), AtomPhraseCodec::encode(atom)])
        }
    }
}

fn decode_arg(value: &Value) -> Result<ast::Arg, DecodeError> {
    source::decode_phrase(value, |value| {
        let (tag, fields) = variant(value)?;
        match (tag, fields) {
            ("ExpA", [exp]) => Ok(ArgKind::Exp(Box::new(decode_exp(exp)?))),
            ("DefA", [id]) => Ok(ArgKind::Def(il::decode_id(id)?)),
            ("ExpA" | "DefA", _) => Err(DecodeError::Expected("valid PL argument arity")),
            (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
        }
    })
}

fn encode_arg(arg: &ast::Arg) -> Value {
    source::encode_phrase(arg, |arg| match arg {
        ArgKind::Exp(exp) => json!(["ExpA", encode_exp(exp)]),
        ArgKind::Def(id) => json!(["DefA", il::encode_id(id)]),
    })
}

fn decode_param(value: &Value) -> Result<ast::Param, DecodeError> {
    source::decode_phrase(value, |value| {
        let (tag, fields) = variant(value)?;
        match (tag, fields) {
            ("ExpP", [typ, exp]) => Ok(ParamKind::Exp(
                il::decode_typ(typ)?,
                Box::new(decode_exp(exp)?),
            )),
            ("DefP", [id, tparams, params, typ]) => Ok(ParamKind::Def(
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
        ParamKind::Exp(typ, exp) => json!(["ExpP", il::encode_typ(typ), encode_exp(exp)]),
        ParamKind::Def(id, tparams, params, typ) => json!([
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
        ("BoolG", [value]) => Ok(Guard::Bool(boolean(value)?)),
        ("CmpG", [op, typ, exp]) => Ok(Guard::Cmp(
            il::decode_cmp_op(op)?,
            il::decode_op_typ(typ)?,
            decode_exp(exp)?,
        )),
        ("SubG", [typ, subcheck]) => Ok(Guard::Sub(
            il::decode_typ(typ)?,
            Box::new(il::decode_subcheck(subcheck)?),
        )),
        ("MatchG", [pattern]) => Ok(Guard::Match(il::decode_pattern(pattern)?)),
        ("MemG", [exp]) => Ok(Guard::Mem(decode_exp(exp)?)),
        ("CheckLetSubG", [typ, subcheck, exp]) => Ok(Guard::CheckLetSub(
            il::decode_typ(typ)?,
            Box::new(il::decode_subcheck(subcheck)?),
            decode_exp(exp)?,
        )),
        ("CheckLetMatchG", [pattern, exp]) => Ok(Guard::CheckLetMatch(
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
        Guard::Bool(value) => json!(["BoolG", value]),
        Guard::Cmp(op, typ, exp) => json!([
            "CmpG",
            il::encode_cmp_op(*op),
            il::encode_op_typ(*typ),
            encode_exp(exp)
        ]),
        Guard::Sub(typ, subcheck) => {
            json!(["SubG", il::encode_typ(typ), il::encode_subcheck(subcheck)])
        }
        Guard::Match(pattern) => json!(["MatchG", il::encode_pattern(pattern)]),
        Guard::Mem(exp) => json!(["MemG", encode_exp(exp)]),
        Guard::CheckLetSub(typ, subcheck, exp) => json!([
            "CheckLetSubG",
            il::encode_typ(typ),
            il::encode_subcheck(subcheck),
            encode_exp(exp)
        ]),
        Guard::CheckLetMatch(pattern, exp) => json!([
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
    let mut instr = ast::instr(kind, iid, fallthrough, span);
    instr.hints = decode_hints(field(value, "hints")?)?;
    Ok(instr)
}

fn encode_instr<T>(instr: &ast::Instr<T>, encode_tier: fn(&T) -> Value) -> Value {
    json!({
        "node": source::encode_annotated(&instr.node.node.kind, &(instr.node.node.note.iid, instr.node.node.note.fallthrough.as_ref()), &instr.node.span, |kind| encode_instr_kind(kind, encode_tier), |(iid, fallthrough)| encode_inote(*iid, *fallthrough)),
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
        ("BothH", [left, right]) => Ok(HoldCase::Both(
            decode_block(left, decode_tier)?,
            decode_block(right, decode_tier)?,
        )),
        ("HoldH", [block, dangle]) => Ok(HoldCase::Hold(
            decode_block(block, decode_tier)?,
            boolean(dangle)?,
        )),
        ("NotHoldH", [block, dangle]) => Ok(HoldCase::NotHold(
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
        HoldCase::Both(left, right) => json!([
            "BothH",
            encode_block(left, encode_tier),
            encode_block(right, encode_tier)
        ]),
        HoldCase::Hold(block, dangle) => {
            json!(["HoldH", encode_block(block, encode_tier), dangle])
        }
        HoldCase::NotHold(block, dangle) => {
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
        ("IfI", [exp, iters, block, dangle]) => Ok(InstrKind::If(IfInstr {
            exp: decode_exp(exp)?,
            iter_exps: il::decode_list(iters, il::decode_iter_exp)?,
            block: decode_block(block, decode_tier)?,
            dangle: boolean(dangle)?,
        })),
        ("HoldI", [id, exp, iters, case]) => Ok(InstrKind::Hold(HoldInstr {
            id: il::decode_id(id)?,
            not_exp: mixfix::decode(exp, decode_exp)?,
            iter_exps: il::decode_list(iters, il::decode_iter_exp)?,
            hold_case: decode_hold_case(case, decode_tier)?,
        })),
        ("CaseI", [exp, cases, dangle]) => Ok(InstrKind::Case(CaseInstr {
            exp: decode_exp(exp)?,
            cases: il::decode_list(cases, |value| match array(value)? {
                [guard, block] => Ok(ast::Case {
                    guard: decode_guard(guard)?,
                    block: decode_block(block, decode_tier)?,
                }),
                _ => Err(DecodeError::Expected("PL case pair")),
            })?,
            dangle: boolean(dangle)?,
        })),
        ("LetI", [left, right, iters]) => Ok(InstrKind::Let(LetInstr {
            exp_l: decode_exp(left)?,
            exp_r: decode_exp(right)?,
            iter_instrs: il::decode_list(iters, il::decode_iter_prem)?,
        })),
        ("DebugI", [exp]) => Ok(InstrKind::Debug(DebugInstr {
            exp: decode_exp(exp)?,
        })),
        ("DestructI", [bindings, exp]) => Ok(InstrKind::Destruct(DestructInstr {
            bindings: il::decode_list(bindings, |value| match array(value)? {
                [name, exp] => Ok((
                    decode_option(name, |name| Ok(string(name)?.to_owned()))?,
                    decode_exp(exp)?,
                )),
                _ => Err(DecodeError::Expected("PL destruct binding pair")),
            })?,
            exp: decode_exp(exp)?,
        })),
        ("CheckLetSubI", [typ, subcheck, left, right, block]) => {
            Ok(InstrKind::CheckLetSub(CheckLetSubInstr {
                typ: il::decode_typ(typ)?,
                subcheck: Box::new(il::decode_subcheck(subcheck)?),
                exp_l: decode_exp(left)?,
                exp_r: decode_exp(right)?,
                block: decode_block(block, decode_tier)?,
            }))
        }
        ("CheckLetMatchI", [pattern, left, right, block]) => {
            Ok(InstrKind::CheckLetMatch(CheckLetMatchInstr {
                pattern: il::decode_pattern(pattern)?,
                exp_l: decode_exp(left)?,
                exp_r: decode_exp(right)?,
                block: decode_block(block, decode_tier)?,
            }))
        }
        ("OptionGetI", [left, right, block]) => Ok(InstrKind::OptionGet(OptionGetInstr {
            exp_l: decode_exp(left)?,
            exp_r: decode_exp(right)?,
            block: decode_block(block, decode_tier)?,
        })),
        ("TierI", [tier]) => Ok(InstrKind::Tier(TierInstr {
            tier: decode_tier(tier)?,
        })),
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
        InstrKind::If(IfInstr {
            exp,
            iter_exps: iters,
            block,
            dangle,
        }) => json!([
            "IfI",
            encode_exp(exp),
            il::encode_list(iters, il::encode_iter_exp),
            encode_block(block, encode_tier),
            dangle
        ]),
        InstrKind::Hold(HoldInstr {
            id,
            not_exp,
            iter_exps: iters,
            hold_case: case,
        }) => json!([
            "HoldI",
            il::encode_id(id),
            mixfix::encode(not_exp, encode_exp),
            il::encode_list(iters, il::encode_iter_exp),
            encode_hold_case(case, encode_tier)
        ]),
        InstrKind::Case(CaseInstr { exp, cases, dangle }) => json!([
            "CaseI",
            encode_exp(exp),
            cases
                .iter()
                .map(|case| json!([
                    encode_guard(&case.guard),
                    encode_block(&case.block, encode_tier)
                ]))
                .collect::<Vec<_>>(),
            dangle
        ]),
        InstrKind::Let(LetInstr {
            exp_l: left,
            exp_r: right,
            iter_instrs: iters,
        }) => json!([
            "LetI",
            encode_exp(left),
            encode_exp(right),
            il::encode_list(iters, il::encode_iter_prem)
        ]),
        InstrKind::Debug(DebugInstr { exp }) => json!(["DebugI", encode_exp(exp)]),
        InstrKind::Destruct(DestructInstr { bindings, exp }) => json!([
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
        InstrKind::CheckLetSub(CheckLetSubInstr {
            typ,
            subcheck,
            exp_l: left,
            exp_r: right,
            block,
        }) => json!([
            "CheckLetSubI",
            il::encode_typ(typ),
            il::encode_subcheck(subcheck),
            encode_exp(left),
            encode_exp(right),
            encode_block(block, encode_tier)
        ]),
        InstrKind::CheckLetMatch(CheckLetMatchInstr {
            pattern,
            exp_l: left,
            exp_r: right,
            block,
        }) => json!([
            "CheckLetMatchI",
            il::encode_pattern(pattern),
            encode_exp(left),
            encode_exp(right),
            encode_block(block, encode_tier)
        ]),
        InstrKind::OptionGet(OptionGetInstr {
            exp_l: left,
            exp_r: right,
            block,
        }) => json!([
            "OptionGetI",
            encode_exp(left),
            encode_exp(right),
            encode_block(block, encode_tier)
        ]),
        InstrKind::Tier(TierInstr { tier }) => json!(["TierI", encode_tier(tier)]),
    }
}

fn decode_rel_signature(value: &Value) -> Result<ast::RelSignature, DecodeError> {
    match array(value)? {
        [typ, input] => Ok(ast::RelSignature {
            not_typ: il::decode_not_typ(typ)?,
            input_hint: il::decode_input_hint(input)?,
        }),
        _ => Err(DecodeError::Expected("PL relation signature pair")),
    }
}

fn encode_rel_signature(rel_signature: &ast::RelSignature) -> Value {
    json!([
        il::encode_not_typ(&rel_signature.not_typ),
        il::encode_input_hint(&rel_signature.input_hint)
    ])
}

fn decode_instr_group(value: &Value) -> Result<InstrGroup, DecodeError> {
    let (tag, fields) = variant(value)?;
    match (tag, fields) {
        ("ResultI", [rel_signature, exps_output]) => Ok(InstrGroup::Result(ResultGroupInstr {
            rel_signature: decode_rel_signature(rel_signature)?,
            exps_output: il::decode_list(exps_output, decode_exp)?,
        })),
        ("ReturnI", [exp]) => Ok(InstrGroup::Return(ReturnGroupInstr {
            exp: decode_exp(exp)?,
        })),
        ("RuleI", [id, not_exp, input_hint, iter_instrs]) => Ok(InstrGroup::Rule(RuleGroupInstr {
            id: il::decode_id(id)?,
            not_exp: mixfix::decode(not_exp, decode_exp)?,
            input_hint: il::decode_input_hint(input_hint)?,
            iter_instrs: il::decode_list(iter_instrs, il::decode_iter_prem)?,
        })),
        ("BacktrackI", [arms]) => Ok(InstrGroup::Backtrack(BacktrackGroupInstr {
            blocks: il::decode_list(arms, |block| decode_block(block, decode_instr_group))?,
        })),
        ("ResultI" | "ReturnI" | "RuleI" | "BacktrackI", _) => {
            Err(DecodeError::Expected("valid PL group instruction arity"))
        }
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

fn encode_instr_group(instr: &InstrGroup) -> Value {
    match instr {
        InstrGroup::Result(ResultGroupInstr {
            rel_signature,
            exps_output,
        }) => json!([
            "ResultI",
            encode_rel_signature(rel_signature),
            il::encode_list(exps_output, encode_exp)
        ]),
        InstrGroup::Return(ReturnGroupInstr { exp }) => json!(["ReturnI", encode_exp(exp)]),
        InstrGroup::Rule(RuleGroupInstr {
            id,
            not_exp,
            input_hint,
            iter_instrs,
        }) => json!([
            "RuleI",
            il::encode_id(id),
            mixfix::encode(not_exp, encode_exp),
            il::encode_input_hint(input_hint),
            il::encode_list(iter_instrs, il::encode_iter_prem)
        ]),
        InstrGroup::Backtrack(BacktrackGroupInstr { blocks }) => json!([
            "BacktrackI",
            blocks
                .iter()
                .map(|block| encode_block(block, encode_instr_group))
                .collect::<Vec<_>>()
        ]),
    }
}

fn decode_instr_dispatch(value: &Value) -> Result<InstrDispatch, DecodeError> {
    let (tag, fields) = variant(value)?;
    match (tag, fields) {
        ("GroupI", [id_group, id_rel, rel_signature, exps_input, block]) => {
            Ok(InstrDispatch::Group(GroupDispatchInstr {
                id_rel: il::decode_id(id_rel)?,
                id_group: il::decode_id(id_group)?,
                rel_signature: decode_rel_signature(rel_signature)?,
                exps_input: il::decode_list(exps_input, decode_exp)?,
                block: decode_block(block, decode_instr_group)?,
            }))
        }
        ("RouteI", [arms]) => Ok(InstrDispatch::Route(RouteDispatchInstr {
            blocks: il::decode_list(arms, |block| decode_block(block, decode_instr_dispatch))?,
        })),
        ("GroupI" | "RouteI", _) => {
            Err(DecodeError::Expected("valid PL dispatch instruction arity"))
        }
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

fn encode_instr_dispatch(instr: &InstrDispatch) -> Value {
    match instr {
        InstrDispatch::Group(GroupDispatchInstr {
            id_rel,
            id_group,
            rel_signature,
            exps_input,
            block,
        }) => json!([
            "GroupI",
            il::encode_id(id_group),
            il::encode_id(id_rel),
            encode_rel_signature(rel_signature),
            il::encode_list(exps_input, encode_exp),
            encode_block(block, encode_instr_group)
        ]),
        InstrDispatch::Route(RouteDispatchInstr { blocks }) => json!([
            "RouteI",
            blocks
                .iter()
                .map(|block| encode_block(block, encode_instr_dispatch))
                .collect::<Vec<_>>()
        ]),
    }
}

fn decode_extern_rel(value: &Value) -> Result<ast::ExternRel, DecodeError> {
    match array(value)? {
        [id, rel_signature, exps_input] => Ok(ast::ExternRel {
            id: il::decode_id(id)?,
            rel_signature: decode_rel_signature(rel_signature)?,
            exps_input: il::decode_list(exps_input, decode_exp)?,
        }),
        _ => Err(DecodeError::Expected("PL external relation triple")),
    }
}
fn encode_extern_rel(relation: &ast::ExternRel) -> Value {
    json!([
        il::encode_id(&relation.id),
        encode_rel_signature(&relation.rel_signature),
        il::encode_list(&relation.exps_input, encode_exp)
    ])
}

fn decode_rel(value: &Value) -> Result<ast::Rel, DecodeError> {
    match array(value)? {
        [id, rel_signature, exps_input, block, block_else_opt] => Ok(ast::Rel {
            id: il::decode_id(id)?,
            rel_signature: decode_rel_signature(rel_signature)?,
            exps_input: il::decode_list(exps_input, decode_exp)?,
            block: decode_block(block, decode_instr_dispatch)?,
            block_else_opt: decode_option(block_else_opt, |block| {
                decode_block(block, decode_instr_dispatch)
            })?,
        }),
        _ => Err(DecodeError::Expected("PL relation quintuple")),
    }
}
fn encode_rel(relation: &ast::Rel) -> Value {
    json!([
        il::encode_id(&relation.id),
        encode_rel_signature(&relation.rel_signature),
        il::encode_list(&relation.exps_input, encode_exp),
        encode_block(&relation.block, encode_instr_dispatch),
        encode_option(relation.block_else_opt.as_ref(), |block| encode_block(
            block,
            encode_instr_dispatch
        ))
    ])
}

fn decode_extern_func(value: &Value) -> Result<ast::ExternFunc, DecodeError> {
    match array(value)? {
        [id, tparams, params, typ] => Ok(ast::ExternFunc {
            id: il::decode_id(id)?,
            tparams: il::decode_list(tparams, il::decode_tparam)?,
            params: il::decode_list(params, decode_param)?,
            typ: il::decode_typ(typ)?,
        }),
        _ => Err(DecodeError::Expected("PL function quadruple")),
    }
}
fn encode_extern_func(function: &ast::ExternFunc) -> Value {
    json!([
        il::encode_id(&function.id),
        il::encode_list(&function.tparams, il::encode_tparam),
        il::encode_list(&function.params, encode_param),
        il::encode_typ(&function.typ)
    ])
}

fn decode_builtin_func(value: &Value) -> Result<ast::BuiltinFunc, DecodeError> {
    let function = decode_extern_func(value)?;
    Ok(ast::BuiltinFunc {
        id: function.id,
        tparams: function.tparams,
        params: function.params,
        typ: function.typ,
    })
}

fn encode_builtin_func(function: &ast::BuiltinFunc) -> Value {
    json!([
        il::encode_id(&function.id),
        il::encode_list(&function.tparams, il::encode_tparam),
        il::encode_list(&function.params, encode_param),
        il::encode_typ(&function.typ)
    ])
}

fn decode_table_row(value: &Value) -> Result<ast::TableRow, DecodeError> {
    match array(value)? {
        [exps_input, exp, block] => Ok(ast::TableRow {
            exps_input: il::decode_list(exps_input, decode_exp)?,
            exp: decode_exp(exp)?,
            block: decode_block(block, decode_instr_group)?,
        }),
        _ => Err(DecodeError::Expected("PL table row triple")),
    }
}
fn encode_table_row(row: &ast::TableRow) -> Value {
    json!([
        il::encode_list(&row.exps_input, encode_exp),
        encode_exp(&row.exp),
        encode_block(&row.block, encode_instr_group)
    ])
}

fn decode_table_func(value: &Value) -> Result<ast::TableFunc, DecodeError> {
    match array(value)? {
        [id, params, typ, rows] => Ok(ast::TableFunc {
            id: il::decode_id(id)?,
            params: il::decode_list(params, decode_param)?,
            typ: il::decode_typ(typ)?,
            rows: il::decode_list(rows, decode_table_row)?,
        }),
        _ => Err(DecodeError::Expected("PL table function quadruple")),
    }
}
fn encode_table_func(function: &ast::TableFunc) -> Value {
    json!([
        il::encode_id(&function.id),
        il::encode_list(&function.params, encode_param),
        il::encode_typ(&function.typ),
        il::encode_list(&function.rows, encode_table_row)
    ])
}

fn decode_defined_func(value: &Value) -> Result<ast::DefinedFunc, DecodeError> {
    match array(value)? {
        [id, tparams, params, typ, block, block_else_opt] => Ok(ast::DefinedFunc {
            id: il::decode_id(id)?,
            tparams: il::decode_list(tparams, il::decode_tparam)?,
            params: il::decode_list(params, decode_param)?,
            typ: il::decode_typ(typ)?,
            block: decode_block(block, decode_instr_group)?,
            block_else_opt: decode_option(block_else_opt, |block| {
                decode_block(block, decode_instr_group)
            })?,
        }),
        _ => Err(DecodeError::Expected("PL defined function sextuple")),
    }
}
fn encode_defined_func(function: &ast::DefinedFunc) -> Value {
    json!([
        il::encode_id(&function.id),
        il::encode_list(&function.tparams, il::encode_tparam),
        il::encode_list(&function.params, encode_param),
        il::encode_typ(&function.typ),
        encode_block(&function.block, encode_instr_group),
        encode_option(function.block_else_opt.as_ref(), |block| encode_block(
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
            ("ExternTypD", [id]) => Ok(DefKind::ExternTyp(ExternTypDef {
                id: il::decode_id(id)?,
            })),
            ("TypD", [id, tparams, typ]) => Ok(DefKind::Typ(TypDef {
                id: il::decode_id(id)?,
                tparams: il::decode_list(tparams, il::decode_tparam)?,
                def_typ: il::decode_def_typ(typ)?,
            })),
            ("VarD", [id, typ]) => Ok(DefKind::Var(VarDef {
                id: il::decode_id(id)?,
                typ: il::decode_typ(typ)?,
            })),
            ("ExternRelD", [rel]) => Ok(DefKind::ExternRel(decode_extern_rel(rel)?)),
            ("RelD", [rel]) => Ok(DefKind::Rel(decode_rel(rel)?)),
            ("ExternDecD", [func]) => Ok(DefKind::ExternDec(decode_extern_func(func)?)),
            ("BuiltinDecD", [func]) => Ok(DefKind::BuiltinDec(decode_builtin_func(func)?)),
            ("TableDecD", [func]) => Ok(DefKind::TableDec(decode_table_func(func)?)),
            ("FuncDecD", [func]) => Ok(DefKind::FuncDec(decode_defined_func(func)?)),
            (
                "ExternTypD" | "TypD" | "VarD" | "ExternRelD" | "RelD" | "ExternDecD"
                | "BuiltinDecD" | "TableDecD" | "FuncDecD",
                _,
            ) => Err(DecodeError::Expected("valid PL definition arity")),
            (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
        }
    })?;
    let mut definition = crate::annotated! {
        node: def.node,
        span: def,
    };
    definition.hints = decode_hints(field(value, "hints")?)?;
    Ok(definition)
}

fn encode_def(def: &ast::Def) -> Value {
    let node = source::encode_phrase(
        &crate::spanned! {
            node: def.node.node.clone(),
            span: def.node.span.clone(),
        },
        |def| match def {
            DefKind::ExternTyp(ExternTypDef { id }) => json!(["ExternTypD", il::encode_id(id)]),
            DefKind::Typ(TypDef {
                id,
                tparams,
                def_typ: typ,
            }) => json!([
                "TypD",
                il::encode_id(id),
                il::encode_list(tparams, il::encode_tparam),
                il::encode_def_typ(typ)
            ]),
            DefKind::Var(VarDef { id, typ }) => {
                json!(["VarD", il::encode_id(id), il::encode_typ(typ)])
            }
            DefKind::ExternRel(rel) => json!(["ExternRelD", encode_extern_rel(rel)]),
            DefKind::Rel(rel) => json!(["RelD", encode_rel(rel)]),
            DefKind::ExternDec(func) => json!(["ExternDecD", encode_extern_func(func)]),
            DefKind::BuiltinDec(func) => json!(["BuiltinDecD", encode_builtin_func(func)]),
            DefKind::TableDec(func) => json!(["TableDecD", encode_table_func(func)]),
            DefKind::FuncDec(func) => json!(["FuncDecD", encode_defined_func(func)]),
        },
    );
    json!({"node": node, "hints": encode_hints(&def.hints)})
}
