use serde_json::json;

use crate::util::json::json;

use crate::{
    lang::{
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
    pub fn decode(json: &json) -> Result<ast::Exp, DecodeError> {
        on_codec_stack(|| decode_exp(json))
    }

    pub fn encode(exp: &ast::Exp) -> json {
        on_codec_stack(|| encode_exp(exp))
    }
}

pub struct PathCodec;

impl PathCodec {
    pub fn decode(json: &json) -> Result<ast::Path, DecodeError> {
        on_codec_stack(|| decode_path(json))
    }

    pub fn encode(path: &ast::Path) -> json {
        on_codec_stack(|| encode_path(path))
    }
}

pub struct SpecCodec;

impl SpecCodec {
    pub fn decode(json: &json) -> Result<ast::Spec, DecodeError> {
        on_codec_stack(|| il::decode_list(json, decode_def))
    }

    pub fn encode(spec: &ast::Spec) -> Result<json, EncodeError> {
        on_codec_stack(|| Ok(il::encode_list(spec, encode_def)))
    }
}

fn decode_option<T>(
    json: &json,
    decode: impl FnOnce(&json) -> Result<T, DecodeError>,
) -> Result<Option<T>, DecodeError> {
    if json.is_null() {
        Ok(None)
    } else {
        Ok(Some(decode(json)?))
    }
}

fn encode_option<T>(value: Option<&T>, encode: impl FnOnce(&T) -> json) -> json {
    value.map_or(json::Null, encode)
}

fn decode_alter(json: &json) -> Result<alter::AlterationHint, DecodeError> {
    let (tag, fields) = variant(json)?;
    match (tag, fields) {
        ("TextH", [text]) => Ok(alter::AlterationHint::Text(string(text)?.to_owned())),
        ("AtomH", [atom]) => Ok(alter::AlterationHint::Atom(AtomPhraseCodec::decode(atom)?)),
        ("SeqH", [hints]) => Ok(alter::AlterationHint::Seq(il::decode_list(
            hints,
            decode_alter,
        )?)),
        ("BrackH", [atom_l, hint, atom_r]) => Ok(alter::AlterationHint::Brack(
            AtomPhraseCodec::decode(atom_l)?,
            Box::new(decode_alter(hint)?),
            AtomPhraseCodec::decode(atom_r)?,
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
        ("FuseH", [hint_l, hint_r]) => Ok(alter::AlterationHint::Fuse(
            Box::new(decode_alter(hint_l)?),
            Box::new(decode_alter(hint_r)?),
        )),
        ("OtherH", [exp]) => Ok(alter::AlterationHint::Other(el::decode_exp(exp)?)),
        ("TextH" | "AtomH" | "SeqH" | "BrackH" | "HoleH" | "FuseH" | "OtherH", _) => {
            Err(DecodeError::Expected("valid PL alter hint arity"))
        }
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

fn encode_alter(hint: &alter::AlterationHint) -> json {
    match hint {
        alter::AlterationHint::Text(text) => json!(["TextH", text]),
        alter::AlterationHint::Atom(atom) => json!(["AtomH", AtomPhraseCodec::encode(atom)]),
        alter::AlterationHint::Seq(hints) => {
            json!(["SeqH", il::encode_list(hints, encode_alter)])
        }
        alter::AlterationHint::Brack(atom_l, hint, atom_r) => json!([
            "BrackH",
            AtomPhraseCodec::encode(atom_l),
            encode_alter(hint),
            AtomPhraseCodec::encode(atom_r)
        ]),
        alter::AlterationHint::Hole(alter::Hole::Next) => json!(["HoleH", ["Next"]]),
        alter::AlterationHint::Hole(alter::Hole::Num(index)) => {
            json!(["HoleH", ["Num", index]])
        }
        alter::AlterationHint::Fuse(hint_l, hint_r) => {
            json!(["FuseH", encode_alter(hint_l), encode_alter(hint_r)])
        }
        alter::AlterationHint::Other(exp) => json!(["OtherH", el::encode_exp(exp)]),
    }
}

fn decode_hints(json: &json) -> Result<annot::Hints, DecodeError> {
    let json = object(json)?;
    Ok(annot::Hints {
        prose: decode_option(field(json, "prose")?, decode_alter)?,
        prose_in: decode_option(field(json, "prose_in")?, decode_alter)?,
        prose_out: decode_option(field(json, "prose_out")?, decode_alter)?,
        prose_true: decode_option(field(json, "prose_true")?, decode_alter)?,
        prose_false: decode_option(field(json, "prose_false")?, decode_alter)?,
        prose_fields: decode_option(field(json, "prose_fields")?, |json| {
            Ok(fields::FieldHint::new(il::decode_list(json, |json| {
                Ok(string(json)?.to_owned())
            })?))
        })?,
        prose_input_exps: decode_option(field(json, "prose_input_exps")?, |json| {
            il::decode_list(json, il::decode_exp)
        })?,
        prose_output_exps: decode_option(field(json, "prose_output_exps")?, |json| {
            il::decode_list(json, il::decode_exp)
        })?,
    })
}

fn encode_hints(hints: &annot::Hints) -> json {
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

fn decode_exp(json: &json) -> Result<ast::Exp, DecodeError> {
    let json = object(json)?;
    Ok(annot::Annotated {
        node: decode_exp_node(field(json, "node")?)?,
        hints: decode_hints(field(json, "hints")?)?,
    })
}

fn encode_exp(exp: &ast::Exp) -> json {
    json!({"node": encode_exp_node(&exp.node), "hints": encode_hints(&exp.hints)})
}

fn decode_exp_node(json: &json) -> Result<ast::ExpNode, DecodeError> {
    source::decode_note_phrase(json, decode_exp_kind, il::decode_typ_kind)
}

fn encode_exp_node(exp: &ast::ExpNode) -> json {
    source::encode_note_phrase(exp, encode_exp_kind, il::encode_typ_kind)
}

fn decode_exp_kind(json: &json) -> Result<ExpKind, DecodeError> {
    let (tag, fields) = variant(json)?;
    match (tag, fields) {
        ("BoolE", [json]) => Ok(ExpKind::Bool(boolean(json)?)),
        ("NumE", [num]) => Ok(ExpKind::Num(xl::decode_num(num)?)),
        ("TextE", [text]) => Ok(ExpKind::Text(string(text)?.to_owned())),
        ("VarE", [id]) => Ok(ExpKind::Var(il::decode_id(id)?)),
        ("UnE", [op, typ, exp]) => Ok(ExpKind::Un(
            il::decode_un_op(op)?,
            il::decode_op_typ(typ)?,
            Box::new(decode_exp(exp)?),
        )),
        ("BinE", [op, typ, exp_l, exp_r]) => Ok(ExpKind::Bin(
            il::decode_bin_op(op)?,
            il::decode_op_typ(typ)?,
            Box::new(decode_exp(exp_l)?),
            Box::new(decode_exp(exp_r)?),
        )),
        ("CmpE", [op, typ, exp_l, exp_r]) => Ok(ExpKind::Cmp(
            il::decode_cmp_op(op)?,
            il::decode_op_typ(typ)?,
            Box::new(decode_exp(exp_l)?),
            Box::new(decode_exp(exp_r)?),
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
            |json| match array(json)? {
                [atom, exp] => Ok((AtomPhraseCodec::decode(atom)?, decode_exp(exp)?)),
                _ => Err(DecodeError::Expected("PL structure field pair")),
            },
        )?)),
        ("OptE", [exp]) => Ok(ExpKind::Opt(decode_option(exp, |exp| {
            Ok(Box::new(decode_exp(exp)?))
        })?)),
        ("ListE", [exps]) => Ok(ExpKind::List(il::decode_list(exps, decode_exp)?)),
        ("ConsE", [exp_head, exp_tail]) => Ok(ExpKind::Cons(
            Box::new(decode_exp(exp_head)?),
            Box::new(decode_exp(exp_tail)?),
        )),
        ("CatE", [exp_l, exp_r]) => Ok(ExpKind::Cat(
            Box::new(decode_exp(exp_l)?),
            Box::new(decode_exp(exp_r)?),
        )),
        ("MemE", [exp_l, exp_r]) => Ok(ExpKind::Mem(
            Box::new(decode_exp(exp_l)?),
            Box::new(decode_exp(exp_r)?),
        )),
        ("LenE", [exp]) => Ok(ExpKind::Len(Box::new(decode_exp(exp)?))),
        ("DotE", [exp, atom]) => Ok(ExpKind::Dot(
            Box::new(decode_exp(exp)?),
            AtomPhraseCodec::decode(atom)?,
        )),
        ("IdxE", [exp_base, exp_idx]) => Ok(ExpKind::Idx(
            Box::new(decode_exp(exp_base)?),
            Box::new(decode_exp(exp_idx)?),
        )),
        ("SliceE", [exp_base, exp_idx, exp_len]) => Ok(ExpKind::Slice(
            Box::new(decode_exp(exp_base)?),
            Box::new(decode_exp(exp_idx)?),
            Box::new(decode_exp(exp_len)?),
        )),
        ("UpdE", [exp_base, path, exp_field]) => Ok(ExpKind::Upd(
            Box::new(decode_exp(exp_base)?),
            Box::new(decode_path(path)?),
            Box::new(decode_exp(exp_field)?),
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

fn encode_exp_kind(exp: &ExpKind) -> json {
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
        ExpKind::Bin(op, typ, exp_l, exp_r) => json!([
            "BinE",
            il::encode_bin_op(*op),
            il::encode_op_typ(*typ),
            encode_exp(exp_l),
            encode_exp(exp_r)
        ]),
        ExpKind::Cmp(op, typ, exp_l, exp_r) => json!([
            "CmpE",
            il::encode_cmp_op(*op),
            il::encode_op_typ(*typ),
            encode_exp(exp_l),
            encode_exp(exp_r)
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
        ExpKind::Cons(exp_head, exp_tail) => {
            json!(["ConsE", encode_exp(exp_head), encode_exp(exp_tail)])
        }
        ExpKind::Cat(exp_l, exp_r) => json!(["CatE", encode_exp(exp_l), encode_exp(exp_r)]),
        ExpKind::Mem(exp_l, exp_r) => json!(["MemE", encode_exp(exp_l), encode_exp(exp_r)]),
        ExpKind::Len(exp) => json!(["LenE", encode_exp(exp)]),
        ExpKind::Dot(exp, atom) => json!(["DotE", encode_exp(exp), AtomPhraseCodec::encode(atom)]),
        ExpKind::Idx(exp_base, exp_idx) => {
            json!(["IdxE", encode_exp(exp_base), encode_exp(exp_idx)])
        }
        ExpKind::Slice(exp_base, exp_idx, exp_len) => json!([
            "SliceE",
            encode_exp(exp_base),
            encode_exp(exp_idx),
            encode_exp(exp_len)
        ]),
        ExpKind::Upd(exp_base, path, exp_field) => {
            json!([
                "UpdE",
                encode_exp(exp_base),
                encode_path(path),
                encode_exp(exp_field)
            ])
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

fn decode_path(json: &json) -> Result<ast::Path, DecodeError> {
    source::decode_note_phrase(json, decode_path_kind, il::decode_typ_kind)
}

fn encode_path(path: &ast::Path) -> json {
    source::encode_note_phrase(path, encode_path_kind, il::encode_typ_kind)
}

fn decode_path_kind(json: &json) -> Result<PathKind, DecodeError> {
    let (tag, fields) = variant(json)?;
    match (tag, fields) {
        ("RootP", []) => Ok(PathKind::Root),
        ("IdxP", [path, exp_idx]) => Ok(PathKind::Idx(
            Box::new(decode_path(path)?),
            Box::new(decode_exp(exp_idx)?),
        )),
        ("SliceP", [path, exp_idx, exp_len]) => Ok(PathKind::Slice(
            Box::new(decode_path(path)?),
            Box::new(decode_exp(exp_idx)?),
            Box::new(decode_exp(exp_len)?),
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

fn encode_path_kind(path: &PathKind) -> json {
    match path {
        PathKind::Root => json!(["RootP"]),
        PathKind::Idx(path, exp_idx) => json!(["IdxP", encode_path(path), encode_exp(exp_idx)]),
        PathKind::Slice(path, exp_idx, exp_len) => json!([
            "SliceP",
            encode_path(path),
            encode_exp(exp_idx),
            encode_exp(exp_len)
        ]),
        PathKind::Dot(path, atom) => {
            json!(["DotP", encode_path(path), AtomPhraseCodec::encode(atom)])
        }
    }
}

fn decode_arg(json: &json) -> Result<ast::Arg, DecodeError> {
    source::decode_phrase(json, |json| {
        let (tag, fields) = variant(json)?;
        match (tag, fields) {
            ("ExpA", [exp]) => Ok(ArgKind::Exp(Box::new(decode_exp(exp)?))),
            ("DefA", [id]) => Ok(ArgKind::Def(il::decode_id(id)?)),
            ("ExpA" | "DefA", _) => Err(DecodeError::Expected("valid PL argument arity")),
            (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
        }
    })
}

fn encode_arg(arg: &ast::Arg) -> json {
    source::encode_phrase(arg, |arg| match arg {
        ArgKind::Exp(exp) => json!(["ExpA", encode_exp(exp)]),
        ArgKind::Def(id) => json!(["DefA", il::encode_id(id)]),
    })
}

fn decode_param(json: &json) -> Result<ast::Param, DecodeError> {
    source::decode_phrase(json, |json| {
        let (tag, fields) = variant(json)?;
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

fn encode_param(param: &ast::Param) -> json {
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

fn decode_fallthrough(json: &json) -> Result<Fallthrough, DecodeError> {
    let (tag, fields) = variant(json)?;
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

fn encode_fallthrough(fallthrough: &Fallthrough) -> json {
    match fallthrough {
        Fallthrough::FallGroup(id) => json!(["FallGroup", il::encode_id(id)]),
        Fallthrough::FallNext => json!(["FallNext"]),
        Fallthrough::FallElse => json!(["FallElse"]),
        Fallthrough::FallFail => json!(["FallFail"]),
    }
}

fn decode_instr_note(json: &json) -> Result<Option<Fallthrough>, DecodeError> {
    let json = object(json)?;
    integer(field(json, "iid")?)?;
    let fallthrough = decode_option(field(json, "fallthrough")?, decode_fallthrough)?;
    Ok(fallthrough)
}

fn encode_instr_note(fallthrough: &Option<Fallthrough>) -> json {
    // OCaml still requires an instruction identifier in its wire format.
    json!({"iid": 0, "fallthrough": encode_option(fallthrough.as_ref(), encode_fallthrough)})
}

fn decode_guard(json: &json) -> Result<Guard, DecodeError> {
    let (tag, fields) = variant(json)?;
    match (tag, fields) {
        ("BoolG", [json]) => Ok(Guard::Bool(boolean(json)?)),
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

fn encode_guard(guard: &Guard) -> json {
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
    json: &json,
    decode_tier: fn(&json) -> Result<T, DecodeError>,
) -> Result<ast::Instr<T>, DecodeError> {
    let json = object(json)?;
    let node = source::decode_note_phrase(
        field(json, "node")?,
        |json| decode_instr_kind(json, decode_tier),
        decode_instr_note,
    )?;
    let hints = decode_hints(field(json, "hints")?)?;
    Ok(annot::Annotated { node, hints })
}

fn encode_instr<T>(instr: &ast::Instr<T>, encode_tier: fn(&T) -> json) -> json {
    json!({
        "node": source::encode_note_phrase(&instr.node, |kind| encode_instr_kind(kind, encode_tier), encode_instr_note),
        "hints": encode_hints(&instr.hints)
    })
}

fn decode_block<T>(
    json: &json,
    decode_tier: fn(&json) -> Result<T, DecodeError>,
) -> Result<ast::Block<T>, DecodeError> {
    il::decode_list(json, |json| decode_instr(json, decode_tier))
}

fn encode_block<T>(block: &ast::Block<T>, encode_tier: fn(&T) -> json) -> json {
    il::encode_list(block, |instr| encode_instr(instr, encode_tier))
}

fn decode_hold_case<T>(
    json: &json,
    decode_tier: fn(&json) -> Result<T, DecodeError>,
) -> Result<HoldCase<T>, DecodeError> {
    let (tag, fields) = variant(json)?;
    match (tag, fields) {
        ("BothH", [block_hold, block_not_hold]) => Ok(HoldCase::Both(
            decode_block(block_hold, decode_tier)?,
            decode_block(block_not_hold, decode_tier)?,
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

fn encode_hold_case<T>(case: &HoldCase<T>, encode_tier: fn(&T) -> json) -> json {
    match case {
        HoldCase::Both(block_hold, block_not_hold) => json!([
            "BothH",
            encode_block(block_hold, encode_tier),
            encode_block(block_not_hold, encode_tier)
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
    json: &json,
    decode_tier: fn(&json) -> Result<T, DecodeError>,
) -> Result<InstrKind<T>, DecodeError> {
    let (tag, fields) = variant(json)?;
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
            cases: il::decode_list(cases, |json| match array(json)? {
                [guard, block] => Ok(ast::Case {
                    guard: decode_guard(guard)?,
                    block: decode_block(block, decode_tier)?,
                }),
                _ => Err(DecodeError::Expected("PL case pair")),
            })?,
            dangle: boolean(dangle)?,
        })),
        ("LetI", [exp_l, exp_r, iters]) => Ok(InstrKind::Let(LetInstr {
            exp_l: decode_exp(exp_l)?,
            exp_r: decode_exp(exp_r)?,
            iter_instrs: il::decode_list(iters, il::decode_prem_iter)?,
        })),
        ("DebugI", [exp]) => Ok(InstrKind::Debug(DebugInstr {
            exp: decode_exp(exp)?,
        })),
        ("DestructI", [bindings, exp]) => Ok(InstrKind::Destruct(DestructInstr {
            bindings: il::decode_list(bindings, |json| match array(json)? {
                [name, exp] => Ok((
                    decode_option(name, |name| Ok(string(name)?.to_owned()))?,
                    decode_exp(exp)?,
                )),
                _ => Err(DecodeError::Expected("PL destruct binding pair")),
            })?,
            exp: decode_exp(exp)?,
        })),
        ("CheckLetSubI", [typ, subcheck, exp_l, exp_r, block]) => {
            Ok(InstrKind::CheckLetSub(CheckLetSubInstr {
                typ: il::decode_typ(typ)?,
                subcheck: Box::new(il::decode_subcheck(subcheck)?),
                exp_l: decode_exp(exp_l)?,
                exp_r: decode_exp(exp_r)?,
                block: decode_block(block, decode_tier)?,
            }))
        }
        ("CheckLetMatchI", [pattern, exp_l, exp_r, block]) => {
            Ok(InstrKind::CheckLetMatch(CheckLetMatchInstr {
                pattern: il::decode_pattern(pattern)?,
                exp_l: decode_exp(exp_l)?,
                exp_r: decode_exp(exp_r)?,
                block: decode_block(block, decode_tier)?,
            }))
        }
        ("OptionGetI", [exp_l, exp_r, block]) => Ok(InstrKind::OptionGet(OptionGetInstr {
            exp_l: decode_exp(exp_l)?,
            exp_r: decode_exp(exp_r)?,
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

fn encode_instr_kind<T>(instr: &InstrKind<T>, encode_tier: fn(&T) -> json) -> json {
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
            exp_l,
            exp_r,
            iter_instrs: iters,
        }) => json!([
            "LetI",
            encode_exp(exp_l),
            encode_exp(exp_r),
            il::encode_list(iters, il::encode_prem_iter)
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
            exp_l,
            exp_r,
            block,
        }) => json!([
            "CheckLetSubI",
            il::encode_typ(typ),
            il::encode_subcheck(subcheck),
            encode_exp(exp_l),
            encode_exp(exp_r),
            encode_block(block, encode_tier)
        ]),
        InstrKind::CheckLetMatch(CheckLetMatchInstr {
            pattern,
            exp_l,
            exp_r,
            block,
        }) => json!([
            "CheckLetMatchI",
            il::encode_pattern(pattern),
            encode_exp(exp_l),
            encode_exp(exp_r),
            encode_block(block, encode_tier)
        ]),
        InstrKind::OptionGet(OptionGetInstr {
            exp_l,
            exp_r,
            block,
        }) => json!([
            "OptionGetI",
            encode_exp(exp_l),
            encode_exp(exp_r),
            encode_block(block, encode_tier)
        ]),
        InstrKind::Tier(TierInstr { tier }) => json!(["TierI", encode_tier(tier)]),
    }
}

fn decode_rel_signature(json: &json) -> Result<ast::RelSignature, DecodeError> {
    match array(json)? {
        [typ, input] => Ok(ast::RelSignature {
            not_typ: il::decode_not_typ(typ)?,
            input_hint: il::decode_input_hint(input)?,
        }),
        _ => Err(DecodeError::Expected("PL relation signature pair")),
    }
}

fn encode_rel_signature(rel_signature: &ast::RelSignature) -> json {
    json!([
        il::encode_not_typ(&rel_signature.not_typ),
        il::encode_input_hint(&rel_signature.input_hint)
    ])
}

fn decode_instr_group(json: &json) -> Result<InstrGroup, DecodeError> {
    let (tag, fields) = variant(json)?;
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
            iter_instrs: il::decode_list(iter_instrs, il::decode_prem_iter)?,
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

fn encode_instr_group(instr: &InstrGroup) -> json {
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
            il::encode_list(iter_instrs, il::encode_prem_iter)
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

fn decode_instr_dispatch(json: &json) -> Result<InstrDispatch, DecodeError> {
    let (tag, fields) = variant(json)?;
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

fn encode_instr_dispatch(instr: &InstrDispatch) -> json {
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

fn decode_extern_rel(json: &json) -> Result<ast::ExternRel, DecodeError> {
    match array(json)? {
        [id, rel_signature, exps_input] => Ok(ast::ExternRel {
            id: il::decode_id(id)?,
            rel_signature: decode_rel_signature(rel_signature)?,
            exps_input: il::decode_list(exps_input, decode_exp)?,
        }),
        _ => Err(DecodeError::Expected("PL external relation triple")),
    }
}
fn encode_extern_rel(relation: &ast::ExternRel) -> json {
    json!([
        il::encode_id(&relation.id),
        encode_rel_signature(&relation.rel_signature),
        il::encode_list(&relation.exps_input, encode_exp)
    ])
}

fn decode_defined_rel(json: &json) -> Result<ast::DefinedRel, DecodeError> {
    match array(json)? {
        [id, rel_signature, exps_input, block, block_else_opt] => Ok(ast::DefinedRel {
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
fn encode_defined_rel(relation: &ast::DefinedRel) -> json {
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

fn decode_extern_func(json: &json) -> Result<ast::ExternFunc, DecodeError> {
    match array(json)? {
        [id, tparams, params, typ] => Ok(ast::ExternFunc {
            id: il::decode_id(id)?,
            tparams: il::decode_list(tparams, il::decode_tparam)?,
            params: il::decode_list(params, decode_param)?,
            typ: il::decode_typ(typ)?,
        }),
        _ => Err(DecodeError::Expected("PL function quadruple")),
    }
}
fn encode_extern_func(func: &ast::ExternFunc) -> json {
    json!([
        il::encode_id(&func.id),
        il::encode_list(&func.tparams, il::encode_tparam),
        il::encode_list(&func.params, encode_param),
        il::encode_typ(&func.typ)
    ])
}

fn decode_builtin_func(json: &json) -> Result<ast::BuiltinFunc, DecodeError> {
    let func = decode_extern_func(json)?;
    Ok(ast::BuiltinFunc {
        id: func.id,
        tparams: func.tparams,
        params: func.params,
        typ: func.typ,
    })
}

fn encode_builtin_func(func: &ast::BuiltinFunc) -> json {
    json!([
        il::encode_id(&func.id),
        il::encode_list(&func.tparams, il::encode_tparam),
        il::encode_list(&func.params, encode_param),
        il::encode_typ(&func.typ)
    ])
}

fn decode_table_row(json: &json) -> Result<ast::TableRow, DecodeError> {
    match array(json)? {
        [exps_input, exp, block] => Ok(ast::TableRow {
            exps_input: il::decode_list(exps_input, decode_exp)?,
            exp: decode_exp(exp)?,
            block: decode_block(block, decode_instr_group)?,
        }),
        _ => Err(DecodeError::Expected("PL table row triple")),
    }
}
fn encode_table_row(row: &ast::TableRow) -> json {
    json!([
        il::encode_list(&row.exps_input, encode_exp),
        encode_exp(&row.exp),
        encode_block(&row.block, encode_instr_group)
    ])
}

fn decode_table_func(json: &json) -> Result<ast::TableFunc, DecodeError> {
    match array(json)? {
        [id, params, typ, rows] => Ok(ast::TableFunc {
            id: il::decode_id(id)?,
            params: il::decode_list(params, decode_param)?,
            typ: il::decode_typ(typ)?,
            rows: il::decode_list(rows, decode_table_row)?,
        }),
        _ => Err(DecodeError::Expected("PL table function quadruple")),
    }
}
fn encode_table_func(func: &ast::TableFunc) -> json {
    json!([
        il::encode_id(&func.id),
        il::encode_list(&func.params, encode_param),
        il::encode_typ(&func.typ),
        il::encode_list(&func.rows, encode_table_row)
    ])
}

fn decode_defined_func(json: &json) -> Result<ast::DefinedFunc, DecodeError> {
    match array(json)? {
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
fn encode_defined_func(func: &ast::DefinedFunc) -> json {
    json!([
        il::encode_id(&func.id),
        il::encode_list(&func.tparams, il::encode_tparam),
        il::encode_list(&func.params, encode_param),
        il::encode_typ(&func.typ),
        encode_block(&func.block, encode_instr_group),
        encode_option(func.block_else_opt.as_ref(), |block| encode_block(
            block,
            encode_instr_group
        ))
    ])
}

fn decode_def(json: &json) -> Result<ast::Def, DecodeError> {
    let json = object(json)?;
    let def = source::decode_phrase(field(json, "node")?, |json| {
        let (tag, fields) = variant(json)?;
        match (tag, fields) {
            ("ExternTypD", [id]) => Ok(DefKind::Typ(TypDef::Extern(ExternTyp {
                id: il::decode_id(id)?,
            }))),
            ("TypD", [id, tparams, typ]) => {
                Ok(DefKind::Typ(TypDef::Defined(Box::new(DefinedTyp {
                    id: il::decode_id(id)?,
                    tparams: il::decode_list(tparams, il::decode_tparam)?,
                    def_typ: il::decode_def_typ(typ)?,
                }))))
            }
            ("VarD", [id, typ]) => Ok(DefKind::Var(VarDef {
                id: il::decode_id(id)?,
                typ: il::decode_typ(typ)?,
            })),
            ("ExternRelD", [rel_value]) => {
                Ok(DefKind::Rel(RelDef::Extern(decode_extern_rel(rel_value)?)))
            }
            ("RelD", [rel_value]) => Ok(DefKind::Rel(RelDef::Defined(decode_defined_rel(
                rel_value,
            )?))),
            ("ExternDecD", [func_value]) => Ok(DefKind::MetaFunc(MetaFuncDef::Extern(
                decode_extern_func(func_value)?,
            ))),
            ("BuiltinDecD", [func_value]) => Ok(DefKind::MetaFunc(MetaFuncDef::Builtin(
                decode_builtin_func(func_value)?,
            ))),
            ("TableDecD", [func_value]) => Ok(DefKind::MetaFunc(MetaFuncDef::Table(
                decode_table_func(func_value)?,
            ))),
            ("FuncDecD", [func_value]) => Ok(DefKind::MetaFunc(MetaFuncDef::Defined(
                decode_defined_func(func_value)?,
            ))),
            (
                "ExternTypD" | "TypD" | "VarD" | "ExternRelD" | "RelD" | "ExternDecD"
                | "BuiltinDecD" | "TableDecD" | "FuncDecD",
                _,
            ) => Err(DecodeError::Expected("valid PL definition arity")),
            (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
        }
    })?;
    let mut def = crate::annotated! {
        node: def.node,
        span: def,
    };
    def.hints = decode_hints(field(json, "hints")?)?;
    Ok(def)
}

fn encode_typ_def(typ_def_pl: &TypDef) -> json {
    match typ_def_pl {
        TypDef::Extern(extern_typ_pl) => {
            json!(["ExternTypD", il::encode_id(&extern_typ_pl.id)])
        }
        TypDef::Defined(defined_typ_pl) => json!([
            "TypD",
            il::encode_id(&defined_typ_pl.id),
            il::encode_list(&defined_typ_pl.tparams, il::encode_tparam),
            il::encode_def_typ(&defined_typ_pl.def_typ)
        ]),
    }
}

fn encode_var_def(var_def_pl: &VarDef) -> json {
    json!([
        "VarD",
        il::encode_id(&var_def_pl.id),
        il::encode_typ(&var_def_pl.typ)
    ])
}

fn encode_rel_def(rel_def_pl: &RelDef) -> json {
    match rel_def_pl {
        RelDef::Extern(extern_rel_pl) => {
            json!(["ExternRelD", encode_extern_rel(extern_rel_pl)])
        }
        RelDef::Defined(defined_rel_pl) => {
            json!(["RelD", encode_defined_rel(defined_rel_pl)])
        }
    }
}

fn encode_meta_func_def(meta_func_def_pl: &MetaFuncDef) -> json {
    match meta_func_def_pl {
        MetaFuncDef::Extern(extern_func_pl) => {
            json!(["ExternDecD", encode_extern_func(extern_func_pl)])
        }
        MetaFuncDef::Builtin(builtin_func_pl) => {
            json!(["BuiltinDecD", encode_builtin_func(builtin_func_pl)])
        }
        MetaFuncDef::Table(table_func_pl) => {
            json!(["TableDecD", encode_table_func(table_func_pl)])
        }
        MetaFuncDef::Defined(defined_func_pl) => {
            json!(["FuncDecD", encode_defined_func(defined_func_pl)])
        }
    }
}

fn encode_def(def_pl: &ast::Def) -> json {
    let node = source::encode_phrase(
        &crate::phrase! {
            node: def_pl.node.node.clone(),
            span: def_pl.node.span.clone(),
        },
        |def_kind_pl| match def_kind_pl {
            DefKind::Typ(typ_def_pl) => encode_typ_def(typ_def_pl),
            DefKind::Var(var_def_pl) => encode_var_def(var_def_pl),
            DefKind::Rel(rel_def_pl) => encode_rel_def(rel_def_pl),
            DefKind::MetaFunc(meta_func_def_pl) => encode_meta_func_def(meta_func_def_pl),
        },
    );
    json!({"node": node, "hints": encode_hints(&def_pl.hints)})
}
