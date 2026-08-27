use serde_json::{Value, json};

use crate::lang::el::ast::*;
use crate::lang::el::ast::{
    self, ArgKind, BinOp, CmpOp, DefKind, DefTypKind, ExpKind, Hole, Iter, NotTypKind, NumOp,
    ParamKind, PathKind, PlainTypKind, PremKind, UnOp,
};

use super::{
    super::{
        DecodeError, EncodeError, array, field, integer, object, on_codec_stack, string, variant,
    },
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

pub struct SpecCodec;

impl SpecCodec {
    pub fn decode(value: &Value) -> Result<ast::Spec, DecodeError> {
        on_codec_stack(|| decode_list(value, decode_def))
    }

    pub fn encode(spec: &ast::Spec) -> Result<Value, EncodeError> {
        on_codec_stack(|| Ok(encode_list(spec, encode_def)))
    }
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
        ("DecOp", []) => Ok(NumOp::Dec),
        ("HexOp", []) => Ok(NumOp::Hex),
        ("DecOp" | "HexOp", _) => Err(DecodeError::Expected("valid EL number operator arity")),
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

fn encode_num_op(op: NumOp) -> Value {
    match op {
        NumOp::Dec => json!(["DecOp"]),
        NumOp::Hex => json!(["HexOp"]),
    }
}

fn decode_un_op(value: &Value) -> Result<UnOp, DecodeError> {
    let (tag, fields) = variant(value)?;
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

fn encode_un_op(op: UnOp) -> Value {
    match op {
        UnOp::Bool(crate::lang::xl::bool::UnOp::Not) => json!(["NotOp"]),
        UnOp::Num(crate::lang::xl::num::UnOp::Plus) => json!(["PlusOp"]),
        UnOp::Num(crate::lang::xl::num::UnOp::Minus) => json!(["MinusOp"]),
    }
}

fn decode_bin_op(value: &Value) -> Result<BinOp, DecodeError> {
    let (tag, fields) = variant(value)?;
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

fn encode_bin_op(op: BinOp) -> Value {
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

fn decode_cmp_op(value: &Value) -> Result<CmpOp, DecodeError> {
    let (tag, fields) = variant(value)?;
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

fn encode_cmp_op(op: CmpOp) -> Value {
    match op {
        CmpOp::Bool(crate::lang::xl::bool::CmpOp::Eq) => json!(["EqOp"]),
        CmpOp::Bool(crate::lang::xl::bool::CmpOp::Ne) => json!(["NeOp"]),
        CmpOp::Num(crate::lang::xl::num::CmpOp::Lt) => json!(["LtOp"]),
        CmpOp::Num(crate::lang::xl::num::CmpOp::Gt) => json!(["GtOp"]),
        CmpOp::Num(crate::lang::xl::num::CmpOp::Le) => json!(["LeOp"]),
        CmpOp::Num(crate::lang::xl::num::CmpOp::Ge) => json!(["GeOp"]),
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
        ("BoolT", []) => Ok(PlainTypKind::Bool),
        ("NumT", [typ]) => Ok(PlainTypKind::Num(xl::decode_num_typ(typ)?)),
        ("TextT", []) => Ok(PlainTypKind::Text),
        ("VarT", [id, targs]) => Ok(PlainTypKind::Var(
            decode_id(id)?,
            decode_list(targs, decode_targ)?,
        )),
        ("ParenT", [typ]) => Ok(PlainTypKind::Paren(Box::new(decode_plain_typ(typ)?))),
        ("TupleT", [types]) => Ok(PlainTypKind::Tuple(decode_list(types, decode_plain_typ)?)),
        ("IterT", [typ, iter]) => Ok(PlainTypKind::Iter(
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

fn decode_path(value: &Value) -> Result<ast::Path, DecodeError> {
    source::decode_phrase(value, decode_path_kind)
}

fn encode_path(path: &ast::Path) -> Value {
    source::encode_phrase(path, encode_path_kind)
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
            Err(DecodeError::Expected("valid EL path arity"))
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
            ("DefA", [id]) => Ok(ArgKind::Def(decode_id(id)?)),
            ("ExpA" | "DefA", _) => Err(DecodeError::Expected("valid EL argument arity")),
            (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
        }
    })
}

fn encode_arg(arg: &ast::Arg) -> Value {
    source::encode_phrase(arg, |arg| match arg {
        ArgKind::Exp(exp) => json!(["ExpA", encode_exp(exp)]),
        ArgKind::Def(id) => json!(["DefA", encode_id(id)]),
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

pub(super) fn decode_exp(value: &Value) -> Result<ast::Exp, DecodeError> {
    source::decode_phrase(value, decode_exp_kind)
}

pub(super) fn encode_exp(exp: &ast::Exp) -> Value {
    source::encode_phrase(exp, encode_exp_kind)
}

fn decode_exp_kind(value: &Value) -> Result<ExpKind, DecodeError> {
    let (tag, fields) = variant(value)?;
    match (tag, fields) {
        ("BoolE", [value]) => Ok(ExpKind::Bool(super::super::boolean(value)?)),
        ("NumE", [op, num]) => Ok(ExpKind::Num(decode_num_op(op)?, xl::decode_num(num)?)),
        ("TextE", [text]) => Ok(ExpKind::Text(string(text)?.to_owned())),
        ("VarE", [id]) => Ok(ExpKind::Var(decode_id(id)?)),
        ("UnE", [op, exp]) => Ok(ExpKind::Un(decode_un_op(op)?, Box::new(decode_exp(exp)?))),
        ("BinE", [left, op, right]) => Ok(ExpKind::Bin(
            Box::new(decode_exp(left)?),
            decode_bin_op(op)?,
            Box::new(decode_exp(right)?),
        )),
        ("CmpE", [left, op, right]) => Ok(ExpKind::Cmp(
            Box::new(decode_exp(left)?),
            decode_cmp_op(op)?,
            Box::new(decode_exp(right)?),
        )),
        ("ArithE", [exp]) => Ok(ExpKind::Arith(Box::new(decode_exp(exp)?))),
        ("EpsE", []) => Ok(ExpKind::Eps),
        ("ListE", [exps]) => Ok(ExpKind::List(decode_list(exps, decode_exp)?)),
        ("ConsE", [head, tail]) => Ok(ExpKind::Cons(
            Box::new(decode_exp(head)?),
            Box::new(decode_exp(tail)?),
        )),
        ("CatE", [left, right]) => Ok(ExpKind::Cat(
            Box::new(decode_exp(left)?),
            Box::new(decode_exp(right)?),
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
        ("LenE", [exp]) => Ok(ExpKind::Len(Box::new(decode_exp(exp)?))),
        ("MemE", [left, right]) => Ok(ExpKind::Mem(
            Box::new(decode_exp(left)?),
            Box::new(decode_exp(right)?),
        )),
        ("StrE", [fields]) => Ok(ExpKind::Str(decode_list(fields, |field| {
            match array(field)? {
                [atom, exp] => Ok((AtomPhraseCodec::decode(atom)?, decode_exp(exp)?)),
                _ => Err(DecodeError::Expected("EL structure field pair")),
            }
        })?)),
        ("DotE", [exp, atom]) => Ok(ExpKind::Dot(
            Box::new(decode_exp(exp)?),
            AtomPhraseCodec::decode(atom)?,
        )),
        ("UpdE", [base, path, value]) => Ok(ExpKind::Upd(
            Box::new(decode_exp(base)?),
            decode_path(path)?,
            Box::new(decode_exp(value)?),
        )),
        ("ParenE", [exp]) => Ok(ExpKind::Paren(Box::new(decode_exp(exp)?))),
        ("TupleE", [exps]) => Ok(ExpKind::Tuple(decode_list(exps, decode_exp)?)),
        ("CallE", [id, targs, args]) => Ok(ExpKind::Call(
            decode_id(id)?,
            decode_list(targs, decode_targ)?,
            decode_list(args, decode_arg)?,
        )),
        ("IterE", [exp, iter]) => Ok(ExpKind::Iter(
            Box::new(decode_exp(exp)?),
            decode_iter(iter)?,
        )),
        ("SubE", [exp, typ]) => Ok(ExpKind::Sub(
            Box::new(decode_exp(exp)?),
            decode_plain_typ(typ)?,
        )),
        ("AtomE", [atom]) => Ok(ExpKind::Atom(AtomPhraseCodec::decode(atom)?)),
        ("SeqE", [exps]) => Ok(ExpKind::Seq(decode_list(exps, decode_exp)?)),
        ("InfixE", [left, atom, right]) => Ok(ExpKind::Infix(
            Box::new(decode_exp(left)?),
            AtomPhraseCodec::decode(atom)?,
            Box::new(decode_exp(right)?),
        )),
        ("BrackE", [left, exp, right]) => Ok(ExpKind::Brack(
            AtomPhraseCodec::decode(left)?,
            Box::new(decode_exp(exp)?),
            AtomPhraseCodec::decode(right)?,
        )),
        ("HoleE", [hole]) => Ok(ExpKind::Hole(decode_hole(hole)?)),
        ("FuseE", [left, right]) => Ok(ExpKind::Fuse(
            Box::new(decode_exp(left)?),
            Box::new(decode_exp(right)?),
        )),
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

fn encode_exp_kind(exp: &ExpKind) -> Value {
    match exp {
        ExpKind::Bool(value) => json!(["BoolE", value]),
        ExpKind::Num(op, num) => json!(["NumE", encode_num_op(*op), xl::encode_num(num)]),
        ExpKind::Text(text) => json!(["TextE", text]),
        ExpKind::Var(id) => json!(["VarE", encode_id(id)]),
        ExpKind::Un(op, exp) => json!(["UnE", encode_un_op(*op), encode_exp(exp)]),
        ExpKind::Bin(left, op, right) => {
            json!([
                "BinE",
                encode_exp(left),
                encode_bin_op(*op),
                encode_exp(right)
            ])
        }
        ExpKind::Cmp(left, op, right) => {
            json!([
                "CmpE",
                encode_exp(left),
                encode_cmp_op(*op),
                encode_exp(right)
            ])
        }
        ExpKind::Arith(exp) => json!(["ArithE", encode_exp(exp)]),
        ExpKind::Eps => json!(["EpsE"]),
        ExpKind::List(exps) => json!(["ListE", encode_list(exps, encode_exp)]),
        ExpKind::Cons(head, tail) => json!(["ConsE", encode_exp(head), encode_exp(tail)]),
        ExpKind::Cat(left, right) => json!(["CatE", encode_exp(left), encode_exp(right)]),
        ExpKind::Idx(base, index) => json!(["IdxE", encode_exp(base), encode_exp(index)]),
        ExpKind::Slice(base, left, right) => json!([
            "SliceE",
            encode_exp(base),
            encode_exp(left),
            encode_exp(right)
        ]),
        ExpKind::Len(exp) => json!(["LenE", encode_exp(exp)]),
        ExpKind::Mem(left, right) => json!(["MemE", encode_exp(left), encode_exp(right)]),
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
        ExpKind::Upd(base, path, value) => {
            json!([
                "UpdE",
                encode_exp(base),
                encode_path(path),
                encode_exp(value)
            ])
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
        ExpKind::Infix(left, atom, right) => json!([
            "InfixE",
            encode_exp(left),
            AtomPhraseCodec::encode(atom),
            encode_exp(right)
        ]),
        ExpKind::Brack(left, exp, right) => json!([
            "BrackE",
            AtomPhraseCodec::encode(left),
            encode_exp(exp),
            AtomPhraseCodec::encode(right)
        ]),
        ExpKind::Hole(hole) => json!(["HoleE", encode_hole(hole)]),
        ExpKind::Fuse(left, right) => json!(["FuseE", encode_exp(left), encode_exp(right)]),
        ExpKind::Unparen(exp) => json!(["UnparenE", encode_exp(exp)]),
        ExpKind::Latex(latex) => json!(["LatexE", latex]),
    }
}

pub(super) fn decode_hint(value: &Value) -> Result<ast::Hint, DecodeError> {
    let object = object(value)?;
    Ok((
        decode_id(field(object, "hintid")?)?,
        decode_exp(field(object, "hintexp")?)?,
    ))
}

pub(super) fn encode_hint(hint: &ast::Hint) -> Value {
    json!({
        "hintid": encode_id(&hint.0),
        "hintexp": encode_exp(&hint.1),
    })
}

fn decode_typ(value: &Value) -> Result<ast::Typ, DecodeError> {
    let (tag, fields) = variant(value)?;
    match (tag, fields) {
        ("PlainT", [typ]) => Ok(ast::Typ::Plain(decode_plain_typ(typ)?)),
        ("NotationT", [typ]) => Ok(ast::Typ::Notation(decode_not_typ(typ)?)),
        ("PlainT" | "NotationT", _) => Err(DecodeError::Expected("valid EL type arity")),
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

fn encode_typ(typ: &ast::Typ) -> Value {
    match typ {
        ast::Typ::Plain(typ) => json!(["PlainT", encode_plain_typ(typ)]),
        ast::Typ::Notation(typ) => json!(["NotationT", encode_not_typ(typ)]),
    }
}

fn decode_not_typ(value: &Value) -> Result<ast::NotTyp, DecodeError> {
    source::decode_phrase(value, |value| {
        let (tag, fields) = variant(value)?;
        match (tag, fields) {
            ("AtomT", [atom]) => Ok(NotTypKind::Atom(AtomPhraseCodec::decode(atom)?)),
            ("SeqT", [types]) => Ok(NotTypKind::Seq(decode_list(types, decode_typ)?)),
            ("InfixT", [left, atom, right]) => Ok(NotTypKind::Infix(
                Box::new(decode_typ(left)?),
                AtomPhraseCodec::decode(atom)?,
                Box::new(decode_typ(right)?),
            )),
            ("BrackT", [left, typ, right]) => Ok(NotTypKind::Brack(
                AtomPhraseCodec::decode(left)?,
                Box::new(decode_typ(typ)?),
                AtomPhraseCodec::decode(right)?,
            )),
            ("AtomT" | "SeqT" | "InfixT" | "BrackT", _) => {
                Err(DecodeError::Expected("valid EL notation type arity"))
            }
            (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
        }
    })
}

fn encode_not_typ(typ: &ast::NotTyp) -> Value {
    source::encode_phrase(typ, |typ| match typ {
        NotTypKind::Atom(atom) => json!(["AtomT", AtomPhraseCodec::encode(atom)]),
        NotTypKind::Seq(types) => json!(["SeqT", encode_list(types, encode_typ)]),
        NotTypKind::Infix(left, atom, right) => json!([
            "InfixT",
            encode_typ(left),
            AtomPhraseCodec::encode(atom),
            encode_typ(right)
        ]),
        NotTypKind::Brack(left, typ, right) => json!([
            "BrackT",
            AtomPhraseCodec::encode(left),
            encode_typ(typ),
            AtomPhraseCodec::encode(right)
        ]),
    })
}

fn decode_def_typ(value: &Value) -> Result<ast::DefTyp, DecodeError> {
    source::decode_phrase(value, |value| {
        let (tag, fields) = variant(value)?;
        match (tag, fields) {
            ("PlainTD", [typ]) => Ok(DefTypKind::Plain(decode_plain_typ(typ)?)),
            ("StructTD", [fields]) => {
                Ok(DefTypKind::Struct(decode_list(fields, decode_typ_field)?))
            }
            ("VariantTD", [cases]) => Ok(DefTypKind::Variant(decode_list(cases, decode_typ_case)?)),
            ("PlainTD" | "StructTD" | "VariantTD", _) => {
                Err(DecodeError::Expected("valid EL defined type arity"))
            }
            (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
        }
    })
}

fn encode_def_typ(typ: &ast::DefTyp) -> Value {
    source::encode_phrase(typ, |typ| match typ {
        DefTypKind::Plain(typ) => json!(["PlainTD", encode_plain_typ(typ)]),
        DefTypKind::Struct(fields) => json!(["StructTD", encode_list(fields, encode_typ_field)]),
        DefTypKind::Variant(cases) => json!(["VariantTD", encode_list(cases, encode_typ_case)]),
    })
}

fn decode_typ_field(value: &Value) -> Result<ast::TypField, DecodeError> {
    match array(value)? {
        [atom, typ, hints] => Ok((
            AtomPhraseCodec::decode(atom)?,
            decode_plain_typ(typ)?,
            decode_list(hints, decode_hint)?,
        )),
        _ => Err(DecodeError::Expected("EL type field triple")),
    }
}

fn encode_typ_field(field: &ast::TypField) -> Value {
    json!([
        AtomPhraseCodec::encode(&field.0),
        encode_plain_typ(&field.1),
        encode_list(&field.2, encode_hint)
    ])
}

fn decode_typ_case(value: &Value) -> Result<ast::TypCase, DecodeError> {
    match array(value)? {
        [typ, hints] => Ok((decode_typ(typ)?, decode_list(hints, decode_hint)?)),
        _ => Err(DecodeError::Expected("EL type case pair")),
    }
}

fn encode_typ_case((typ, hints): &ast::TypCase) -> Value {
    json!([encode_typ(typ), encode_list(hints, encode_hint)])
}

fn decode_tparam(value: &Value) -> Result<ast::TParam, DecodeError> {
    source::decode_phrase(value, |value| Ok(string(value)?.to_owned()))
}

fn encode_tparam(param: &ast::TParam) -> Value {
    source::encode_phrase(param, |param| json!(param))
}

fn decode_param(value: &Value) -> Result<ast::Param, DecodeError> {
    source::decode_phrase(value, |value| {
        let (tag, fields) = variant(value)?;
        match (tag, fields) {
            ("ExpP", [typ]) => Ok(ParamKind::Exp(decode_plain_typ(typ)?)),
            ("DefP", [id, tparams, params, typ]) => Ok(ParamKind::Def(
                decode_id(id)?,
                decode_list(tparams, decode_tparam)?,
                decode_list(params, decode_param)?,
                decode_plain_typ(typ)?,
            )),
            ("ExpP" | "DefP", _) => Err(DecodeError::Expected("valid EL parameter arity")),
            (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
        }
    })
}

fn encode_param(param: &ast::Param) -> Value {
    source::encode_phrase(param, |param| match param {
        ParamKind::Exp(typ) => json!(["ExpP", encode_plain_typ(typ)]),
        ParamKind::Def(id, tparams, params, typ) => json!([
            "DefP",
            encode_id(id),
            encode_list(tparams, encode_tparam),
            encode_list(params, encode_param),
            encode_plain_typ(typ)
        ]),
    })
}

fn decode_prem(value: &Value) -> Result<ast::Prem, DecodeError> {
    source::decode_phrase(value, |value| {
        let (tag, fields) = variant(value)?;
        match (tag, fields) {
            ("VarPr", [id, typ]) => Ok(PremKind::Var(VarPrem {
                id: decode_id(id)?,
                plain_typ: decode_plain_typ(typ)?,
            })),
            ("RulePr", [id, exp]) => Ok(PremKind::Rule(RulePrem {
                id: decode_id(id)?,
                exp: decode_exp(exp)?,
            })),
            ("RuleNotPr", [id, exp]) => Ok(PremKind::RuleNot(RuleNotPrem {
                id: decode_id(id)?,
                exp: decode_exp(exp)?,
            })),
            ("IfPr", [exp]) => Ok(PremKind::If(IfPrem {
                exp: decode_exp(exp)?,
            })),
            ("ElsePr", []) => Ok(PremKind::Else),
            ("IterPr", [prem, iter]) => Ok(PremKind::Iter(IterPrem {
                prem: Box::new(decode_prem(prem)?),
                iter: decode_iter(iter)?,
            })),
            ("DebugPr", [exp]) => Ok(PremKind::Debug(DebugPrem {
                exp: decode_exp(exp)?,
            })),
            ("VarPr" | "RulePr" | "RuleNotPr" | "IfPr" | "ElsePr" | "IterPr" | "DebugPr", _) => {
                Err(DecodeError::Expected("valid EL premise arity"))
            }
            (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
        }
    })
}

fn encode_prem(prem: &ast::Prem) -> Value {
    source::encode_phrase(prem, |prem| match prem {
        PremKind::Var(VarPrem { id, plain_typ }) => {
            json!(["VarPr", encode_id(id), encode_plain_typ(plain_typ)])
        }
        PremKind::Rule(RulePrem { id, exp }) => json!(["RulePr", encode_id(id), encode_exp(exp)]),
        PremKind::RuleNot(RuleNotPrem { id, exp }) => {
            json!(["RuleNotPr", encode_id(id), encode_exp(exp)])
        }
        PremKind::If(IfPrem { exp }) => json!(["IfPr", encode_exp(exp)]),
        PremKind::Else => json!(["ElsePr"]),
        PremKind::Iter(IterPrem { prem, iter }) => {
            json!(["IterPr", encode_prem(prem), encode_iter(*iter)])
        }
        PremKind::Debug(DebugPrem { exp }) => json!(["DebugPr", encode_exp(exp)]),
    })
}

fn decode_rule(value: &Value) -> Result<ast::Rule, DecodeError> {
    source::decode_phrase(value, |value| match array(value)? {
        [rel, id, exp, prems] => Ok((
            decode_id(rel)?,
            decode_id(id)?,
            decode_exp(exp)?,
            decode_list(prems, decode_prem)?,
        )),
        _ => Err(DecodeError::Expected("EL rule quadruple")),
    })
}

fn encode_rule(rule: &ast::Rule) -> Value {
    source::encode_phrase(rule, |rule| {
        json!([
            encode_id(&rule.0),
            encode_id(&rule.1),
            encode_exp(&rule.2),
            encode_list(&rule.3, encode_prem)
        ])
    })
}

fn decode_table_row(value: &Value) -> Result<ast::TableRow, DecodeError> {
    source::decode_phrase(value, |value| match array(value)? {
        [left, right] => Ok((decode_exp(left)?, decode_exp(right)?)),
        _ => Err(DecodeError::Expected("EL table row pair")),
    })
}

fn encode_table_row(row: &ast::TableRow) -> Value {
    source::encode_phrase(row, |(left, right)| {
        json!([encode_exp(left), encode_exp(right)])
    })
}

fn decode_syn_entry(value: &Value) -> Result<(ast::Id, Vec<ast::TParam>), DecodeError> {
    match array(value)? {
        [id, params] => Ok((decode_id(id)?, decode_list(params, decode_tparam)?)),
        _ => Err(DecodeError::Expected("EL syntax entry pair")),
    }
}

fn decode_def(value: &Value) -> Result<ast::Def, DecodeError> {
    source::decode_phrase(value, |value| {
        let (tag, fields) = variant(value)?;
        match (tag, fields) {
            ("ExternSynD", [id, hints]) => Ok(DefKind::ExternSyntax(ExternSyntaxDef {
                id: decode_id(id)?,
                hints: decode_list(hints, decode_hint)?,
            })),
            ("SynD", [entries]) => Ok(DefKind::Syntax(SyntaxDef {
                entries: decode_list(entries, decode_syn_entry)?
                    .into_iter()
                    .map(|(id, tparams)| SyntaxDefEntry { id, tparams })
                    .collect(),
            })),
            ("TypD", [id, params, typ, hints]) => Ok(DefKind::Typ(TypDef {
                id: decode_id(id)?,
                tparams: decode_list(params, decode_tparam)?,
                def_typ: decode_def_typ(typ)?,
                hints: decode_list(hints, decode_hint)?,
            })),
            ("VarD", [id, typ, hints]) => Ok(DefKind::Var(VarDef {
                id: decode_id(id)?,
                plain_typ: decode_plain_typ(typ)?,
                hints: decode_list(hints, decode_hint)?,
            })),
            ("ExternRelD", [id, typ, hints]) => Ok(DefKind::ExternRel(ExternRelDef {
                id: decode_id(id)?,
                not_typ: decode_not_typ(typ)?,
                hints: decode_list(hints, decode_hint)?,
            })),
            ("RelD", [id, typ, hints]) => Ok(DefKind::Rel(RelDef {
                id: decode_id(id)?,
                not_typ: decode_not_typ(typ)?,
                hints: decode_list(hints, decode_hint)?,
            })),
            ("RuleGroupD", [id, anchor, rules]) => Ok(DefKind::RuleGroup(RuleGroupDef {
                relid: decode_id(id)?,
                groupid: decode_id(anchor)?,
                rules: decode_list(rules, decode_rule)?,
            })),
            ("ExternDecD", [id, tparams, params, typ, hints]) => {
                Ok(DefKind::ExternDec(ExternDecDef {
                    id: decode_id(id)?,
                    tparams: decode_list(tparams, decode_tparam)?,
                    params: decode_list(params, decode_param)?,
                    plain_typ: decode_plain_typ(typ)?,
                    hints: decode_list(hints, decode_hint)?,
                }))
            }
            ("BuiltinDecD", [id, tparams, params, typ, hints]) => {
                Ok(DefKind::BuiltinDec(BuiltinDecDef {
                    id: decode_id(id)?,
                    tparams: decode_list(tparams, decode_tparam)?,
                    params: decode_list(params, decode_param)?,
                    plain_typ: decode_plain_typ(typ)?,
                    hints: decode_list(hints, decode_hint)?,
                }))
            }
            ("TableDecD", [id, params, typ, hints]) => Ok(DefKind::TableDec(TableDecDef {
                id: decode_id(id)?,
                params: decode_list(params, decode_param)?,
                plain_typ: decode_plain_typ(typ)?,
                hints: decode_list(hints, decode_hint)?,
            })),
            ("FuncDecD", [id, tparams, params, typ, hints]) => Ok(DefKind::FuncDec(FuncDecDef {
                id: decode_id(id)?,
                tparams: decode_list(tparams, decode_tparam)?,
                params: decode_list(params, decode_param)?,
                plain_typ: decode_plain_typ(typ)?,
                hints: decode_list(hints, decode_hint)?,
            })),
            ("TableDefD", [id, rows]) => Ok(DefKind::TableDef(TableDef {
                id: decode_id(id)?,
                rows: decode_list(rows, decode_table_row)?,
            })),
            ("FuncDefD", [id, tparams, args, exp, prems]) => Ok(DefKind::FuncDef(FuncDef {
                id: decode_id(id)?,
                tparams: decode_list(tparams, decode_tparam)?,
                args: decode_list(args, decode_arg)?,
                exp: decode_exp(exp)?,
                prems: decode_list(prems, decode_prem)?,
            })),
            ("SepD", []) => Ok(DefKind::Sep),
            (
                "ExternSynD" | "SynD" | "TypD" | "VarD" | "ExternRelD" | "RelD" | "RuleGroupD"
                | "ExternDecD" | "BuiltinDecD" | "TableDecD" | "FuncDecD" | "TableDefD"
                | "FuncDefD" | "SepD",
                _,
            ) => Err(DecodeError::Expected("valid EL definition arity")),
            (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
        }
    })
}

fn encode_def(def: &ast::Def) -> Value {
    source::encode_phrase(def, |def| match def {
        DefKind::ExternSyntax(ExternSyntaxDef { id, hints }) => {
            json!(["ExternSynD", encode_id(id), encode_list(hints, encode_hint)])
        }
        DefKind::Syntax(SyntaxDef { entries }) => json!([
            "SynD",
            entries
                .iter()
                .map(|entry| json!([
                    encode_id(&entry.id),
                    encode_list(&entry.tparams, encode_tparam)
                ]))
                .collect::<Vec<_>>()
        ]),
        DefKind::Typ(TypDef {
            id,
            tparams,
            def_typ,
            hints,
        }) => json!([
            "TypD",
            encode_id(id),
            encode_list(tparams, encode_tparam),
            encode_def_typ(def_typ),
            encode_list(hints, encode_hint)
        ]),
        DefKind::Var(VarDef {
            id,
            plain_typ,
            hints,
        }) => json!([
            "VarD",
            encode_id(id),
            encode_plain_typ(plain_typ),
            encode_list(hints, encode_hint)
        ]),
        DefKind::ExternRel(ExternRelDef { id, not_typ, hints }) => json!([
            "ExternRelD",
            encode_id(id),
            encode_not_typ(not_typ),
            encode_list(hints, encode_hint)
        ]),
        DefKind::Rel(RelDef { id, not_typ, hints }) => json!([
            "RelD",
            encode_id(id),
            encode_not_typ(not_typ),
            encode_list(hints, encode_hint)
        ]),
        DefKind::RuleGroup(RuleGroupDef {
            relid,
            groupid,
            rules,
        }) => json!([
            "RuleGroupD",
            encode_id(relid),
            encode_id(groupid),
            encode_list(rules, encode_rule)
        ]),
        DefKind::ExternDec(ExternDecDef {
            id,
            tparams,
            params,
            plain_typ,
            hints,
        }) => json!([
            "ExternDecD",
            encode_id(id),
            encode_list(tparams, encode_tparam),
            encode_list(params, encode_param),
            encode_plain_typ(plain_typ),
            encode_list(hints, encode_hint)
        ]),
        DefKind::BuiltinDec(BuiltinDecDef {
            id,
            tparams,
            params,
            plain_typ,
            hints,
        }) => json!([
            "BuiltinDecD",
            encode_id(id),
            encode_list(tparams, encode_tparam),
            encode_list(params, encode_param),
            encode_plain_typ(plain_typ),
            encode_list(hints, encode_hint)
        ]),
        DefKind::TableDec(TableDecDef {
            id,
            params,
            plain_typ,
            hints,
        }) => json!([
            "TableDecD",
            encode_id(id),
            encode_list(params, encode_param),
            encode_plain_typ(plain_typ),
            encode_list(hints, encode_hint)
        ]),
        DefKind::FuncDec(FuncDecDef {
            id,
            tparams,
            params,
            plain_typ,
            hints,
        }) => json!([
            "FuncDecD",
            encode_id(id),
            encode_list(tparams, encode_tparam),
            encode_list(params, encode_param),
            encode_plain_typ(plain_typ),
            encode_list(hints, encode_hint)
        ]),
        DefKind::TableDef(TableDef { id, rows }) => json!([
            "TableDefD",
            encode_id(id),
            encode_list(rows, encode_table_row)
        ]),
        DefKind::FuncDef(FuncDef {
            id,
            tparams,
            args,
            exp,
            prems,
        }) => json!([
            "FuncDefD",
            encode_id(id),
            encode_list(tparams, encode_tparam),
            encode_list(args, encode_arg),
            encode_exp(exp),
            encode_list(prems, encode_prem)
        ]),
        DefKind::Sep => json!(["SepD"]),
    })
}
