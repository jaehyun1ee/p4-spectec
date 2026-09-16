use serde_json::json;

use crate::util::json::json;

use crate::lang::el::ast::{
    self, ArgKind, BinOp, CmpOp, ExpKind, Hole, Iter, NumOp, PathKind, PlainTypKind, UnOp,
};

use super::{
    super::{DecodeError, array, field, object, string, unsigned, variant},
    xl,
};
use crate::wire::ocaml::{atom::AtomPhraseCodec, source};

fn decode_list<T>(
    json: &json,
    decode: impl Fn(&json) -> Result<T, DecodeError>,
) -> Result<Vec<T>, DecodeError> {
    array(json)?.iter().map(decode).collect()
}

fn encode_list<T>(values: &[T], encode: impl Fn(&T) -> json) -> json {
    json::Array(values.iter().map(encode).collect())
}

fn decode_id(json: &json) -> Result<ast::Id, DecodeError> {
    source::decode_phrase(json, |json| Ok(string(json)?.to_owned()))
}

fn encode_id(id: &ast::Id) -> json {
    source::encode_phrase(id, |id| json!(id))
}

fn decode_iter(json: &json) -> Result<Iter, DecodeError> {
    let (tag, fields) = variant(json)?;
    match (tag, fields) {
        ("Opt", []) => Ok(Iter::Opt),
        ("List", []) => Ok(Iter::List),
        ("Opt" | "List", _) => Err(DecodeError::Expected("valid EL iterator arity")),
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

fn encode_iter(iter: Iter) -> json {
    match iter {
        Iter::Opt => json!(["Opt"]),
        Iter::List => json!(["List"]),
    }
}

fn decode_num_op(json: &json) -> Result<NumOp, DecodeError> {
    let (tag, fields) = variant(json)?;
    match (tag, fields) {
        ("DecOp", []) => Ok(NumOp::Dec),
        ("HexOp", []) => Ok(NumOp::Hex),
        ("DecOp" | "HexOp", _) => Err(DecodeError::Expected("valid EL number operator arity")),
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

fn encode_num_op(op: NumOp) -> json {
    match op {
        NumOp::Dec => json!(["DecOp"]),
        NumOp::Hex => json!(["HexOp"]),
    }
}

fn decode_un_op(json: &json) -> Result<UnOp, DecodeError> {
    let (tag, fields) = variant(json)?;
    match (tag, fields) {
        ("NotOp", []) => Ok(UnOp::Bool(crate::lang::xl::bool::UnOp::Not)),
        ("PlusOp", []) => Ok(UnOp::Num(crate::lang::xl::num::UnOp::Plus)),
        ("MinusOp", []) => Ok(UnOp::Num(crate::lang::xl::num::UnOp::Minus)),
        ("NotOp" | "PlusOp" | "MinusOp", _) => {
            Err(DecodeError::Expected("valid EL unary operator arity"))
        }
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

fn encode_un_op(op: UnOp) -> json {
    match op {
        UnOp::Bool(crate::lang::xl::bool::UnOp::Not) => json!(["NotOp"]),
        UnOp::Num(crate::lang::xl::num::UnOp::Plus) => json!(["PlusOp"]),
        UnOp::Num(crate::lang::xl::num::UnOp::Minus) => json!(["MinusOp"]),
    }
}

fn decode_bin_op(json: &json) -> Result<BinOp, DecodeError> {
    let (tag, fields) = variant(json)?;
    match (tag, fields) {
        ("AndOp", []) => Ok(BinOp::Bool(crate::lang::xl::bool::BinOp::And)),
        ("OrOp", []) => Ok(BinOp::Bool(crate::lang::xl::bool::BinOp::Or)),
        ("ImplOp", []) => Ok(BinOp::Bool(crate::lang::xl::bool::BinOp::Impl)),
        ("EquivOp", []) => Ok(BinOp::Bool(crate::lang::xl::bool::BinOp::Equiv)),
        ("AddOp", []) => Ok(BinOp::Num(crate::lang::xl::num::BinOp::Add)),
        ("SubOp", []) => Ok(BinOp::Num(crate::lang::xl::num::BinOp::Sub)),
        ("MulOp", []) => Ok(BinOp::Num(crate::lang::xl::num::BinOp::Mul)),
        ("DivOp", []) => Ok(BinOp::Num(crate::lang::xl::num::BinOp::Div)),
        ("ModOp", []) => Ok(BinOp::Num(crate::lang::xl::num::BinOp::Mod)),
        ("PowOp", []) => Ok(BinOp::Num(crate::lang::xl::num::BinOp::Pow)),
        (
            "AndOp" | "OrOp" | "ImplOp" | "EquivOp" | "AddOp" | "SubOp" | "MulOp" | "DivOp"
            | "ModOp" | "PowOp",
            _,
        ) => Err(DecodeError::Expected("valid EL binary operator arity")),
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

fn encode_bin_op(op: BinOp) -> json {
    match op {
        BinOp::Bool(crate::lang::xl::bool::BinOp::And) => json!(["AndOp"]),
        BinOp::Bool(crate::lang::xl::bool::BinOp::Or) => json!(["OrOp"]),
        BinOp::Bool(crate::lang::xl::bool::BinOp::Impl) => json!(["ImplOp"]),
        BinOp::Bool(crate::lang::xl::bool::BinOp::Equiv) => json!(["EquivOp"]),
        BinOp::Num(crate::lang::xl::num::BinOp::Add) => json!(["AddOp"]),
        BinOp::Num(crate::lang::xl::num::BinOp::Sub) => json!(["SubOp"]),
        BinOp::Num(crate::lang::xl::num::BinOp::Mul) => json!(["MulOp"]),
        BinOp::Num(crate::lang::xl::num::BinOp::Div) => json!(["DivOp"]),
        BinOp::Num(crate::lang::xl::num::BinOp::Mod) => json!(["ModOp"]),
        BinOp::Num(crate::lang::xl::num::BinOp::Pow) => json!(["PowOp"]),
    }
}

fn decode_cmp_op(json: &json) -> Result<CmpOp, DecodeError> {
    let (tag, fields) = variant(json)?;
    match (tag, fields) {
        ("EqOp", []) => Ok(CmpOp::Bool(crate::lang::xl::bool::CmpOp::Eq)),
        ("NeOp", []) => Ok(CmpOp::Bool(crate::lang::xl::bool::CmpOp::Ne)),
        ("LtOp", []) => Ok(CmpOp::Num(crate::lang::xl::num::CmpOp::Lt)),
        ("GtOp", []) => Ok(CmpOp::Num(crate::lang::xl::num::CmpOp::Gt)),
        ("LeOp", []) => Ok(CmpOp::Num(crate::lang::xl::num::CmpOp::Le)),
        ("GeOp", []) => Ok(CmpOp::Num(crate::lang::xl::num::CmpOp::Ge)),
        ("EqOp" | "NeOp" | "LtOp" | "GtOp" | "LeOp" | "GeOp", _) => {
            Err(DecodeError::Expected("valid EL comparison operator arity"))
        }
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

fn encode_cmp_op(op: CmpOp) -> json {
    match op {
        CmpOp::Bool(crate::lang::xl::bool::CmpOp::Eq) => json!(["EqOp"]),
        CmpOp::Bool(crate::lang::xl::bool::CmpOp::Ne) => json!(["NeOp"]),
        CmpOp::Num(crate::lang::xl::num::CmpOp::Lt) => json!(["LtOp"]),
        CmpOp::Num(crate::lang::xl::num::CmpOp::Gt) => json!(["GtOp"]),
        CmpOp::Num(crate::lang::xl::num::CmpOp::Le) => json!(["LeOp"]),
        CmpOp::Num(crate::lang::xl::num::CmpOp::Ge) => json!(["GeOp"]),
    }
}

fn decode_plain_typ(json: &json) -> Result<ast::PlainTyp, DecodeError> {
    source::decode_phrase(json, decode_plain_typ_kind)
}

fn encode_plain_typ(typ: &ast::PlainTyp) -> json {
    source::encode_phrase(typ, encode_plain_typ_kind)
}

fn decode_targ(json: &json) -> Result<ast::Targ, DecodeError> {
    source::decode_phrase(json, decode_plain_typ_kind)
}

fn encode_targ(targ: &ast::Targ) -> json {
    source::encode_phrase(targ, encode_plain_typ_kind)
}

fn decode_plain_typ_kind(json: &json) -> Result<PlainTypKind, DecodeError> {
    let (tag, fields) = variant(json)?;
    match (tag, fields) {
        ("BoolT", []) => Ok(PlainTypKind::Bool),
        ("NumT", [typ]) => Ok(PlainTypKind::Num(xl::decode_num_typ(typ)?)),
        ("TextT", []) => Ok(PlainTypKind::Text),
        ("VarT", [id, targs]) => {
            Ok(PlainTypKind::Var(decode_id(id)?, decode_list(targs, decode_targ)?))
        }
        ("ParenT", [typ]) => Ok(PlainTypKind::Paren(Box::new(decode_plain_typ(typ)?))),
        ("TupleT", [types]) => Ok(PlainTypKind::Tuple(decode_list(types, decode_plain_typ)?)),
        ("IterT", [typ, iter]) => {
            Ok(PlainTypKind::Iter(Box::new(decode_plain_typ(typ)?), decode_iter(iter)?))
        }
        ("BoolT" | "NumT" | "TextT" | "VarT" | "ParenT" | "TupleT" | "IterT", _) => {
            Err(DecodeError::Expected("valid EL plain type arity"))
        }
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

fn encode_plain_typ_kind(typ: &PlainTypKind) -> json {
    match typ {
        PlainTypKind::Bool => json!(["BoolT"]),
        PlainTypKind::Num(typ) => json!(["NumT", xl::encode_num_typ(*typ)]),
        PlainTypKind::Text => json!(["TextT"]),
        PlainTypKind::Var(id, targs) => {
            json!(["VarT", encode_id(id), encode_list(targs, encode_targ)])
        }
        PlainTypKind::Paren(typ) => json!(["ParenT", encode_plain_typ(typ)]),
        PlainTypKind::Tuple(types) => {
            json!(["TupleT", encode_list(types, encode_plain_typ)])
        }
        PlainTypKind::Iter(typ, iter) => {
            json!(["IterT", encode_plain_typ(typ), encode_iter(*iter)])
        }
    }
}

fn decode_path(json: &json) -> Result<ast::Path, DecodeError> {
    source::decode_phrase(json, decode_path_kind)
}

fn encode_path(path: &ast::Path) -> json {
    source::encode_phrase(path, encode_path_kind)
}

fn decode_path_kind(json: &json) -> Result<PathKind, DecodeError> {
    let (tag, fields) = variant(json)?;
    match (tag, fields) {
        ("RootP", []) => Ok(PathKind::Root),
        ("IdxP", [path, exp_idx]) => {
            Ok(PathKind::Idx(Box::new(decode_path(path)?), Box::new(decode_exp(exp_idx)?)))
        }
        ("SliceP", [path, exp_idx, exp_len]) => Ok(PathKind::Slice(
            Box::new(decode_path(path)?),
            Box::new(decode_exp(exp_idx)?),
            Box::new(decode_exp(exp_len)?),
        )),
        ("DotP", [path, atom]) => {
            Ok(PathKind::Dot(Box::new(decode_path(path)?), AtomPhraseCodec::decode(atom)?))
        }
        ("RootP" | "IdxP" | "SliceP" | "DotP", _) => {
            Err(DecodeError::Expected("valid EL path arity"))
        }
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

fn encode_path_kind(path: &PathKind) -> json {
    match path {
        PathKind::Root => json!(["RootP"]),
        PathKind::Idx(path, exp_idx) => json!(["IdxP", encode_path(path), encode_exp(exp_idx)]),
        PathKind::Slice(path, exp_idx, exp_len) => {
            json!(["SliceP", encode_path(path), encode_exp(exp_idx), encode_exp(exp_len)])
        }
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
            ("DefA", [id]) => Ok(ArgKind::Def(decode_id(id)?)),
            ("ExpA" | "DefA", _) => Err(DecodeError::Expected("valid EL argument arity")),
            (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
        }
    })
}

fn encode_arg(arg: &ast::Arg) -> json {
    source::encode_phrase(arg, |arg| match arg {
        ArgKind::Exp(exp) => json!(["ExpA", encode_exp(exp)]),
        ArgKind::Def(id) => json!(["DefA", encode_id(id)]),
    })
}

fn decode_hole(json: &json) -> Result<Hole, DecodeError> {
    let (tag, fields) = variant(json)?;
    match (tag, fields) {
        ("Num", [num]) => Ok(Hole::Num(unsigned(num)?)),
        ("Next", []) => Ok(Hole::Next),
        ("Rest", []) => Ok(Hole::Rest),
        ("None", []) => Ok(Hole::None),
        ("Num" | "Next" | "Rest" | "None", _) => Err(DecodeError::Expected("valid EL hole arity")),
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

fn encode_hole(hole: &Hole) -> json {
    match hole {
        Hole::Num(num) => json!(["Num", num]),
        Hole::Next => json!(["Next"]),
        Hole::Rest => json!(["Rest"]),
        Hole::None => json!(["None"]),
    }
}

pub(super) fn decode_exp(json: &json) -> Result<ast::Exp, DecodeError> {
    source::decode_phrase(json, decode_exp_kind)
}

pub(super) fn encode_exp(exp: &ast::Exp) -> json {
    source::encode_phrase(exp, encode_exp_kind)
}

fn decode_exp_kind(json: &json) -> Result<ExpKind, DecodeError> {
    let (tag, fields) = variant(json)?;
    match (tag, fields) {
        ("BoolE", [json]) => Ok(ExpKind::Bool(super::super::boolean(json)?)),
        ("NumE", [op, num]) => Ok(ExpKind::Num(decode_num_op(op)?, xl::decode_num(num)?)),
        ("TextE", [text]) => Ok(ExpKind::Text(string(text)?.to_owned())),
        ("VarE", [id]) => Ok(ExpKind::Var(decode_id(id)?)),
        ("UnE", [op, exp]) => Ok(ExpKind::Un(decode_un_op(op)?, Box::new(decode_exp(exp)?))),
        ("BinE", [exp_l, op, exp_r]) => Ok(ExpKind::Bin(
            Box::new(decode_exp(exp_l)?),
            decode_bin_op(op)?,
            Box::new(decode_exp(exp_r)?),
        )),
        ("CmpE", [exp_l, op, exp_r]) => Ok(ExpKind::Cmp(
            Box::new(decode_exp(exp_l)?),
            decode_cmp_op(op)?,
            Box::new(decode_exp(exp_r)?),
        )),
        ("ArithE", [exp]) => Ok(ExpKind::Arith(Box::new(decode_exp(exp)?))),
        ("EpsE", []) => Ok(ExpKind::Eps),
        ("ListE", [exps]) => Ok(ExpKind::List(decode_list(exps, decode_exp)?)),
        ("ConsE", [exp_head, exp_tail]) => {
            Ok(ExpKind::Cons(Box::new(decode_exp(exp_head)?), Box::new(decode_exp(exp_tail)?)))
        }
        ("CatE", [exp_l, exp_r]) => {
            Ok(ExpKind::Cat(Box::new(decode_exp(exp_l)?), Box::new(decode_exp(exp_r)?)))
        }
        ("IdxE", [exp_base, exp_idx]) => {
            Ok(ExpKind::Idx(Box::new(decode_exp(exp_base)?), Box::new(decode_exp(exp_idx)?)))
        }
        ("SliceE", [exp_base, exp_idx, exp_len]) => Ok(ExpKind::Slice(
            Box::new(decode_exp(exp_base)?),
            Box::new(decode_exp(exp_idx)?),
            Box::new(decode_exp(exp_len)?),
        )),
        ("LenE", [exp]) => Ok(ExpKind::Len(Box::new(decode_exp(exp)?))),
        ("MemE", [exp_l, exp_r]) => {
            Ok(ExpKind::Mem(Box::new(decode_exp(exp_l)?), Box::new(decode_exp(exp_r)?)))
        }
        ("StrE", [fields]) => Ok(ExpKind::Str(decode_list(fields, |field| match array(field)? {
            [atom, exp] => Ok((AtomPhraseCodec::decode(atom)?, decode_exp(exp)?)),
            _ => Err(DecodeError::Expected("EL structure field pair")),
        })?)),
        ("DotE", [exp, atom]) => {
            Ok(ExpKind::Dot(Box::new(decode_exp(exp)?), AtomPhraseCodec::decode(atom)?))
        }
        ("UpdE", [exp_base, path, exp_field]) => Ok(ExpKind::Upd(
            Box::new(decode_exp(exp_base)?),
            decode_path(path)?,
            Box::new(decode_exp(exp_field)?),
        )),
        ("ParenE", [exp]) => Ok(ExpKind::Paren(Box::new(decode_exp(exp)?))),
        ("TupleE", [exps]) => Ok(ExpKind::Tuple(decode_list(exps, decode_exp)?)),
        ("CallE", [id, targs, args]) => Ok(ExpKind::Call(
            decode_id(id)?,
            decode_list(targs, decode_targ)?,
            decode_list(args, decode_arg)?,
        )),
        ("IterE", [exp, iter]) => Ok(ExpKind::Iter(Box::new(decode_exp(exp)?), decode_iter(iter)?)),
        ("SubE", [exp, typ]) => {
            Ok(ExpKind::Sub(Box::new(decode_exp(exp)?), decode_plain_typ(typ)?))
        }
        ("AtomE", [atom]) => Ok(ExpKind::Atom(AtomPhraseCodec::decode(atom)?)),
        ("SeqE", [exps]) => Ok(ExpKind::Seq(decode_list(exps, decode_exp)?)),
        ("InfixE", [exp_l, atom, exp_r]) => Ok(ExpKind::Infix(
            Box::new(decode_exp(exp_l)?),
            AtomPhraseCodec::decode(atom)?,
            Box::new(decode_exp(exp_r)?),
        )),
        ("BrackE", [atom_l, exp_inner, atom_r]) => Ok(ExpKind::Brack(
            AtomPhraseCodec::decode(atom_l)?,
            Box::new(decode_exp(exp_inner)?),
            AtomPhraseCodec::decode(atom_r)?,
        )),
        ("HoleE", [hole]) => Ok(ExpKind::Hole(decode_hole(hole)?)),
        ("FuseE", [exp_l, exp_r]) => {
            Ok(ExpKind::Fuse(Box::new(decode_exp(exp_l)?), Box::new(decode_exp(exp_r)?)))
        }
        ("UnparenE", [exp]) => Ok(ExpKind::Unparen(Box::new(decode_exp(exp)?))),
        ("LatexE", [latex]) => Ok(ExpKind::Latex(string(latex)?.to_owned())),
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

fn encode_exp_kind(exp: &ExpKind) -> json {
    match exp {
        ExpKind::Bool(value) => json!(["BoolE", value]),
        ExpKind::Num(op, num) => json!(["NumE", encode_num_op(*op), xl::encode_num(num)]),
        ExpKind::Text(text) => json!(["TextE", text]),
        ExpKind::Var(id) => json!(["VarE", encode_id(id)]),
        ExpKind::Un(op, exp) => json!(["UnE", encode_un_op(*op), encode_exp(exp)]),
        ExpKind::Bin(exp_l, op, exp_r) => {
            json!(["BinE", encode_exp(exp_l), encode_bin_op(*op), encode_exp(exp_r)])
        }
        ExpKind::Cmp(exp_l, op, exp_r) => {
            json!(["CmpE", encode_exp(exp_l), encode_cmp_op(*op), encode_exp(exp_r)])
        }
        ExpKind::Arith(exp) => json!(["ArithE", encode_exp(exp)]),
        ExpKind::Eps => json!(["EpsE"]),
        ExpKind::List(exps) => json!(["ListE", encode_list(exps, encode_exp)]),
        ExpKind::Cons(exp_head, exp_tail) => {
            json!(["ConsE", encode_exp(exp_head), encode_exp(exp_tail)])
        }
        ExpKind::Cat(exp_l, exp_r) => json!(["CatE", encode_exp(exp_l), encode_exp(exp_r)]),
        ExpKind::Idx(exp_base, exp_idx) => {
            json!(["IdxE", encode_exp(exp_base), encode_exp(exp_idx)])
        }
        ExpKind::Slice(exp_base, exp_idx, exp_len) => {
            json!(["SliceE", encode_exp(exp_base), encode_exp(exp_idx), encode_exp(exp_len)])
        }
        ExpKind::Len(exp) => json!(["LenE", encode_exp(exp)]),
        ExpKind::Mem(exp_l, exp_r) => json!(["MemE", encode_exp(exp_l), encode_exp(exp_r)]),
        ExpKind::Str(fields) => json!([
            "StrE",
            fields
                .iter()
                .map(|(atom, exp)| json!([AtomPhraseCodec::encode(atom), encode_exp(exp)]))
                .collect::<Vec<_>>()
        ]),
        ExpKind::Dot(exp, atom) => {
            json!(["DotE", encode_exp(exp), AtomPhraseCodec::encode(atom)])
        }
        ExpKind::Upd(exp_base, path, exp_field) => {
            json!(["UpdE", encode_exp(exp_base), encode_path(path), encode_exp(exp_field)])
        }
        ExpKind::Paren(exp) => json!(["ParenE", encode_exp(exp)]),
        ExpKind::Tuple(exps) => json!(["TupleE", encode_list(exps, encode_exp)]),
        ExpKind::Call(id, targs, args) => json!([
            "CallE",
            encode_id(id),
            encode_list(targs, encode_targ),
            encode_list(args, encode_arg)
        ]),
        ExpKind::Iter(exp, iter) => json!(["IterE", encode_exp(exp), encode_iter(*iter)]),
        ExpKind::Sub(exp, typ) => json!(["SubE", encode_exp(exp), encode_plain_typ(typ)]),
        ExpKind::Atom(atom) => json!(["AtomE", AtomPhraseCodec::encode(atom)]),
        ExpKind::Seq(exps) => json!(["SeqE", encode_list(exps, encode_exp)]),
        ExpKind::Infix(exp_l, atom, exp_r) => {
            json!(["InfixE", encode_exp(exp_l), AtomPhraseCodec::encode(atom), encode_exp(exp_r)])
        }
        ExpKind::Brack(atom_l, exp_inner, atom_r) => json!([
            "BrackE",
            AtomPhraseCodec::encode(atom_l),
            encode_exp(exp_inner),
            AtomPhraseCodec::encode(atom_r)
        ]),
        ExpKind::Hole(hole) => json!(["HoleE", encode_hole(hole)]),
        ExpKind::Fuse(exp_l, exp_r) => json!(["FuseE", encode_exp(exp_l), encode_exp(exp_r)]),
        ExpKind::Unparen(exp) => json!(["UnparenE", encode_exp(exp)]),
        ExpKind::Latex(latex) => json!(["LatexE", latex]),
    }
}

pub(super) fn decode_hint(json: &json) -> Result<ast::Hint, DecodeError> {
    let object = object(json)?;
    Ok((decode_id(field(object, "hintid")?)?, decode_exp(field(object, "hintexp")?)?))
}

pub(super) fn encode_hint(hint: &ast::Hint) -> json {
    json!({
        "hintid": encode_id(&hint.0),
        "hintexp": encode_exp(&hint.1),
    })
}
