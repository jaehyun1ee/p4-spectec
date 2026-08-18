use serde_json::{Value, json};

use crate::lang::el::ast::{
    self, ArgKind, BinOp, CmpOp, ExpKind, Hole, Iter, NumOp, PathKind, PlainTypKind, UnOp,
};

use super::{
    super::{DecodeError, array, field, integer, object, string, variant},
    xl,
};
use crate::wire::ocaml::{atom::AtomPhraseCodec, source};

fn decode_list<T>(
    value: &Value,
    decode: impl Fn(&Value) -> Result<T, DecodeError>,
) -> Result<Vec<T>, DecodeError> {
    array(value)?.iter().map(decode).collect()
}

fn encode_list<T>(values: &[T], encode: impl Fn(&T) -> Value) -> Value {
    Value::Array(values.iter().map(encode).collect())
}

fn decode_id(value: &Value) -> Result<ast::Id, DecodeError> {
    source::decode_phrase(value, |value| Ok(string(value)?.to_owned()))
}

fn encode_id(id: &ast::Id) -> Value {
    source::encode_phrase(id, |id| json!(id))
}

fn decode_iter(value: &Value) -> Result<Iter, DecodeError> {
    let (tag, fields) = variant(value)?;
    match (tag, fields) {
        ("Opt", []) => Ok(Iter::Opt),
        ("List", []) => Ok(Iter::List),
        ("Opt" | "List", _) => Err(DecodeError::Expected("valid EL iterator arity")),
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

fn encode_iter(iter: Iter) -> Value {
    match iter {
        Iter::Opt => json!(["Opt"]),
        Iter::List => json!(["List"]),
    }
}

fn decode_num_op(value: &Value) -> Result<NumOp, DecodeError> {
    let (tag, fields) = variant(value)?;
    match (tag, fields) {
        ("DecOp", []) => Ok(NumOp::DecOp),
        ("HexOp", []) => Ok(NumOp::HexOp),
        ("DecOp" | "HexOp", _) => Err(DecodeError::Expected("valid EL number operator arity")),
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

fn encode_num_op(op: NumOp) -> Value {
    match op {
        NumOp::DecOp => json!(["DecOp"]),
        NumOp::HexOp => json!(["HexOp"]),
    }
}

fn decode_un_op(value: &Value) -> Result<UnOp, DecodeError> {
    let (tag, fields) = variant(value)?;
    match (tag, fields) {
        ("NotOp", []) => Ok(UnOp::NotOp),
        ("PlusOp", []) => Ok(UnOp::PlusOp),
        ("MinusOp", []) => Ok(UnOp::MinusOp),
        ("NotOp" | "PlusOp" | "MinusOp", _) => {
            Err(DecodeError::Expected("valid EL unary operator arity"))
        }
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

fn encode_un_op(op: UnOp) -> Value {
    match op {
        UnOp::NotOp => json!(["NotOp"]),
        UnOp::PlusOp => json!(["PlusOp"]),
        UnOp::MinusOp => json!(["MinusOp"]),
    }
}

fn decode_bin_op(value: &Value) -> Result<BinOp, DecodeError> {
    let (tag, fields) = variant(value)?;
    match (tag, fields) {
        ("AndOp", []) => Ok(BinOp::AndOp),
        ("OrOp", []) => Ok(BinOp::OrOp),
        ("ImplOp", []) => Ok(BinOp::ImplOp),
        ("EquivOp", []) => Ok(BinOp::EquivOp),
        ("AddOp", []) => Ok(BinOp::AddOp),
        ("SubOp", []) => Ok(BinOp::SubOp),
        ("MulOp", []) => Ok(BinOp::MulOp),
        ("DivOp", []) => Ok(BinOp::DivOp),
        ("ModOp", []) => Ok(BinOp::ModOp),
        ("PowOp", []) => Ok(BinOp::PowOp),
        (
            "AndOp" | "OrOp" | "ImplOp" | "EquivOp" | "AddOp" | "SubOp" | "MulOp" | "DivOp"
            | "ModOp" | "PowOp",
            _,
        ) => Err(DecodeError::Expected("valid EL binary operator arity")),
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

fn encode_bin_op(op: BinOp) -> Value {
    match op {
        BinOp::AndOp => json!(["AndOp"]),
        BinOp::OrOp => json!(["OrOp"]),
        BinOp::ImplOp => json!(["ImplOp"]),
        BinOp::EquivOp => json!(["EquivOp"]),
        BinOp::AddOp => json!(["AddOp"]),
        BinOp::SubOp => json!(["SubOp"]),
        BinOp::MulOp => json!(["MulOp"]),
        BinOp::DivOp => json!(["DivOp"]),
        BinOp::ModOp => json!(["ModOp"]),
        BinOp::PowOp => json!(["PowOp"]),
    }
}

fn decode_cmp_op(value: &Value) -> Result<CmpOp, DecodeError> {
    let (tag, fields) = variant(value)?;
    match (tag, fields) {
        ("EqOp", []) => Ok(CmpOp::EqOp),
        ("NeOp", []) => Ok(CmpOp::NeOp),
        ("LtOp", []) => Ok(CmpOp::LtOp),
        ("GtOp", []) => Ok(CmpOp::GtOp),
        ("LeOp", []) => Ok(CmpOp::LeOp),
        ("GeOp", []) => Ok(CmpOp::GeOp),
        ("EqOp" | "NeOp" | "LtOp" | "GtOp" | "LeOp" | "GeOp", _) => {
            Err(DecodeError::Expected("valid EL comparison operator arity"))
        }
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

fn encode_cmp_op(op: CmpOp) -> Value {
    match op {
        CmpOp::EqOp => json!(["EqOp"]),
        CmpOp::NeOp => json!(["NeOp"]),
        CmpOp::LtOp => json!(["LtOp"]),
        CmpOp::GtOp => json!(["GtOp"]),
        CmpOp::LeOp => json!(["LeOp"]),
        CmpOp::GeOp => json!(["GeOp"]),
    }
}

fn decode_plain_typ(value: &Value) -> Result<ast::PlainTyp, DecodeError> {
    source::decode_phrase(value, decode_plain_typ_kind)
}

fn encode_plain_typ(typ: &ast::PlainTyp) -> Value {
    source::encode_phrase(typ, encode_plain_typ_kind)
}

fn decode_targ(value: &Value) -> Result<ast::Targ, DecodeError> {
    source::decode_phrase(value, decode_plain_typ_kind)
}

fn encode_targ(targ: &ast::Targ) -> Value {
    source::encode_phrase(targ, encode_plain_typ_kind)
}

fn decode_plain_typ_kind(value: &Value) -> Result<PlainTypKind, DecodeError> {
    let (tag, fields) = variant(value)?;
    match (tag, fields) {
        ("BoolT", []) => Ok(PlainTypKind::BoolT),
        ("NumT", [typ]) => Ok(PlainTypKind::NumT(xl::decode_num_typ(typ)?)),
        ("TextT", []) => Ok(PlainTypKind::TextT),
        ("VarT", [id, targs]) => Ok(PlainTypKind::VarT(
            decode_id(id)?,
            decode_list(targs, decode_targ)?,
        )),
        ("ParenT", [typ]) => Ok(PlainTypKind::ParenT(Box::new(decode_plain_typ(typ)?))),
        ("TupleT", [types]) => Ok(PlainTypKind::TupleT(decode_list(types, decode_plain_typ)?)),
        ("IterT", [typ, iter]) => Ok(PlainTypKind::IterT(
            Box::new(decode_plain_typ(typ)?),
            decode_iter(iter)?,
        )),
        ("BoolT" | "NumT" | "TextT" | "VarT" | "ParenT" | "TupleT" | "IterT", _) => {
            Err(DecodeError::Expected("valid EL plain type arity"))
        }
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

fn encode_plain_typ_kind(typ: &PlainTypKind) -> Value {
    match typ {
        PlainTypKind::BoolT => json!(["BoolT"]),
        PlainTypKind::NumT(typ) => json!(["NumT", xl::encode_num_typ(*typ)]),
        PlainTypKind::TextT => json!(["TextT"]),
        PlainTypKind::VarT(id, targs) => {
            json!(["VarT", encode_id(id), encode_list(targs, encode_targ)])
        }
        PlainTypKind::ParenT(typ) => json!(["ParenT", encode_plain_typ(typ)]),
        PlainTypKind::TupleT(types) => {
            json!(["TupleT", encode_list(types, encode_plain_typ)])
        }
        PlainTypKind::IterT(typ, iter) => {
            json!(["IterT", encode_plain_typ(typ), encode_iter(*iter)])
        }
    }
}

fn decode_path(value: &Value) -> Result<ast::Path, DecodeError> {
    source::decode_phrase(value, decode_path_kind)
}

fn encode_path(path: &ast::Path) -> Value {
    source::encode_phrase(path, encode_path_kind)
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
            Err(DecodeError::Expected("valid EL path arity"))
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
            ("DefA", [id]) => Ok(ArgKind::DefA(decode_id(id)?)),
            ("ExpA" | "DefA", _) => Err(DecodeError::Expected("valid EL argument arity")),
            (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
        }
    })
}

fn encode_arg(arg: &ast::Arg) -> Value {
    source::encode_phrase(arg, |arg| match arg {
        ArgKind::ExpA(exp) => json!(["ExpA", encode_exp(exp)]),
        ArgKind::DefA(id) => json!(["DefA", encode_id(id)]),
    })
}

fn decode_hole(value: &Value) -> Result<Hole, DecodeError> {
    let (tag, fields) = variant(value)?;
    match (tag, fields) {
        ("Num", [num]) => Ok(Hole::Num(integer(num)?)),
        ("Next", []) => Ok(Hole::Next),
        ("Rest", []) => Ok(Hole::Rest),
        ("None", []) => Ok(Hole::None),
        ("Num" | "Next" | "Rest" | "None", _) => Err(DecodeError::Expected("valid EL hole arity")),
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

fn encode_hole(hole: &Hole) -> Value {
    match hole {
        Hole::Num(num) => json!(["Num", num]),
        Hole::Next => json!(["Next"]),
        Hole::Rest => json!(["Rest"]),
        Hole::None => json!(["None"]),
    }
}

fn decode_exp(value: &Value) -> Result<ast::Exp, DecodeError> {
    source::decode_phrase(value, decode_exp_kind)
}

fn encode_exp(exp: &ast::Exp) -> Value {
    source::encode_phrase(exp, encode_exp_kind)
}

fn decode_exp_kind(value: &Value) -> Result<ExpKind, DecodeError> {
    let (tag, fields) = variant(value)?;
    match (tag, fields) {
        ("BoolE", [value]) => Ok(ExpKind::BoolE(super::super::boolean(value)?)),
        ("NumE", [op, num]) => Ok(ExpKind::NumE(decode_num_op(op)?, xl::decode_num(num)?)),
        ("TextE", [text]) => Ok(ExpKind::TextE(string(text)?.to_owned())),
        ("VarE", [id]) => Ok(ExpKind::VarE(decode_id(id)?)),
        ("UnE", [op, exp]) => Ok(ExpKind::UnE(decode_un_op(op)?, Box::new(decode_exp(exp)?))),
        ("BinE", [left, op, right]) => Ok(ExpKind::BinE(
            Box::new(decode_exp(left)?),
            decode_bin_op(op)?,
            Box::new(decode_exp(right)?),
        )),
        ("CmpE", [left, op, right]) => Ok(ExpKind::CmpE(
            Box::new(decode_exp(left)?),
            decode_cmp_op(op)?,
            Box::new(decode_exp(right)?),
        )),
        ("ArithE", [exp]) => Ok(ExpKind::ArithE(Box::new(decode_exp(exp)?))),
        ("EpsE", []) => Ok(ExpKind::EpsE),
        ("ListE", [exps]) => Ok(ExpKind::ListE(decode_list(exps, decode_exp)?)),
        ("ConsE", [head, tail]) => Ok(ExpKind::ConsE(
            Box::new(decode_exp(head)?),
            Box::new(decode_exp(tail)?),
        )),
        ("CatE", [left, right]) => Ok(ExpKind::CatE(
            Box::new(decode_exp(left)?),
            Box::new(decode_exp(right)?),
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
        ("LenE", [exp]) => Ok(ExpKind::LenE(Box::new(decode_exp(exp)?))),
        ("MemE", [left, right]) => Ok(ExpKind::MemE(
            Box::new(decode_exp(left)?),
            Box::new(decode_exp(right)?),
        )),
        ("StrE", [fields]) => Ok(ExpKind::StrE(decode_list(fields, |field| {
            match array(field)? {
                [atom, exp] => Ok((AtomPhraseCodec::decode(atom)?, decode_exp(exp)?)),
                _ => Err(DecodeError::Expected("EL structure field pair")),
            }
        })?)),
        ("DotE", [exp, atom]) => Ok(ExpKind::DotE(
            Box::new(decode_exp(exp)?),
            AtomPhraseCodec::decode(atom)?,
        )),
        ("UpdE", [base, path, value]) => Ok(ExpKind::UpdE(
            Box::new(decode_exp(base)?),
            decode_path(path)?,
            Box::new(decode_exp(value)?),
        )),
        ("ParenE", [exp]) => Ok(ExpKind::ParenE(Box::new(decode_exp(exp)?))),
        ("TupleE", [exps]) => Ok(ExpKind::TupleE(decode_list(exps, decode_exp)?)),
        ("CallE", [id, targs, args]) => Ok(ExpKind::CallE(
            decode_id(id)?,
            decode_list(targs, decode_targ)?,
            decode_list(args, decode_arg)?,
        )),
        ("IterE", [exp, iter]) => Ok(ExpKind::IterE(
            Box::new(decode_exp(exp)?),
            decode_iter(iter)?,
        )),
        ("SubE", [exp, typ]) => Ok(ExpKind::SubE(
            Box::new(decode_exp(exp)?),
            decode_plain_typ(typ)?,
        )),
        ("AtomE", [atom]) => Ok(ExpKind::AtomE(AtomPhraseCodec::decode(atom)?)),
        ("SeqE", [exps]) => Ok(ExpKind::SeqE(decode_list(exps, decode_exp)?)),
        ("InfixE", [left, atom, right]) => Ok(ExpKind::InfixE(
            Box::new(decode_exp(left)?),
            AtomPhraseCodec::decode(atom)?,
            Box::new(decode_exp(right)?),
        )),
        ("BrackE", [left, exp, right]) => Ok(ExpKind::BrackE(
            AtomPhraseCodec::decode(left)?,
            Box::new(decode_exp(exp)?),
            AtomPhraseCodec::decode(right)?,
        )),
        ("HoleE", [hole]) => Ok(ExpKind::HoleE(decode_hole(hole)?)),
        ("FuseE", [left, right]) => Ok(ExpKind::FuseE(
            Box::new(decode_exp(left)?),
            Box::new(decode_exp(right)?),
        )),
        ("UnparenE", [exp]) => Ok(ExpKind::UnparenE(Box::new(decode_exp(exp)?))),
        ("LatexE", [latex]) => Ok(ExpKind::LatexE(string(latex)?.to_owned())),
        (
            "BoolE" | "NumE" | "TextE" | "VarE" | "UnE" | "BinE" | "CmpE" | "ArithE" | "EpsE"
            | "ListE" | "ConsE" | "CatE" | "IdxE" | "SliceE" | "LenE" | "MemE" | "StrE" | "DotE"
            | "UpdE" | "ParenE" | "TupleE" | "CallE" | "IterE" | "SubE" | "AtomE" | "SeqE"
            | "InfixE" | "BrackE" | "HoleE" | "FuseE" | "UnparenE" | "LatexE",
            _,
        ) => Err(DecodeError::Expected("valid EL expression arity")),
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

fn encode_exp_kind(exp: &ExpKind) -> Value {
    match exp {
        ExpKind::BoolE(value) => json!(["BoolE", value]),
        ExpKind::NumE(op, num) => json!(["NumE", encode_num_op(*op), xl::encode_num(num)]),
        ExpKind::TextE(text) => json!(["TextE", text]),
        ExpKind::VarE(id) => json!(["VarE", encode_id(id)]),
        ExpKind::UnE(op, exp) => json!(["UnE", encode_un_op(*op), encode_exp(exp)]),
        ExpKind::BinE(left, op, right) => {
            json!([
                "BinE",
                encode_exp(left),
                encode_bin_op(*op),
                encode_exp(right)
            ])
        }
        ExpKind::CmpE(left, op, right) => {
            json!([
                "CmpE",
                encode_exp(left),
                encode_cmp_op(*op),
                encode_exp(right)
            ])
        }
        ExpKind::ArithE(exp) => json!(["ArithE", encode_exp(exp)]),
        ExpKind::EpsE => json!(["EpsE"]),
        ExpKind::ListE(exps) => json!(["ListE", encode_list(exps, encode_exp)]),
        ExpKind::ConsE(head, tail) => json!(["ConsE", encode_exp(head), encode_exp(tail)]),
        ExpKind::CatE(left, right) => json!(["CatE", encode_exp(left), encode_exp(right)]),
        ExpKind::IdxE(base, index) => json!(["IdxE", encode_exp(base), encode_exp(index)]),
        ExpKind::SliceE(base, left, right) => json!([
            "SliceE",
            encode_exp(base),
            encode_exp(left),
            encode_exp(right)
        ]),
        ExpKind::LenE(exp) => json!(["LenE", encode_exp(exp)]),
        ExpKind::MemE(left, right) => json!(["MemE", encode_exp(left), encode_exp(right)]),
        ExpKind::StrE(fields) => json!([
            "StrE",
            fields
                .iter()
                .map(|(atom, exp)| json!([AtomPhraseCodec::encode(atom), encode_exp(exp)]))
                .collect::<Vec<_>>()
        ]),
        ExpKind::DotE(exp, atom) => {
            json!(["DotE", encode_exp(exp), AtomPhraseCodec::encode(atom)])
        }
        ExpKind::UpdE(base, path, value) => {
            json!([
                "UpdE",
                encode_exp(base),
                encode_path(path),
                encode_exp(value)
            ])
        }
        ExpKind::ParenE(exp) => json!(["ParenE", encode_exp(exp)]),
        ExpKind::TupleE(exps) => json!(["TupleE", encode_list(exps, encode_exp)]),
        ExpKind::CallE(id, targs, args) => json!([
            "CallE",
            encode_id(id),
            encode_list(targs, encode_targ),
            encode_list(args, encode_arg)
        ]),
        ExpKind::IterE(exp, iter) => json!(["IterE", encode_exp(exp), encode_iter(*iter)]),
        ExpKind::SubE(exp, typ) => json!(["SubE", encode_exp(exp), encode_plain_typ(typ)]),
        ExpKind::AtomE(atom) => json!(["AtomE", AtomPhraseCodec::encode(atom)]),
        ExpKind::SeqE(exps) => json!(["SeqE", encode_list(exps, encode_exp)]),
        ExpKind::InfixE(left, atom, right) => json!([
            "InfixE",
            encode_exp(left),
            AtomPhraseCodec::encode(atom),
            encode_exp(right)
        ]),
        ExpKind::BrackE(left, exp, right) => json!([
            "BrackE",
            AtomPhraseCodec::encode(left),
            encode_exp(exp),
            AtomPhraseCodec::encode(right)
        ]),
        ExpKind::HoleE(hole) => json!(["HoleE", encode_hole(hole)]),
        ExpKind::FuseE(left, right) => json!(["FuseE", encode_exp(left), encode_exp(right)]),
        ExpKind::UnparenE(exp) => json!(["UnparenE", encode_exp(exp)]),
        ExpKind::LatexE(latex) => json!(["LatexE", latex]),
    }
}

pub(super) fn decode_hint(value: &Value) -> Result<ast::Hint, DecodeError> {
    let object = object(value)?;
    Ok(ast::Hint {
        hintid: decode_id(field(object, "hintid")?)?,
        hintexp: decode_exp(field(object, "hintexp")?)?,
    })
}

pub(super) fn encode_hint(hint: &ast::Hint) -> Value {
    json!({
        "hintid": encode_id(&hint.hintid),
        "hintexp": encode_exp(&hint.hintexp),
    })
}
