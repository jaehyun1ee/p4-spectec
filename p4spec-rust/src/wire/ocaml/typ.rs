use super::{DecodeError, decode_list, encode_list, num as num_codec, source, string, variant};
use crate::lang::{
    common::{Id, Iter},
    data::typ::{FuncTyp, Typ, TypKind},
};
use crate::util::json::json;
use serde_json::json;

pub(crate) fn decode_id(json: &json) -> Result<Id, DecodeError> {
    source::decode_phrase(json, |json| Ok(string(json)?.to_owned()))
}

pub(crate) fn encode_id(id: &Id) -> json {
    source::encode_phrase(id, |id| json!(id))
}

pub(crate) fn decode_iter(json: &json) -> Result<Iter, DecodeError> {
    let (tag, fields) = variant(json)?;
    match (tag, fields) {
        ("Opt", []) => Ok(Iter::Opt),
        ("List", []) => Ok(Iter::List),
        ("Opt" | "List", _) => Err(DecodeError::Expected("valid IL iterator arity")),
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

pub(crate) fn encode_iter(iter: Iter) -> json {
    match iter {
        Iter::Opt => json!(["Opt"]),
        Iter::List => json!(["List"]),
    }
}

pub(crate) fn decode_typ(json: &json) -> Result<Typ, DecodeError> {
    source::decode_phrase(json, decode_typ_kind)
}

pub(crate) fn encode_typ(typ: &Typ) -> json {
    source::encode_phrase(typ, encode_typ_kind)
}

pub(crate) fn decode_targ(json: &json) -> Result<Typ, DecodeError> {
    source::decode_phrase(json, decode_typ_kind)
}

pub(crate) fn encode_targ(targ: &Typ) -> json {
    source::encode_phrase(targ, encode_typ_kind)
}

pub(crate) fn decode_tparam(json: &json) -> Result<Id, DecodeError> {
    source::decode_phrase(json, |json| Ok(string(json)?.to_owned()))
}

pub(crate) fn encode_tparam(tparam: &Id) -> json {
    source::encode_phrase(tparam, |tparam| json!(tparam))
}

pub(crate) fn decode_typ_kind(json: &json) -> Result<TypKind, DecodeError> {
    let (tag, fields) = variant(json)?;
    match (tag, fields) {
        ("BoolT", []) => Ok(TypKind::Bool),
        ("NumT", [typ]) => Ok(TypKind::Num(num_codec::decode_num_typ(typ)?)),
        ("TextT", []) => Ok(TypKind::Text),
        ("VarT", [id, targs]) => Ok(TypKind::Var(
            decode_id(id)?,
            decode_list(targs, decode_targ)?,
        )),
        ("TupleT", [typs]) => Ok(TypKind::Tuple(decode_list(typs, decode_typ)?)),
        ("IterT", [typ, iter]) => Ok(TypKind::Iter(
            Box::new(decode_typ(typ)?),
            decode_iter(iter)?,
        )),
        ("FuncT", [tparams, params, result]) => {
            let tparams = decode_list(tparams, decode_tparam)?;
            let typs_params = decode_list(params, decode_typ)?;
            let typ_ret = decode_typ(result)?;
            let typ_ret = Box::new(typ_ret);
            let typ_func = FuncTyp {
                tparams,
                typs_params,
                typ_ret,
            };
            Ok(TypKind::Func(typ_func))
        }
        ("BoolT" | "NumT" | "TextT" | "VarT" | "TupleT" | "IterT" | "FuncT", _) => {
            Err(DecodeError::Expected("valid IL type arity"))
        }
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

pub(crate) fn encode_typ_kind(typ: &TypKind) -> json {
    match typ {
        TypKind::Bool => json!(["BoolT"]),
        TypKind::Num(typ) => json!(["NumT", num_codec::encode_num_typ(*typ)]),
        TypKind::Text => json!(["TextT"]),
        TypKind::Var(id, targs) => {
            json!(["VarT", encode_id(id), encode_list(targs, encode_targ)])
        }
        TypKind::Tuple(typs) => json!(["TupleT", encode_list(typs, encode_typ)]),
        TypKind::Iter(typ, iter) => json!(["IterT", encode_typ(typ), encode_iter(*iter)]),
        TypKind::Func(typ_func) => json!([
            "FuncT",
            encode_list(&typ_func.tparams, encode_tparam),
            encode_list(&typ_func.typs_params, encode_typ),
            encode_typ(&typ_func.typ_ret)
        ]),
    }
}
