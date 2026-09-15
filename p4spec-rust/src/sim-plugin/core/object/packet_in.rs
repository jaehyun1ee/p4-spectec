use num_bigint::BigInt;
use num_traits::ToPrimitive;
use serde::{Deserialize, Serialize};

use crate::{
    lang::{
        common::source::Span,
        data::{
            typ,
            value::{Value, get, make},
        },
        xl::num,
    },
    runner::{Extern, ExternError, Interface, Interpreter, RunnerContext},
};

use crate::sim_plugin::spec_impl::{func, pack, rel, unpack};

use super::bits::{bits_to_int_unsigned, string_to_bits};

/// Input packet data and its extraction cursor
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PacketIn {
    pub bits: Vec<bool>,
    pub idx: usize,
    pub len: usize,
}

impl PacketIn {
    pub fn init(text: &str) -> Result<Self, ExternError> {
        let bits = string_to_bits(text)?;
        let len = bits.len();
        Ok(Self { bits, idx: 0, len })
    }

    pub fn reset(&mut self) {
        self.idx = 0;
    }

    fn check_bounds(&self) -> Result<(), ExternError> {
        if self.idx > self.len || self.len > self.bits.len() {
            return Err(ExternError::Failure(
                "invalid packet cursor or length".to_owned(),
            ));
        }
        Ok(())
    }

    fn has_size(&self, size: usize) -> Result<bool, ExternError> {
        self.check_bounds()?;
        Ok(size <= self.len - self.idx)
    }

    pub fn parse(&self, size: usize) -> Result<(Self, Vec<bool>), ExternError> {
        if !self.has_size(size)? {
            return Err(ExternError::Failure(
                "packet parse exceeds available bits".to_owned(),
            ));
        }
        let bits = self.bits[self.idx..self.idx + size].to_vec();
        let pkt = Self {
            idx: self.idx + size,
            ..self.clone()
        };
        Ok((pkt, bits))
    }

    pub fn payload(&self) -> Result<&[bool], ExternError> {
        self.check_bounds()?;
        Ok(&self.bits[self.idx..self.len])
    }

    pub fn payload_bytes(&self) -> Result<Vec<BigInt>, ExternError> {
        Ok(self
            .payload()?
            .chunks_exact(8)
            .map(bits_to_int_unsigned)
            .collect())
    }

    /// Reads a fixed-size header into `hdr` and advances the packet cursor
    ///
    /// `T` must be a fixed-size header type. Extraction may trigger
    /// `PacketTooShort` or `StackOutOfBounds`:
    /// ```text
    /// void extract<T>(out T hdr);
    /// ```
    pub fn extract<Interp, Iface, Exn>(
        &self,
        ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
        value_ctx: Value,
        value_arch: Value,
    ) -> Result<(Self, Value, Value, Value), Interp::Error>
    where
        Iface: Interface,
        Exn: Extern,
        Interp: Interpreter<Iface, Exn>,
    {
        let value_typ = func::find_type_e_local(ctx, value_ctx, "T")?;
        let value_typ_subst = func::subst_type_e_local(ctx, value_ctx, value_typ)?;
        let size = (func::sizeof_max_size_in_bits(ctx, value_typ_subst)?)
            .to_usize()
            .ok_or_else(|| ExternError::Failure("invalid packet size".to_owned()))?;
        if !self.has_size(size)? {
            let value_name = make::text(
                ctx.arena_mut(),
                "PacketTooShort".to_owned(),
                Span::default(),
            )
            .map_err(ExternError::from)?;
            let value_err = make::case_shaped! {
                arena: ctx.arena_mut(),
                shape: "ERROR '.' nameIR",
                args: vec![value_name],
                typ: "errorValue",
                span: Span::default(),
            }
            .map_err(ExternError::from)?;
            let value_call_result = make::case_shaped! {
                arena: ctx.arena_mut(),
                shape: "REJECT errorValue",
                args: vec![value_err],
                typ: "rejectTransitionResult",
                span: Span::default(),
            }
            .map_err(ExternError::from)?;
            return Ok((self.clone(), value_ctx, value_arch, value_call_result));
        }
        let (pkt, bits) = self.parse(size)?;
        let value_hdr = func::find_var_e_local(ctx, value_ctx, "hdr")?;
        let value_hdr = func::write_value_from_bits(ctx, value_hdr, 0, &bits)?;
        let value_ctx = rel::lvalue_write_var_local(ctx, value_ctx, value_arch, "hdr", value_hdr)?;
        let typ = typ::make::opt(typ::make::var(
            crate::phrase!(node: "value".to_owned(), span: Span::default()),
            Vec::new(),
        ));
        let value_opt = make::opt(ctx.arena_mut(), typ.node.into(), None, Span::default())
            .map_err(ExternError::from)?;
        let value_call_result = make::case_shaped! {
            arena: ctx.arena_mut(),
            shape: "RETURN value?",
            args: vec![value_opt],
            typ: "returnResult",
            span: Span::default(),
        }
        .map_err(ExternError::from)?;
        Ok((pkt, value_ctx, value_arch, value_call_result))
    }

