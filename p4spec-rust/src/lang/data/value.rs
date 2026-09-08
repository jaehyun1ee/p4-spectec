//! Immutable values shared by language representations and runtimes
//!
//! Values are compact handles into an explicitly owned append-only arena.
//! Child handles retain their type and source annotations. Projections,
//! structural comparison, and serialization resolve those handles in the arena.

use std::{cell::RefCell, cmp::Ordering, collections::HashMap, rc::Rc};

use thiserror::Error;

use crate::{
    frontend,
    lang::{
        common::{
            Id, TId,
            notation::{atom, mixfix::Mixfix, mixop::Mixop},
            source::{NotePhrase, Phrase, Span},
        },
        data::typ::{self, Typ, TypKind},
        xl::num::{self, Number},
    },
    yojson::ExternalData,
};

pub type Value = NotePhrase<ValueId, TypeId, SpanId>;

#[derive(Clone, Debug)]
pub enum ValueKind {
    Bool(bool),
    Num(Number),
    Text(String),
    Struct(Vec<ValueField>),
    Case(ValueCase),
    Tuple(Vec<Value>),
    Opt(Option<Value>),
    List(Vec<Value>),
    Func(Phrase<String, SpanId>),
    Extern(ExternalData),
}

pub type ValueField = (Phrase<atom::Atom, SpanId>, Value);
pub type ValueCase = Mixfix<Value, SpanId>;

/// An arena-local body index; handles are meaningful only in their owning arena
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct ValueId(u32);

/// An arena-local runtime type annotation index
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct TypeId(u32);

/// An arena-local source location index
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct SpanId(u32);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArenaIndexKind {
    Value,
    Type,
    Span,
}

/// Append-only storage shared by parser, evaluator, and host calls
#[derive(Debug, Default)]
pub struct ValueArena {
    values: Vec<ValueKind>,
    types: Vec<TypKind>,
    spans: Vec<Span>,
}

impl ValueArena {
    pub fn new() -> Self {
        Self::default()
    }

    fn index(len: usize, kind: ArenaIndexKind) -> Result<u32, ValueError> {
        u32::try_from(len).map_err(|_| ValueError::IndexOverflow { kind })
    }

    pub fn alloc_span(&mut self, span: Span) -> Result<SpanId, ValueError> {
        let id = SpanId(Self::index(self.spans.len(), ArenaIndexKind::Span)?);
        self.spans.push(span);
        Ok(id)
    }

    pub fn alloc_type(&mut self, typ: TypKind) -> Result<TypeId, ValueError> {
        let id = TypeId(Self::index(self.types.len(), ArenaIndexKind::Type)?);
        self.types.push(typ);
        Ok(id)
    }

    pub fn alloc(
        &mut self,
        kind: ValueKind,
        typ: TypKind,
        span: Span,
    ) -> Result<Value, ValueError> {
        let node = ValueId(Self::index(self.values.len(), ArenaIndexKind::Value)?);
        Self::index(self.types.len(), ArenaIndexKind::Type)?;
        Self::index(self.spans.len(), ArenaIndexKind::Span)?;
        let note = self.alloc_type(typ)?;
        let span = self.alloc_span(span)?;
        self.values.push(kind);
        Ok(crate::note_phrase!(node: node, note: note, span: span))
    }

    pub fn body(&self, id: ValueId) -> &ValueKind {
        &self.values[id.0 as usize]
    }
    pub fn type_annotation(&self, id: TypeId) -> &TypKind {
        &self.types[id.0 as usize]
    }
    pub fn location(&self, id: SpanId) -> &Span {
        &self.spans[id.0 as usize]
    }
    pub fn kind(&self, value: &Value) -> &ValueKind {
        self.body(value.node)
    }
    pub fn typ(&self, value: &Value) -> &TypKind {
        self.type_annotation(value.note)
    }
    pub fn span(&self, value: &Value) -> &Span {
        self.location(value.span)
    }

    pub fn alloc_phrase<T>(&mut self, phrase: Phrase<T>) -> Result<Phrase<T, SpanId>, ValueError> {
        Ok(crate::phrase!(node: phrase.node, span: self.alloc_span(phrase.span)?))
    }

