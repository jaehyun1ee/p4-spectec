//! Types shared by the intermediate language representations

use crate::lang::{
    common::{Id, Iter, TId, source::Phrase},
    xl::num,
};

pub type Typ = Phrase<TypKind>;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TypKind {
    /// `bool`
    Bool,
    /// `numtyp`
    Num(num::Typ),
    /// `text`
    Text,
    /// `id (`<` list(targ, `,`) `>`)?`
    Var(Id, Vec<Typ>),
    /// `(` list(typ, `,`) `)`
    Tuple(Vec<Typ>),
    /// `typ iter`
    Iter(Box<Typ>, Iter),
    /// `<` list(tparam, `,`) `>` `(` list(typ, `,`) `)` `:` typ
    Func(FuncTyp),
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FuncTyp {
    pub tparams: Vec<TId>,
    pub typs_params: Vec<Typ>,
    pub typ_ret: Box<Typ>,
}