    /// Extracts a header with a variable-size field
    ///
    /// ```text
    /// void extract<T>(out T variableSizeHeader,
    ///                 in bit<32> variableFieldSizeInBits);
    /// ```
    pub fn extract_varsize<Interp, Iface, Exn>(
        &self,
        ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
        value_ctx: Value,
        value_arch: Value,
    ) -> Result<(Self, Value, Value, Value), Interp::Error>
    where
        Iface: Interface,
        Exn: Extern,
        Interp: Interpreter<Iface, Exn>,
    {
        let value_typ = func::find_type_e_local(ctx, value_ctx, "T")?;
        let value_typ_subst = func::subst_type_e_local(ctx, value_ctx, value_typ)?;
        let size_min = (func::sizeof_min_size_in_bits(ctx, value_typ_subst)?)
            .to_usize()
            .ok_or_else(|| ExternError::Failure("invalid packet size".to_owned()))?;
        let size_max = (func::sizeof_max_size_in_bits(ctx, value_typ_subst)?)
            .to_usize()
            .ok_or_else(|| ExternError::Failure("invalid packet size".to_owned()))?;
        let value_size = func::find_var_e_local(ctx, value_ctx, "variableFieldSizeInBits")?;
        let value_hi = pack::p4_arbitrary_int(ctx.arena_mut(), 2.into())?;
        let value_lo = pack::p4_arbitrary_int(ctx.arena_mut(), 0.into())?;
        let value_alignment = func::bitacc_range_op(ctx, value_size, value_hi, value_lo)?;
        let alignment = (unpack::p4_fixed_bit(ctx.arena(), &value_alignment)?.1)
            .to_usize()
            .ok_or_else(|| ExternError::Failure("invalid packet size".to_owned()))?;
        let values_size = get::case(ctx.arena(), &value_size)
            .map_err(ExternError::from)?
            .args();
        let value_varsize = values_size.get(1).ok_or_else(|| {
            ExternError::from(crate::lang::data::value::ValueError::IndexOutOfBounds {
                index: 1,
                len: values_size.len(),
            })
        })?;
        let size_varsize =
            (num::to_int(get::num(ctx.arena(), value_varsize).map_err(ExternError::from)?))
                .to_usize()
                .ok_or_else(|| ExternError::Failure("invalid packet size".to_owned()))?;
        let size = size_min
            .checked_add(size_varsize)
            .ok_or_else(|| ExternError::Failure("packet size overflow".to_owned()))?;
        if alignment != 0 {
            let value_name = make::text(
                ctx.arena_mut(),
                "ParserInvalidArgument".to_owned(),
                Span::default(),
            )
            .map_err(ExternError::from)?;
            let value_err = make::case_shaped! {
                arena: ctx.arena_mut(),
                shape: "ERROR '.' nameIR",
                args: vec![value_name],
                typ: "errorValue",
                span: Span::default(),
            }
            .map_err(ExternError::from)?;
            let value_call_result = make::case_shaped! {
                arena: ctx.arena_mut(),
                shape: "REJECT errorValue",
                args: vec![value_err],
                typ: "rejectTransitionResult",
                span: Span::default(),
            }
            .map_err(ExternError::from)?;
            return Ok((self.clone(), value_ctx, value_arch, value_call_result));
        }
        if !self.has_size(size)? {
            let value_name = make::text(
                ctx.arena_mut(),
                "PacketTooShort".to_owned(),
                Span::default(),
            )
            .map_err(ExternError::from)?;
            let value_err = make::case_shaped! {
                arena: ctx.arena_mut(),
                shape: "ERROR '.' nameIR",
                args: vec![value_name],
                typ: "errorValue",
                span: Span::default(),
            }
            .map_err(ExternError::from)?;
            let value_call_result = make::case_shaped! {
                arena: ctx.arena_mut(),
                shape: "REJECT errorValue",
                args: vec![value_err],
                typ: "rejectTransitionResult",
                span: Span::default(),
            }
            .map_err(ExternError::from)?;
            return Ok((self.clone(), value_ctx, value_arch, value_call_result));
        }
        if size > size_max {
            let value_name = make::text(
                ctx.arena_mut(),
                "HeaderTooShort".to_owned(),
                Span::default(),
            )
            .map_err(ExternError::from)?;
            let value_err = make::case_shaped! {
                arena: ctx.arena_mut(),
                shape: "ERROR '.' nameIR",
                args: vec![value_name],
                typ: "errorValue",
                span: Span::default(),
            }
            .map_err(ExternError::from)?;
            let value_call_result = make::case_shaped! {
                arena: ctx.arena_mut(),
                shape: "REJECT errorValue",
                args: vec![value_err],
                typ: "rejectTransitionResult",
                span: Span::default(),
            }
            .map_err(ExternError::from)?;
            return Ok((self.clone(), value_ctx, value_arch, value_call_result));
        }
        let (pkt, bits) = self.parse(size)?;
        let value_hdr = func::find_var_e_local(ctx, value_ctx, "variableSizeHeader")?;
        let value_hdr = func::write_value_from_bits(ctx, value_hdr, size_varsize, &bits)?;
        let value_ctx = rel::lvalue_write_var_local(
            ctx,
            value_ctx,
            value_arch,
            "variableSizeHeader",
            value_hdr,
        )?;
        let typ = typ::make::opt(typ::make::var(
            crate::phrase!(node: "value".to_owned(), span: Span::default()),
            Vec::new(),
        ));
        let value_opt = make::opt(ctx.arena_mut(), typ.node.into(), None, Span::default())
            .map_err(ExternError::from)?;
        let value_call_result = make::case_shaped! {
            arena: ctx.arena_mut(),
            shape: "RETURN value?",
            args: vec![value_opt],
            typ: "returnResult",
            span: Span::default(),
        }
        .map_err(ExternError::from)?;
        Ok((pkt, value_ctx, value_arch, value_call_result))
    }