    pub fn resolve_phrase<T: Clone>(&self, phrase: &Phrase<T, SpanId>) -> Phrase<T> {
        crate::phrase!(node: phrase.node.clone(), span: self.location(phrase.span).clone())
    }

    pub fn relocate(&mut self, value: Value, span: Span) -> Result<Value, ValueError> {
        Ok(Value {
            span: self.alloc_span(span)?,
            ..value
        })
    }

    pub fn annotate(&mut self, value: Value, typ: TypKind) -> Result<Value, ValueError> {
        Ok(Value {
            note: self.alloc_type(typ)?,
            ..value
        })
    }

    /// Structural comparison including value types and locations at every depth
    pub fn compare(&self, left: &Value, right: &Value) -> Ordering {
        self.compare_inner(left, right, true)
    }

    pub fn equal(&self, left: &Value, right: &Value) -> bool {
        self.compare(left, right) == Ordering::Equal
    }

    /// Structural equality ignoring all value and label annotations
    pub fn syntax_eq(&self, left: &Value, right: &Value) -> bool {
        self.compare_inner(left, right, false) == Ordering::Equal
    }

    fn compare_inner(&self, left: &Value, right: &Value, annotations: bool) -> Ordering {
        use ValueKind::*;
        let compare = |left: &Value, right: &Value| self.compare_inner(left, right, annotations);
        let compare_values = |left: &[Value], right: &[Value]| {
            for (left, right) in left.iter().zip(right) {
                let order = compare(left, right);
                if order != Ordering::Equal {
                    return order;
                }
            }
            left.len().cmp(&right.len())
        };
        let order = match (self.kind(left), self.kind(right)) {
            (Bool(left), Bool(right)) => left.cmp(right),
            (Num(left), Num(right)) => num::compare(left, right),
            (Text(left), Text(right)) => left.cmp(right),
            (Struct(left), Struct(right)) => {
                let mut order = Ordering::Equal;
                for ((atom_l, value_l), (atom_r, value_r)) in left.iter().zip(right) {
                    order = atom_l
                        .node
                        .cmp(&atom_r.node)
                        .then_with(|| compare(value_l, value_r));
                    if order != Ordering::Equal {
                        break;
                    }
                }
                order.then_with(|| left.len().cmp(&right.len()))
            }
            (Case(left), Case(right)) => left.cmp_by(right, compare),
            (Tuple(left), Tuple(right)) | (List(left), List(right)) => compare_values(left, right),
            (Opt(Some(left)), Opt(Some(right))) => compare(left, right),
            (Opt(None), Opt(None)) => Ordering::Equal,
            (Opt(None), Opt(Some(_))) => Ordering::Less,
            (Opt(Some(_)), Opt(None)) => Ordering::Greater,
            (Func(left), Func(right)) => left.node.cmp(&right.node),
            (Extern(left), Extern(right)) => left.cmp(right),
            (left, right) => left.tag().cmp(&right.tag()),
        };
        if annotations {
            order
                .then_with(|| self.typ(left).cmp(self.typ(right)))
                .then_with(|| self.span(left).cmp(self.span(right)))
        } else {
            order
        }
    }
}

// == Case shapes

thread_local! {
    static SHAPE_CACHE: RefCell<HashMap<Rc<str>, Rc<Mixop>>> = RefCell::new(HashMap::new());
}

pub(crate) fn shape(shape_text: &str) -> Rc<Mixop> {
    SHAPE_CACHE.with(|cache| {
        if let Some(mixop) = cache.borrow().get(shape_text).cloned() {
            return mixop;
        }

        let mixop = frontend::parse::parse_mixop(shape_text)
            .expect("value constructor contains a valid SpecTec mixop");
        let mixop = Rc::new(mixop);
        cache
            .borrow_mut()
            .insert(Rc::from(shape_text), Rc::clone(&mixop));
        mixop
    })
}

// == Comparison

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ValueTag {
    Bool,
    Num,
    Text,
    Struct,
    Case,
    Tuple,
    Opt,
    List,
    Func,
    Extern,
}

