use crate::{
    lang::{
        common::source::Span,
        data::value::{Value, get, make},
    },
    runner::{Extern, ExternError, Interface, Interpreter, RunnerContext},
};

pub fn find_var_value_t<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_cursor: &Value,
    value_ctx: &Value,
    name: &str,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let value_name = make::text(ctx.arena_mut(), name.to_owned(), Span::default())
        .map_err(ExternError::from)
        .map_err(Interp::Error::from)?;
    let value_name = make::case_shaped_(
        ctx.arena_mut(),
        "_BARE nameIR",
        vec![value_name],
        "prefixedNameIR",
        Span::default(),
    )
    .map_err(ExternError::from)
    .map_err(Interp::Error::from)?;
    ctx.call_func(
        "find_var_value_t",
        &[],
        &[value_name, *value_cursor, *value_ctx],
    )
}

pub fn find_var_value_t_local<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_ctx: &Value,
    name: &str,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let value_cursor = make::case_shaped_(
        ctx.arena_mut(),
        "LOCAL",
        Vec::new(),
        "cursor",
        Span::default(),
    )
    .map_err(ExternError::from)
    .map_err(Interp::Error::from)?;
    find_var_value_t(ctx, &value_cursor, value_ctx, name)
}

pub(crate) fn local_cursor(
    arena: &mut crate::lang::data::value::ValueArena,
) -> Result<Value, ExternError> {
    Ok(make::case_shaped_(
        arena,
        "LOCAL",
        Vec::new(),
        "cursor",
        Span::default(),
    )?)
}

pub(crate) fn bare_name(
    arena: &mut crate::lang::data::value::ValueArena,
    name: &str,
) -> Result<Value, ExternError> {
    let value_name = make::text(arena, name.to_owned(), Span::default())?;
    Ok(make::case_shaped_(
        arena,
        "_BARE nameIR",
        vec![value_name],
        "prefixedNameIR",
        Span::default(),
    )?)
}

pub fn find_var_e_local<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_ctx: Value,
    name: &str,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let value_cursor = local_cursor(ctx.arena_mut())?;
    let value_name = bare_name(ctx.arena_mut(), name)?;
    ctx.call_func("find_var_e", &[], &[value_name, value_cursor, value_ctx])
}

pub fn find_type_e_local<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_ctx: Value,
    name: &str,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let value_cursor = local_cursor(ctx.arena_mut())?;
    let value_name =
        make::text(ctx.arena_mut(), name.to_owned(), Span::default()).map_err(ExternError::from)?;
    let value_opt = ctx.call_func("find_type_e", &[], &[value_cursor, value_ctx, value_name])?;
    get::opt(ctx.arena(), &value_opt)
        .map_err(ExternError::from)?
        .ok_or_else(|| ExternError::Failure(format!("type not found: {name}")).into())
}

pub fn subst_type_e_local<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_ctx: Value,
    value_typ: Value,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let value_cursor = local_cursor(ctx.arena_mut())?;
    ctx.call_func("subst_type_e", &[], &[value_cursor, value_ctx, value_typ])
}

pub fn default<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_typ: Value,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    ctx.call_func("default", &[], &[value_typ])
}

pub fn write_bits_from_value<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_source: Value,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    ctx.call_func("write_bits_from_value", &[], &[value_source])
}

pub fn bitacc_range_op<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_base: Value,
    value_hi: Value,
    value_lo: Value,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    ctx.call_func("bitacc_range_op", &[], &[value_base, value_hi, value_lo])
}

pub fn sizeof_min_size_in_bits<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_typ: Value,
) -> Result<num_bigint::BigInt, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let value_size = ctx.call_func("sizeof_minSizeInBits'", &[], &[value_typ])?;
    Ok(
        crate::lang::xl::num::to_int(
            get::num(ctx.arena(), &value_size).map_err(ExternError::from)?,
        )
        .clone(),
    )
}

pub fn sizeof_max_size_in_bits<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_typ: Value,
) -> Result<num_bigint::BigInt, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let value_size = ctx.call_func("sizeof_maxSizeInBits'", &[], &[value_typ])?;
    Ok(
        crate::lang::xl::num::to_int(
            get::num(ctx.arena(), &value_size).map_err(ExternError::from)?,
        )
        .clone(),
    )
}

pub fn write_value_from_bits<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_target: Value,
    size_varsize: usize,
    bits: &[bool],
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let value_varsize = make::nat(
        ctx.arena_mut(),
        crate::lang::xl::num::Natural::try_from(num_bigint::BigInt::from(size_varsize))
            .expect("packet size is nonnegative"),
        Span::default(),
    )
    .map_err(ExternError::from)?;
    let values_bits = bits
        .iter()
        .map(|bit| make::bool(ctx.arena_mut(), *bit, Span::default()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(ExternError::from)?;
    let typ_bits = crate::lang::data::typ::make::list(crate::lang::data::typ::make::var(
        crate::phrase!(node: "bit".to_owned(), span: Span::default()),
        Vec::new(),
    ));
    let value_bits = make::list(
        ctx.arena_mut(),
        typ_bits.node.into(),
        values_bits,
        Span::default(),
    )
    .map_err(ExternError::from)?;
    ctx.call_func(
        "write_value_from_bits",
        &[],
        &[value_target, value_varsize, value_bits],
    )
}
