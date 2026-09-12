use num_bigint::BigInt;
use serde::{Deserialize, Serialize};

use crate::{
    lang::{
        data::value::{Value, get},
        xl::num,
    },
    runner::{Extern, ExternError, Interface, Interpreter, RunnerContext},
    util::json::json,
};

use crate::sim_plugin::spec_impl::{
    func, pack,
    rel::{self, CallResult},
    unpack,
};

use super::{
    PacketResult,
    bits::{bits_to_int_unsigned, string_to_bits},
};

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

    pub fn from_json(json: &json) -> Result<Self, ExternError> {
        let pkt: Self = serde_json::from_value(json.clone())
            .map_err(|error| ExternError::Failure(error.to_string()))?;
        pkt.check_bounds()?;
        Ok(pkt)
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
    ) -> Result<PacketResult<Self>, Interp::Error>
    where
        Iface: Interface,
        Exn: Extern,
        Interp: Interpreter<Iface, Exn>,
    {
        let value_typ = func::find_type_e_local(ctx, value_ctx, "T")?;
        let value_typ_subst = func::subst_type_e_local(ctx, value_ctx, value_typ)?;
        let size = unpack::size(&func::sizeof_max_size_in_bits(ctx, value_typ_subst)?)?;
        if !self.has_size(size)? {
            return self.reject(ctx, value_ctx, value_arch, "PacketTooShort");
        }
        let (pkt, bits) = self.parse(size)?;
        let value_hdr = func::find_var_e_local(ctx, value_ctx, "hdr")?;
        let value_hdr = func::write_value_from_bits(ctx, value_hdr, 0, &bits)?;
        let value_ctx = rel::lvalue_write_var_local(ctx, value_ctx, value_arch, "hdr", value_hdr)?;
        let value_call_result = pack::return_result(ctx.arena_mut(), None)?;
        Ok(PacketResult {
            pkt,
            result: CallResult {
                value_ctx,
                value_arch,
                value_call_result,
            },
        })
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
    ) -> Result<PacketResult<Self>, Interp::Error>
    where
        Iface: Interface,
        Exn: Extern,
        Interp: Interpreter<Iface, Exn>,
    {
        let value_typ = func::find_type_e_local(ctx, value_ctx, "T")?;
        let value_typ_subst = func::subst_type_e_local(ctx, value_ctx, value_typ)?;
        let size_min = unpack::size(&func::sizeof_min_size_in_bits(ctx, value_typ_subst)?)?;
        let size_max = unpack::size(&func::sizeof_max_size_in_bits(ctx, value_typ_subst)?)?;
        let value_size = func::find_var_e_local(ctx, value_ctx, "variableFieldSizeInBits")?;
        let value_hi = pack::p4_arbitrary_int(ctx.arena_mut(), 2.into())?;
        let value_lo = pack::p4_arbitrary_int(ctx.arena_mut(), 0.into())?;
        let value_alignment = func::bitacc_range_op(ctx, value_size, value_hi, value_lo)?;
        let alignment = unpack::size(&unpack::p4_fixed_bit(ctx.arena(), &value_alignment)?.int)?;
        let values_size = get::case(ctx.arena(), &value_size)
            .map_err(ExternError::from)?
            .args();
        let value_varsize = values_size.get(1).ok_or_else(|| {
            ExternError::from(crate::lang::data::value::ValueError::IndexOutOfBounds {
                index: 1,
                len: values_size.len(),
            })
        })?;
        let size_varsize = unpack::size(num::to_int(
            get::num(ctx.arena(), value_varsize).map_err(ExternError::from)?,
        ))?;
        let size = size_min
            .checked_add(size_varsize)
            .ok_or_else(|| ExternError::Failure("packet size overflow".to_owned()))?;
        if alignment != 0 {
            return self.reject(ctx, value_ctx, value_arch, "ParserInvalidArgument");
        }
        if !self.has_size(size)? {
            return self.reject(ctx, value_ctx, value_arch, "PacketTooShort");
        }
        if size > size_max {
            return self.reject(ctx, value_ctx, value_arch, "HeaderTooShort");
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
        let value_call_result = pack::return_result(ctx.arena_mut(), None)?;
        Ok(PacketResult {
            pkt,
            result: CallResult {
                value_ctx,
                value_arch,
                value_call_result,
            },
        })
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
    ) -> Result<PacketResult<Self>, Interp::Error>
    where
        Iface: Interface,
        Exn: Extern,
        Interp: Interpreter<Iface, Exn>,
    {
        let value_typ = func::find_type_e_local(ctx, value_ctx, "T")?;
        let value_typ_subst = func::subst_type_e_local(ctx, value_ctx, value_typ)?;
        let size = unpack::size(&func::sizeof_max_size_in_bits(ctx, value_typ_subst)?)?;
        let value_hdr = func::default(ctx, value_typ)?;
        if !self.has_size(size)? {
            return self.reject(ctx, value_ctx, value_arch, "PacketTooShort");
        }
        let (_, bits) = self.parse(size)?;
        let value_hdr = func::write_value_from_bits(ctx, value_hdr, 0, &bits)?;
        let value_call_result = pack::return_result(ctx.arena_mut(), Some(value_hdr))?;
        Ok(PacketResult {
            pkt: self.clone(),
            result: CallResult {
                value_ctx,
                value_arch,
                value_call_result,
            },
        })
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
    ) -> Result<PacketResult<Self>, Interp::Error>
    where
        Iface: Interface,
        Exn: Extern,
        Interp: Interpreter<Iface, Exn>,
    {
        let value_size = func::find_var_e_local(ctx, value_ctx, "sizeInBits")?;
        let size = unpack::size(&unpack::p4_fixed_bit(ctx.arena(), &value_size)?.int)?;
        if !self.has_size(size)? {
            return self.reject(ctx, value_ctx, value_arch, "PacketTooShort");
        }
        let pkt = Self {
            idx: self.idx + size,
            ..self.clone()
        };
        let value_call_result = pack::return_result(ctx.arena_mut(), None)?;
        Ok(PacketResult {
            pkt,
            result: CallResult {
                value_ctx,
                value_arch,
                value_call_result,
            },
        })
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
    ) -> Result<PacketResult<Self>, Interp::Error>
    where
        Iface: Interface,
        Exn: Extern,
        Interp: Interpreter<Iface, Exn>,
    {
        let value_len =
            pack::p4_fixed_bit(ctx.arena_mut(), 32.into(), self.len.div_ceil(8).into())?;
        let value_call_result = pack::return_result(ctx.arena_mut(), Some(value_len))?;
        Ok(PacketResult {
            pkt: self.clone(),
            result: CallResult {
                value_ctx,
                value_arch,
                value_call_result,
            },
        })
    }

    fn reject<Interp, Iface, Exn>(
        &self,
        ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
        value_ctx: Value,
        value_arch: Value,
        name: &str,
    ) -> Result<PacketResult<Self>, Interp::Error>
    where
        Iface: Interface,
        Exn: Extern,
        Interp: Interpreter<Iface, Exn>,
    {
        let value_call_result = pack::reject_transition(ctx.arena_mut(), name)?;
        Ok(PacketResult {
            pkt: self.clone(),
            result: CallResult {
                value_ctx,
                value_arch,
                value_call_result,
            },
        })
    }
}
