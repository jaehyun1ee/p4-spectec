//! Building and reading values of a specification by their notation text
//!
//! A `Registry` maps the printed notation of every variant case of a
//! specification, such as `IfS % % %`, to its mixop and type name.
//! Values are built as trees with `Registry::case` and friends,
//! then interned with `into_flat`; flat values are read back through `into_tree`.

use std::{collections::HashMap, rc::Rc};

use p4spectec::lang::{
    al::ast as al,
    common::{Id, Iter, prim::num::Number, source::NotePhrase},
    data::{
        arena::Arena,
        notation::tree::Mixop,
        typ::{Typ, TypKind},
        value::{
            flat,
            tree::{Value, ValueCase, ValueKind},
        },
    },
    il::ast::{DefTypKind, TypDef},
    traits::print::Print,
};

/// A value as an owned tree.
pub type Tree = Value;

/// A failure of the driver, as a message.
pub type Error = String;

/// The notation text of a mixop, e.g. `IfS % % %`.
pub fn ctor_of(mixop: &Mixop) -> String {
    Print::to_string(mixop)
}

// = Types

/// A type without a source position.
pub fn typ(kind: TypKind) -> Typ {
    NotePhrase { node: kind, note: (), span: Default::default() }
}

/// The type named `name`, without arguments.
pub fn typ_var(name: &str) -> TypKind {
    let id: Id = NotePhrase { node: Rc::from(name), note: (), span: Default::default() };
    TypKind::Var(id, Vec::new())
}

/// The type of a list of `elem`.
pub fn typ_list(elem: TypKind) -> TypKind {
    TypKind::Iter(Box::new(typ(elem)), Iter::List)
}

/// The type of an option of `elem`.
pub fn typ_opt(elem: TypKind) -> TypKind {
    TypKind::Iter(Box::new(typ(elem)), Iter::Opt)
}

// = Trees

fn tree(kind: ValueKind, note: TypKind) -> Value {
    NotePhrase { node: kind, note, span: Default::default() }
}

pub fn bool(value: bool) -> Value {
    tree(ValueKind::Bool(value), TypKind::Bool)
}

pub fn nat(value: u64) -> Value {
    tree(
        ValueKind::Num(Number::Nat(value.into())),
        TypKind::Num(p4spectec::lang::common::prim::num::Typ::Nat),
    )
}

pub fn text(value: &str) -> Value {
    tree(ValueKind::Text(value.to_owned()), TypKind::Text)
}

pub fn tuple(values: Vec<Value>) -> Value {
    let typs = values.iter().map(|value| typ(value.note.clone())).collect();
    tree(ValueKind::Tuple(values), TypKind::Tuple(typs))
}

pub fn list(elem: TypKind, values: Vec<Value>) -> Value {
    tree(ValueKind::List(values), typ_list(elem))
}

pub fn opt(elem: TypKind, value: Option<Value>) -> Value {
    tree(ValueKind::Opt(value.map(Box::new)), typ_opt(elem))
}

pub fn external(note: TypKind, json: serde_json::Value) -> Value {
    tree(ValueKind::Extern(Rc::new(json)), note)
}

/// Fills the argument positions of `mixop` with `args` in notation order.
fn fill(mixop: &Mixop, args: &mut impl Iterator<Item = Value>) -> Result<ValueCase, Error> {
    Ok(match mixop {
        Mixop::Arg => ValueCase::Arg(Box::new(args.next().ok_or("too few arguments for a case")?)),
        Mixop::Atom(atom) => ValueCase::Atom(atom.clone()),
        Mixop::Brack(atom_l, inner, atom_r) => {
            ValueCase::Brack(atom_l.clone(), Box::new(fill(inner, args)?), atom_r.clone())
        }
        Mixop::Infix(left, atom, right) => ValueCase::Infix(
            Box::new(fill(left, args)?),
            atom.clone(),
            Box::new(fill(right, args)?),
        ),
        Mixop::Seq(mixops) => ValueCase::Seq(
            mixops
                .iter()
                .map(|mixop| fill(mixop, args))
                .collect::<Result<_, _>>()?,
        ),
    })
}