impl ValueKind {
    fn tag(&self) -> ValueTag {
        match self {
            Self::Bool(_) => ValueTag::Bool,
            Self::Num(_) => ValueTag::Num,
            Self::Text(_) => ValueTag::Text,
            Self::Struct(_) => ValueTag::Struct,
            Self::Case(_) => ValueTag::Case,
            Self::Tuple(_) => ValueTag::Tuple,
            Self::Opt(_) => ValueTag::Opt,
            Self::List(_) => ValueTag::List,
            Self::Func(_) => ValueTag::Func,
            Self::Extern(_) => ValueTag::Extern,
        }
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ValueError {
    #[error("{kind:?} arena index overflow")]
    IndexOverflow { kind: ArenaIndexKind },
    #[error("expected {expected:?} value, got {actual:?}")]
    UnexpectedKind {
        expected: ValueTag,
        actual: ValueTag,
    },
    #[error("value index {index} is out of bounds for length {len}")]
    IndexOutOfBounds { index: usize, len: usize },
    #[error("expected exactly {expected} values, got {actual}")]
    ExpectedCount { expected: usize, actual: usize },
}

// == Constructors

pub mod make {
    use super::*;

    macro_rules! case {
        (
            arena: $arena:expr,
            shape: $shape:expr,
            args: $args:expr,
            typ: $typ:expr,
            span: $span:expr $(,)?
        ) => {
            $crate::lang::data::value::make::case__($arena, $shape, $args, $typ, $span)
        };
    }

    pub(crate) fn case__(
        arena: &mut ValueArena,
        shape_text: &str,
        args: Vec<Value>,
        typ_name: &str,
        span: Span,
    ) -> Result<Value, ValueError> {
        let mixop = shape(shape_text);
        let value_case =
            Mixop::fill(mixop.as_ref(), args).expect("mixop arity matches its value constructor");
        let id = crate::phrase! {
            node: typ_name.to_owned(),
            span: Span::default(),
        };
        let typ = typ::make::var(id, Vec::new());
        case_(arena, &typ, value_case, span)
    }

    pub(crate) use case;

    pub fn new(
        arena: &mut ValueArena,
        kind: ValueKind,
        typ: TypKind,
        span: Span,
    ) -> Result<Value, ValueError> {
        arena.alloc(kind, typ, span)
    }

    pub fn bool(arena: &mut ValueArena, value: bool, span: Span) -> Result<Value, ValueError> {
        let kind = ValueKind::Bool(value);
        let typ = typ::make::bool().node;
        new(arena, kind, typ, span)
    }

    pub fn nat(
        arena: &mut ValueArena,
        value: num::Natural,
        span: Span,
    ) -> Result<Value, ValueError> {
        let number = Number::Nat(value);
        let kind = ValueKind::Num(number);
        let typ = typ::make::nat().node;
        new(arena, kind, typ, span)
    }

    pub fn int(
        arena: &mut ValueArena,
        value: num_bigint::BigInt,
        span: Span,
    ) -> Result<Value, ValueError> {
        let number = Number::Int(value);
        let kind = ValueKind::Num(number);
        let typ = typ::make::int().node;
        new(arena, kind, typ, span)
    }

    pub fn num(arena: &mut ValueArena, value: Number, span: Span) -> Result<Value, ValueError> {
        match value {
            Number::Nat(value) => nat(arena, value, span),
            Number::Int(value) => int(arena, value, span),
        }
    }

    pub fn text(arena: &mut ValueArena, value: String, span: Span) -> Result<Value, ValueError> {
        let kind = ValueKind::Text(value);
        let typ = typ::make::text().node;
        new(arena, kind, typ, span)
    }

    pub fn structure(
        arena: &mut ValueArena,
        typ: &Typ,
        fields: Vec<(Phrase<atom::Atom>, Value)>,
        span: Span,
    ) -> Result<Value, ValueError> {
        let fields = fields
            .into_iter()
            .map(|(atom, value)| {
                Ok((
                    crate::phrase!(node: atom.node, span: arena.alloc_span(atom.span)?),
                    value,
                ))
            })
            .collect::<Result<_, ValueError>>()?;
        let kind = ValueKind::Struct(fields);
        let typ = typ.node.clone();
        new(arena, kind, typ, span)
    }

    pub fn case_(
        arena: &mut ValueArena,
        typ: &Typ,
        value_case: Mixfix<Value>,
        span: Span,
    ) -> Result<Value, ValueError> {
        let value_case = value_case.try_map_spans(|span| arena.alloc_span(span))?;
        let kind = ValueKind::Case(value_case);
        let typ = typ.node.clone();
        new(arena, kind, typ, span)
    }

    pub fn tuple(
        arena: &mut ValueArena,
        typ: &Typ,
        values: Vec<Value>,
        span: Span,
    ) -> Result<Value, ValueError> {
        let kind = ValueKind::Tuple(values);
        let typ = typ.node.clone();
        new(arena, kind, typ, span)
    }

    pub fn opt(
        arena: &mut ValueArena,
        typ: &Typ,
        value: Option<Value>,
        span: Span,
    ) -> Result<Value, ValueError> {
        let kind = ValueKind::Opt(value);
        let typ = typ.node.clone();
        new(arena, kind, typ, span)
    }

    pub fn list(
        arena: &mut ValueArena,
        typ: &Typ,
        values: Vec<Value>,
        span: Span,
    ) -> Result<Value, ValueError> {
        let kind = ValueKind::List(values);
        let typ = typ.node.clone();
        new(arena, kind, typ, span)
    }

    pub fn func(
        arena: &mut ValueArena,
        id: Id,
        tparams: Vec<TId>,
        typs_params: Vec<Typ>,
        typ_ret: Typ,
        span: Span,
    ) -> Result<Value, ValueError> {
        let typ = typ::make::func(tparams, typs_params, typ_ret).node;
        let id = crate::phrase!(node: id.node, span: arena.alloc_span(id.span)?);
        let kind = ValueKind::Func(id);
        new(arena, kind, typ, span)
    }

    pub fn external(
        arena: &mut ValueArena,
        typ: &Typ,
        value: ExternalData,
        span: Span,
    ) -> Result<Value, ValueError> {
        let kind = ValueKind::Extern(value);
        let typ = typ.node.clone();
        new(arena, kind, typ, span)
    }
}

// == Projections

pub mod get {
    use super::*;

    macro_rules! matches {
        (
            @arms $value_case:ident;
            $shape:literal $(| $shape_alt:literal)* => |$values:ident| $body:expr,
            $($rest:tt)+
        ) => {{
            match $value_case {
                Some(value_case)
                    if [$shape, $($shape_alt),*].into_iter().any(|shape_text| {
                        let expected = $crate::lang::data::value::shape(shape_text);
                        value_case.eq_shape(expected.as_ref())
                    }) =>
                {
                    let (_, $values) = value_case.split();
                    $body
                }
                _ => $crate::lang::data::value::get::matches! {
                    @arms $value_case;
                    $($rest)+
                },
            }
        }};
        (@arms $value_case:ident; _ => $fallback:expr $(,)?) => {
            $fallback
        };
        ($arena:expr, $value:expr, $($arms:tt)+) => {{
            let value = $value;
            let value_case = match $arena.kind(value) {
                $crate::lang::data::value::ValueKind::Case(value_case) => Some(value_case),
                _ => None,
            };
            $crate::lang::data::value::get::matches! {
                @arms value_case;
                $($arms)+
            }
        }};
    }

    pub(crate) use matches;

    fn unexpected(arena: &ValueArena, value: &Value, expected: ValueTag) -> ValueError {
        ValueError::UnexpectedKind {
            expected,
            actual: arena.kind(value).tag(),
        }
    }

    pub fn bool(arena: &ValueArena, value: &Value) -> Result<bool, ValueError> {
        match arena.kind(value) {
            ValueKind::Bool(value) => Ok(*value),
            _ => Err(unexpected(arena, value, ValueTag::Bool)),
        }
    }

    pub fn num<'a>(arena: &'a ValueArena, value: &Value) -> Result<&'a Number, ValueError> {
        match arena.kind(value) {
            ValueKind::Num(value) => Ok(value),
            _ => Err(unexpected(arena, value, ValueTag::Num)),
        }
    }