    /// Reads a value without advancing the packet cursor
    ///
    /// ```text
    /// T lookahead<T>();
    /// ```
    pub fn lookahead<Interp, Iface, Exn>(
        &self,
        ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
        value_ctx: Value,
        value_arch: Value,
    ) -> Result<(Self, Value, Value, Value), Interp::Error>
    where
        Iface: Interface,
        Exn: Extern,
        Interp: Interpreter<Iface, Exn>,
    {
        let value_typ = func::find_type_e_local(ctx, value_ctx, "T")?;
        let value_typ_subst = func::subst_type_e_local(ctx, value_ctx, value_typ)?;
        let size = (func::sizeof_max_size_in_bits(ctx, value_typ_subst)?)
            .to_usize()
            .ok_or_else(|| ExternError::Failure("invalid packet size".to_owned()))?;
        let value_hdr = func::default(ctx, value_typ)?;
        if !self.has_size(size)? {
            let value_name = make::text(
                ctx.arena_mut(),
                "PacketTooShort".to_owned(),
                Span::default(),
            )
            .map_err(ExternError::from)?;
            let value_err = make::case_shaped! {
                arena: ctx.arena_mut(),
                shape: "ERROR '.' nameIR",
                args: vec![value_name],
                typ: "errorValue",
                span: Span::default(),
            }
            .map_err(ExternError::from)?;
            let value_call_result = make::case_shaped! {
                arena: ctx.arena_mut(),
                shape: "REJECT errorValue",
                args: vec![value_err],
                typ: "rejectTransitionResult",
                span: Span::default(),
            }
            .map_err(ExternError::from)?;
            return Ok((self.clone(), value_ctx, value_arch, value_call_result));
        }
        let bits = &self.bits[self.idx..self.idx + size];
        let value_hdr = func::write_value_from_bits(ctx, value_hdr, 0, bits)?;
        let typ = typ::make::opt(typ::make::var(
            crate::phrase!(node: "value".to_owned(), span: Span::default()),
            Vec::new(),
        ));
        let value_opt = make::opt(
            ctx.arena_mut(),
            typ.node.into(),
            Some(value_hdr),
            Span::default(),
        )
        .map_err(ExternError::from)?;
        let value_call_result = make::case_shaped! {
            arena: ctx.arena_mut(),
            shape: "RETURN value?",
            args: vec![value_opt],
            typ: "returnResult",
            span: Span::default(),
        }
        .map_err(ExternError::from)?;
        Ok((self.clone(), value_ctx, value_arch, value_call_result))
    }

