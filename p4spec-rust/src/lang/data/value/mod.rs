//! Shared value types, arena storage, constructors, and projections

use std::rc::Rc;

mod arena;
mod intern;
#[allow(
    clippy::module_inception,
    reason = "separate facade and implementation"
)]
mod value;

pub use arena::ValueArena;
pub use intern::{CanonEq, CanonHash, CanonId, CanonInterner, Interned, Interner, RcInterner};
pub use value::*;

use crate::{
    lang::{
        common::{
            Id, TId,
            notation::{
                mixfix::Mixfix,
                mixop::{Mixop, shape},
            },
            source::Span,
        },
        data::typ::{self, Typ, TypKind},
        xl::num::{self, Number},
    },
    yojson::ExternalData,
};

// = Smart constructors

pub mod make {
    use super::*;

    // - General

    pub fn new(
        arena: &mut ValueArena,
        kind: ValueKind,
        typ: Rc<TypKind>,
        span: Span,
    ) -> Result<Value, ValueError> {
        arena.alloc(kind, typ, span)
    }

    // - Primitives

    pub fn bool(arena: &mut ValueArena, value: bool, span: Span) -> Result<Value, ValueError> {
        new(arena, ValueKind::Bool(value), arena.typ_bool.clone(), span)
    }

    pub fn num(arena: &mut ValueArena, value: Number, span: Span) -> Result<Value, ValueError> {
        let typ = match num::to_typ(&value) {
            num::Typ::Nat => arena.typ_nat.clone(),
            num::Typ::Int => arena.typ_int.clone(),
        };
        new(arena, ValueKind::Num(value), typ, span)
    }

    pub fn nat(
        arena: &mut ValueArena,
        value: num::Natural,
        span: Span,
    ) -> Result<Value, ValueError> {
        num(arena, Number::Nat(value), span)
    }

    pub fn int(
        arena: &mut ValueArena,
        value: num_bigint::BigInt,
        span: Span,
    ) -> Result<Value, ValueError> {
        num(arena, Number::Int(value), span)
    }

    pub fn text(arena: &mut ValueArena, value: String, span: Span) -> Result<Value, ValueError> {
        new(arena, ValueKind::Text(value), arena.typ_text.clone(), span)
    }

    // - Structures

    pub fn structure(
        arena: &mut ValueArena,
        typ: Rc<TypKind>,
        value_fields: Vec<ValueField>,
        span: Span,
    ) -> Result<Value, ValueError> {
        new(arena, ValueKind::Struct(value_fields), typ, span)
    }

    // - Cases

    pub fn case(
        arena: &mut ValueArena,
        typ: Rc<TypKind>,
        value_case: Mixfix<Value>,
        span: Span,
    ) -> Result<Value, ValueError> {
        new(arena, ValueKind::Case(value_case), typ, span)
    }

    macro_rules! case_shaped {
        (
            arena: $arena:expr,
            shape: $shape:expr,
            args: $args:expr,
            typ: $typ:expr,
            span: $span:expr $(,)?
        ) => {{
            let (shape, args, typ, span) = ($shape, $args, $typ, $span);
            $crate::lang::data::value::make::case_shaped_($arena, shape, args, typ, span)
        }};
    }

    pub(crate) fn case_shaped_(
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
        case(arena, Rc::new(typ.node), value_case, span)
    }

    pub(crate) use case_shaped;

    // - Sequences

    pub fn tuple(
        arena: &mut ValueArena,
        typ: Rc<TypKind>,
        values: Vec<Value>,
        span: Span,
    ) -> Result<Value, ValueError> {
        new(arena, ValueKind::Tuple(values), typ, span)
    }

    pub fn opt(
        arena: &mut ValueArena,
        typ: Rc<TypKind>,
        value: Option<Value>,
        span: Span,
    ) -> Result<Value, ValueError> {
        new(arena, ValueKind::Opt(value), typ, span)
    }

    pub fn list(
        arena: &mut ValueArena,
        typ: Rc<TypKind>,
        values: Vec<Value>,
        span: Span,
    ) -> Result<Value, ValueError> {
        new(arena, ValueKind::List(values), typ, span)
    }

