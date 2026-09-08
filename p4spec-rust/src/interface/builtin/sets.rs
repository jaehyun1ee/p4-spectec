//! Set operations using contextual structural value ordering

use super::{BuiltinError, extract};
use crate::lang::{
    common::{notation::mixop::Mixop, source::Span},
    data::{
        typ,
        value::{Value, ValueArena, get, make, shape},
    },
    il::ast::Typ,
};
use std::rc::Rc;

type ValueSet = Vec<Value>;

fn set_mixop() -> Rc<Mixop> {
    shape("`{ k `}")
}

fn normalize(arena: &ValueArena, values: &mut ValueSet) {
    values.sort_by(|left, right| arena.compare(left, right));
    values.dedup_by(|left, right| arena.equal(left, right));
}

fn contains(arena: &ValueArena, set: &[Value], value: &Value) -> bool {
    set.binary_search_by(|candidate| arena.compare(candidate, value))
        .is_ok()
}

fn set_of_value(arena: &ValueArena, value: &Value) -> Result<ValueSet, BuiltinError> {
    let case = get::case(arena, value).map_err(|_| BuiltinError::new("expected a set"))?;
    if !case.eq_shape(set_mixop().as_ref()) {
        return Err(BuiltinError::new("expected a set"));
    }
    let args = case.args();
    let elements = extract::one(&args)?;
    let mut values = get::list(arena, elements)
        .map_err(|_| BuiltinError::new("expected a set"))?
        .to_vec();
    normalize(arena, &mut values);
    Ok(values)
}

fn value_of_set(
    arena: &mut ValueArena,
    typ_key: &Typ,
    set: ValueSet,
) -> Result<Value, BuiltinError> {
    let typ_list = typ::make::list(typ_key.clone());
    let elements = make::list(arena, &typ_list, set, Span::default())?;
    let id = crate::phrase!(node: "set".to_owned(), span: Span::default());
    let typ = typ::make::var(id, vec![typ_key.clone()]);
    let case = Mixop::fill(set_mixop().as_ref(), [elements])
        .expect("the set mixop has exactly one argument");
    Ok(make::case_(arena, &typ, case, Span::default())?)
}

pub fn intersect_set(
    arena: &mut ValueArena,
    targs: &[Typ],
    values: &[Value],
) -> Result<Value, BuiltinError> {
    let typ = extract::one(targs)?;
    let (left, right) = extract::two(values)?;
    let left = set_of_value(arena, left)?;
    let right = set_of_value(arena, right)?;
    let set = left
        .into_iter()
        .filter(|value| contains(arena, &right, value))
        .collect();
    value_of_set(arena, typ, set)
}

pub fn union_set(
    arena: &mut ValueArena,
    targs: &[Typ],
    values: &[Value],
) -> Result<Value, BuiltinError> {
    let typ = extract::one(targs)?;
    let (left, right) = extract::two(values)?;
    let mut set = set_of_value(arena, left)?;
    set.extend(set_of_value(arena, right)?);
    normalize(arena, &mut set);
    value_of_set(arena, typ, set)
}

pub fn unions_set(
    arena: &mut ValueArena,
    targs: &[Typ],
    values: &[Value],
) -> Result<Value, BuiltinError> {
    let typ = extract::one(targs)?;
    let sets = get::list(arena, extract::one(values)?)?;
    let mut union = Vec::new();
    for value in sets {
        union.extend(set_of_value(arena, value)?);
    }
    normalize(arena, &mut union);
    value_of_set(arena, typ, union)
}

pub fn diff_set(
    arena: &mut ValueArena,
    targs: &[Typ],
    values: &[Value],
) -> Result<Value, BuiltinError> {
    let typ = extract::one(targs)?;
    let (left, right) = extract::two(values)?;
    let left = set_of_value(arena, left)?;
    let right = set_of_value(arena, right)?;
    let set = left
        .into_iter()
        .filter(|value| !contains(arena, &right, value))
        .collect();
    value_of_set(arena, typ, set)
}

pub fn sub_set(
    arena: &mut ValueArena,
    targs: &[Typ],
    values: &[Value],
) -> Result<Value, BuiltinError> {
    let _typ = extract::one(targs)?;
    let (left, right) = extract::two(values)?;
    let left = set_of_value(arena, left)?;
    let right = set_of_value(arena, right)?;
    let subset = left.iter().all(|value| contains(arena, &right, value));
    Ok(make::bool(arena, subset, Span::default())?)
}

pub fn eq_set(
    arena: &mut ValueArena,
    targs: &[Typ],
    values: &[Value],
) -> Result<Value, BuiltinError> {
    let _typ = extract::one(targs)?;
    let (left, right) = extract::two(values)?;
    let left = set_of_value(arena, left)?;
    let right = set_of_value(arena, right)?;
    let equal = left.len() == right.len()
        && left
            .iter()
            .zip(&right)
            .all(|(left, right)| arena.equal(left, right));
    Ok(make::bool(arena, equal, Span::default())?)
}