    /// Advances the packet cursor by the requested number of bits
    ///
    /// ```text
    /// void advance(in bit<32> sizeInBits);
    /// ```
    pub fn advance<Interp, Iface, Exn>(
        &self,
        ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
        value_ctx: Value,
        value_arch: Value,
    ) -> Result<(Self, Value, Value, Value), Interp::Error>
    where
        Iface: Interface,
        Exn: Extern,
        Interp: Interpreter<Iface, Exn>,
    {
        let value_size = func::find_var_e_local(ctx, value_ctx, "sizeInBits")?;
        let size = (unpack::p4_fixed_bit(ctx.arena(), &value_size)?.1)
            .to_usize()
            .ok_or_else(|| ExternError::Failure("invalid packet size".to_owned()))?;
        if !self.has_size(size)? {
            let value_name = make::text(
                ctx.arena_mut(),
                "PacketTooShort".to_owned(),
                Span::default(),
            )
            .map_err(ExternError::from)?;
            let value_err = make::case_shaped! {
                arena: ctx.arena_mut(),
                shape: "ERROR '.' nameIR",
                args: vec![value_name],
                typ: "errorValue",
                span: Span::default(),
            }
            .map_err(ExternError::from)?;
            let value_call_result = make::case_shaped! {
                arena: ctx.arena_mut(),
                shape: "REJECT errorValue",
                args: vec![value_err],
                typ: "rejectTransitionResult",
                span: Span::default(),
            }
            .map_err(ExternError::from)?;
            return Ok((self.clone(), value_ctx, value_arch, value_call_result));
        }
        let pkt = Self {
            idx: self.idx + size,
            ..self.clone()
        };
        let typ = typ::make::opt(typ::make::var(
            crate::phrase!(node: "value".to_owned(), span: Span::default()),
            Vec::new(),
        ));
        let value_opt = make::opt(ctx.arena_mut(), typ.node.into(), None, Span::default())
            .map_err(ExternError::from)?;
        let value_call_result = make::case_shaped! {
            arena: ctx.arena_mut(),
            shape: "RETURN value?",
            args: vec![value_opt],
            typ: "returnResult",
            span: Span::default(),
        }
        .map_err(ExternError::from)?;
        Ok((pkt, value_ctx, value_arch, value_call_result))
    }

    /// Returns the total packet length in bytes
    ///
    /// ```text
    /// bit<32> length();
    /// ```
    pub fn length<Interp, Iface, Exn>(
        &self,
        ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
        value_ctx: Value,
        value_arch: Value,
    ) -> Result<(Self, Value, Value, Value), Interp::Error>
    where
        Iface: Interface,
        Exn: Extern,
        Interp: Interpreter<Iface, Exn>,
    {
        let value_len =
            pack::p4_fixed_bit(ctx.arena_mut(), 32.into(), self.len.div_ceil(8).into())?;
        let typ = typ::make::opt(typ::make::var(
            crate::phrase!(node: "value".to_owned(), span: Span::default()),
            Vec::new(),
        ));
        let value_opt = make::opt(
            ctx.arena_mut(),
            typ.node.into(),
            Some(value_len),
            Span::default(),
        )
        .map_err(ExternError::from)?;
        let value_call_result = make::case_shaped! {
            arena: ctx.arena_mut(),
            shape: "RETURN value?",
            args: vec![value_opt],
            typ: "returnResult",
            span: Span::default(),
        }
        .map_err(ExternError::from)?;
        Ok((self.clone(), value_ctx, value_arch, value_call_result))
    }
}