/// Interns a tree in the arena.
pub fn intern(arena: &mut Arena, value: Value) -> Result<flat::Value, Error> {
    value.into_flat(arena).map_err(|error| error.to_string())
}

/// Reads a flat value as a tree.
pub fn read(arena: &Arena, value: flat::Value) -> Value {
    value.into_tree(arena)
}

// = Registry

/// The variant cases of a specification, by notation text.
pub struct Registry {
    cases: HashMap<String, Vec<(Rc<Mixop>, String)>>,
}

impl Registry {
    /// Collects the variant cases of every syntax definition in `spec`.
    pub fn new(spec: &al::Spec) -> Self {
        let mut cases: HashMap<String, Vec<(Rc<Mixop>, String)>> = HashMap::new();
        for def in spec {
            let al::DefKind::Typ(TypDef::Defined(typ_def)) = &def.node else { continue };
            let DefTypKind::Variant(typ_cases) = &typ_def.def_typ.node else { continue };
            for typ_case in typ_cases {
                let mixop = typ_case.not_typ.node.mixop();
                cases
                    .entry(ctor_of(mixop))
                    .or_default()
                    .push((mixop.clone(), typ_def.id.node.to_string()));
            }
        }
        Self { cases }
    }

    /// The mixop and type name of the case printed as `ctor`.
    pub fn lookup(&self, ctor: &str) -> Result<&(Rc<Mixop>, String), Error> {
        match self.cases.get(ctor).map(Vec::as_slice) {
            Some([entry]) => Ok(entry),
            Some(_) => Err(format!("notation `{ctor}` names cases of several types")),
            None => Err(format!("no case has the notation `{ctor}`")),
        }
    }

    /// A case of its own variant type.
    pub fn case(&self, ctor: &str, args: Vec<Value>) -> Result<Value, Error> {
        let (_, typ_name) = self.lookup(ctor)?;
        self.case_as(typ_var(typ_name), ctor, args)
    }

    /// A case typed as `note`, e.g. an instance of a parametric variant.
    pub fn case_as(&self, note: TypKind, ctor: &str, args: Vec<Value>) -> Result<Value, Error> {
        let (mixop, _) = self.lookup(ctor)?;
        let mut args = args.into_iter();
        let value_case = fill(mixop, &mut args)?;
        if args.next().is_some() {
            return Err(format!("too many arguments for `{ctor}`"));
        }
        Ok(tree(ValueKind::Case(value_case), note))
    }
}

/// The notation text and arguments of a case tree.
pub fn uncase(value: Value) -> Result<(String, Vec<Value>), Error> {
    match value.node {
        ValueKind::Case(value_case) => {
            let (mixop, args) = value_case.into_parts();
            Ok((ctor_of(&mixop), args))
        }
        _ => Err("expected a case".to_owned()),
    }
}

/// Reads flat values as trees.
pub fn read_many(arena: &Arena, values: &[flat::Value]) -> Vec<Value> {
    values.iter().map(|value| read(arena, *value)).collect()
}

/// The atoms and arguments of a case tree in notation order, without consuming it.
pub fn items(value: &Value) -> Result<Vec<Result<String, &Value>>, Error> {
    fn walk<'v>(value_case: &'v ValueCase, out: &mut Vec<Result<String, &'v Value>>) {
        match value_case {
            ValueCase::Arg(value) => out.push(Err(value)),
            ValueCase::Atom(atom) => out.push(Ok(Print::to_string(&atom.node))),
            ValueCase::Brack(atom_l, inner, atom_r) => {
                out.push(Ok(Print::to_string(&atom_l.node)));
                walk(inner, out);
                out.push(Ok(Print::to_string(&atom_r.node)));
            }
            ValueCase::Infix(left, atom, right) => {
                walk(left, out);
                out.push(Ok(Print::to_string(&atom.node)));
                walk(right, out);
            }
            ValueCase::Seq(value_cases) => value_cases
                .iter()
                .for_each(|value_case| walk(value_case, out)),
        }
    }
    let ValueKind::Case(value_case) = &value.node else { return Err("expected a case".to_owned()) };
    let mut out = Vec::new();
    walk(value_case, &mut out);
    Ok(out)
}