    pub fn text<'a>(arena: &'a ValueArena, value: &Value) -> Result<&'a str, ValueError> {
        match arena.kind(value) {
            ValueKind::Text(value) => Ok(value),
            _ => Err(unexpected(arena, value, ValueTag::Text)),
        }
    }

    pub fn structure<'a>(
        arena: &'a ValueArena,
        value: &Value,
    ) -> Result<&'a [ValueField], ValueError> {
        match arena.kind(value) {
            ValueKind::Struct(fields) => Ok(fields),
            _ => Err(unexpected(arena, value, ValueTag::Struct)),
        }
    }

    pub fn case<'a>(arena: &'a ValueArena, value: &Value) -> Result<&'a ValueCase, ValueError> {
        match arena.kind(value) {
            ValueKind::Case(value_case) => Ok(value_case),
            _ => Err(unexpected(arena, value, ValueTag::Case)),
        }
    }

    pub fn tuple<'a>(arena: &'a ValueArena, value: &Value) -> Result<&'a [Value], ValueError> {
        match arena.kind(value) {
            ValueKind::Tuple(values) => Ok(values),
            _ => Err(unexpected(arena, value, ValueTag::Tuple)),
        }
    }

    pub fn opt<'a>(arena: &'a ValueArena, value: &Value) -> Result<Option<&'a Value>, ValueError> {
        match arena.kind(value) {
            ValueKind::Opt(value) => Ok(value.as_ref()),
            _ => Err(unexpected(arena, value, ValueTag::Opt)),
        }
    }

    pub fn list<'a>(arena: &'a ValueArena, value: &Value) -> Result<&'a [Value], ValueError> {
        match arena.kind(value) {
            ValueKind::List(values) => Ok(values),
            _ => Err(unexpected(arena, value, ValueTag::List)),
        }
    }

    pub fn func<'a>(
        arena: &'a ValueArena,
        value: &Value,
    ) -> Result<&'a Phrase<String, SpanId>, ValueError> {
        match arena.kind(value) {
            ValueKind::Func(id) => Ok(id),
            _ => Err(unexpected(arena, value, ValueTag::Func)),
        }
    }

    pub fn external<'a>(
        arena: &'a ValueArena,
        value: &Value,
    ) -> Result<&'a ExternalData, ValueError> {
        match arena.kind(value) {
            ValueKind::Extern(value) => Ok(value),
            _ => Err(unexpected(arena, value, ValueTag::Extern)),
        }
    }

    pub fn nth(values: &[Value], index: usize) -> Result<&Value, ValueError> {
        values.get(index).ok_or(ValueError::IndexOutOfBounds {
            index,
            len: values.len(),
        })
    }

    pub fn one(values: &[Value]) -> Result<&Value, ValueError> {
        match values {
            [value] => Ok(value),
            _ => Err(ValueError::ExpectedCount {
                expected: 1,
                actual: values.len(),
            }),
        }
    }

    pub fn two(values: &[Value]) -> Result<(&Value, &Value), ValueError> {
        match values {
            [value_a, value_b] => Ok((value_a, value_b)),
            _ => Err(ValueError::ExpectedCount {
                expected: 2,
                actual: values.len(),
            }),
        }
    }

    #[allow(clippy::type_complexity)]
    pub fn three(values: &[Value]) -> Result<(&Value, &Value, &Value), ValueError> {
        match values {
            [value_a, value_b, value_c] => Ok((value_a, value_b, value_c)),
            _ => Err(ValueError::ExpectedCount {
                expected: 3,
                actual: values.len(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_overflow_is_typed_for_each_arena_component() {
        let Some(overflow) = (u32::MAX as usize).checked_add(1) else {
            return;
        };
        for kind in [
            ArenaIndexKind::Value,
            ArenaIndexKind::Type,
            ArenaIndexKind::Span,
        ] {
            assert_eq!(
                ValueArena::index(overflow, kind),
                Err(ValueError::IndexOverflow { kind })
            );
            assert_eq!(ValueArena::index(overflow - 1, kind), Ok(u32::MAX));
        }
    }
}