    // - Functions

    pub fn func(
        arena: &mut ValueArena,
        id: Id,
        tparams: Vec<TId>,
        typs_params: Vec<Typ>,
        typ_ret: Typ,
        span: Span,
    ) -> Result<Value, ValueError> {
        let typ = typ::make::func(tparams, typs_params, typ_ret).node;
        new(arena, ValueKind::Func(id), Rc::new(typ), span)
    }

    // - Externals

    pub fn external(
        arena: &mut ValueArena,
        typ: Rc<TypKind>,
        value: ExternalData,
        span: Span,
    ) -> Result<Value, ValueError> {
        new(arena, ValueKind::Extern(value), typ, span)
    }
}

// = Projections

pub mod get {
    use super::*;

    // - Errors

    fn unexpected(arena: &ValueArena, value: &Value, expected: ValueTag) -> ValueError {
        ValueError::UnexpectedKind {
            expected,
            actual: arena.kind(value).tag(),
        }
    }

    // - Primitives

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

    // - Structures

    pub fn structure<'a>(
        arena: &'a ValueArena,
        value: &Value,
    ) -> Result<&'a [ValueField], ValueError> {
        match arena.kind(value) {
            ValueKind::Struct(value_fields) => Ok(value_fields),
            _ => Err(unexpected(arena, value, ValueTag::Struct)),
        }
    }

    // - Cases

    pub fn case<'a>(arena: &'a ValueArena, value: &Value) -> Result<&'a ValueCase, ValueError> {
        match arena.kind(value) {
            ValueKind::Case(value_case) => Ok(value_case),
            _ => Err(unexpected(arena, value, ValueTag::Case)),
        }
    }

    macro_rules! matches {
        (
            @arms $value_case:ident;
            $shape:literal $(| $shape_alt:literal)* => |$values:ident| $body:expr,
            $($rest:tt)+
        ) => {{
            match $value_case {
                Some(value_case)
                    if [$shape, $($shape_alt),*].into_iter().any(|shape_text| {
                        let expected = $crate::lang::common::notation::mixop::shape(shape_text);
                        value_case.eq_shape(expected.as_ref())
                    }) =>
                {
                    let $values = value_case.args();
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
            let arena = $arena;
            let value_case = match arena.kind(value) {
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

    // - Sequences

    pub fn tuple<'a>(arena: &'a ValueArena, value: &Value) -> Result<&'a [Value], ValueError> {
        match arena.kind(value) {
            ValueKind::Tuple(values) => Ok(values),
            _ => Err(unexpected(arena, value, ValueTag::Tuple)),
        }
    }

    pub fn opt(arena: &ValueArena, value: &Value) -> Result<Option<Value>, ValueError> {
        match arena.kind(value) {
            ValueKind::Opt(value) => Ok(*value),
            _ => Err(unexpected(arena, value, ValueTag::Opt)),
        }
    }

    pub fn list<'a>(arena: &'a ValueArena, value: &Value) -> Result<&'a [Value], ValueError> {
        match arena.kind(value) {
            ValueKind::List(values) => Ok(values),
            _ => Err(unexpected(arena, value, ValueTag::List)),
        }
    }

    // - Functions

    pub fn func<'a>(arena: &'a ValueArena, value: &Value) -> Result<&'a Id, ValueError> {
        match arena.kind(value) {
            ValueKind::Func(id) => Ok(id),
            _ => Err(unexpected(arena, value, ValueTag::Func)),
        }
    }

    // - Externals

    pub fn external<'a>(
        arena: &'a ValueArena,
        value: &Value,
    ) -> Result<&'a ExternalData, ValueError> {
        match arena.kind(value) {
            ValueKind::Extern(value) => Ok(value),
            _ => Err(unexpected(arena, value, ValueTag::Extern)),
        }
    }

    // - Indexing

    pub fn nth(values: &[Value], index: usize) -> Result<&Value, ValueError> {
        values.get(index).ok_or(ValueError::IndexOutOfBounds {
            index,
            len: values.len(),
        })
    }

    // - Arity

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
