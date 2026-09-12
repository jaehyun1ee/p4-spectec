//! Match-action table interface

use crate::{
    lang::{
        common::source::Span,
        data::{
            typ,
            value::{Value, ValueArena, ValueError, get, make},
        },
    },
    runner::{Extern, ExternError, Interface, Interpreter, RunnerContext},
};

use super::spec_impl::func;

struct TableName {
    value_unqualified: Value,
    value_qualified: Option<Value>,
}

fn table_name(arena: &mut ValueArena, value_name: Value) -> Result<TableName, ExternError> {
    let name = get::text(arena, &value_name)?.to_owned();
    let names: Vec<_> = name.split('.').collect();
    let value_unqualified = make::text(
        arena,
        names
            .last()
            .expect("split always has a segment")
            .to_string(),
        Span::default(),
    )?;
    let value_qualified = if names.len() == 1 {
        None
    } else {
        let values_name = names
            .into_iter()
            .map(|name| make::text(arena, name.to_owned(), Span::default()))
            .collect::<Result<Vec<_>, _>>()?;
        let typ_id = typ::make::list(typ::make::var(
            crate::phrase!(node: "nameIR".to_owned(), span: Span::default()),
            Vec::new(),
        ));
        Some(make::list(
            arena,
            typ_id.node.into(),
            values_name,
            Span::default(),
        )?)
    };
    Ok(TableName {
        value_unqualified,
        value_qualified,
    })
}

pub fn find_table<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_arch: Value,
    value_name: Value,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let name = table_name(ctx.arena_mut(), value_name)?;
    if let Some(value_id) = name.value_qualified
        && let Some(value_table) = func::find_object_qualified_e(ctx, value_arch, value_id)?
    {
        return Ok(value_table);
    }
    func::find_object_unqualified_e(ctx, value_arch, name.value_unqualified)?
        .ok_or_else(|| ExternError::Failure("table not found".to_owned()).into())
}

pub fn update_table<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_arch: Value,
    value_name: Value,
    value_table: Value,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let name = table_name(ctx.arena_mut(), value_name)?;
    if let Some(value_id) = name.value_qualified
        && func::find_object_qualified_e(ctx, value_arch, value_id)?.is_some()
    {
        return func::update_object_qualified_e(ctx, value_arch, value_id, value_table);
    }
    func::update_object_unqualified_e(ctx, value_arch, name.value_unqualified, value_table)
}

pub fn add_entry<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_ctx: Value,
    value_arch: Value,
    value_name: Value,
    value_priority: Value,
    value_keys: Value,
    value_action: Value,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    // Lookup table object
    let value_table = find_table(ctx, value_arch, value_name)?;
    // Add entry to table object
    let value_table = match func::table_object_add_entry(
        ctx,
        value_ctx,
        value_table,
        value_priority,
        value_keys,
        value_action,
    )? {
        Some(value_table) => value_table,
        None => {
            // Replace keyset names with table key names, assuming fields are in order
            let keys = func::key_interface_of_table_object(ctx, value_table)?;
            let mut values_name = Vec::new();
            for key in keys {
                if get::text(ctx.arena(), &key.value_match_kind).map_err(ExternError::from)?
                    != "selector"
                {
                    values_name.push(key.value_name);
                }
            }
            let values_key = get::list(ctx.arena(), &value_keys)
                .map_err(ExternError::from)?
                .iter()
                .map(|value_key| get::tuple(ctx.arena(), value_key))
                .collect::<Result<Vec<_>, ValueError>>()
                .map_err(ExternError::from)?;
            let values_key = values_key
                .into_iter()
                .map(|values| get::nth(values, 1).copied())
                .collect::<Result<Vec<_>, ValueError>>()
                .map_err(ExternError::from)?;
            if values_name.len() != values_key.len() {
                return Err(ExternError::Value(ValueError::ExpectedCount {
                    expected: values_name.len(),
                    actual: values_key.len(),
                })
                .into());
            }
            let typ_key = typ::make::var(
                crate::phrase!(node: "tableKeyInterface".to_owned(), span: Span::default()),
                Vec::new(),
            );
            let values_key = values_name
                .into_iter()
                .zip(values_key)
                .map(|(value_name, value_key)| {
                    make::tuple(
                        ctx.arena_mut(),
                        typ_key.node.clone().into(),
                        vec![value_name, value_key],
                        Span::default(),
                    )
                })
                .collect::<Result<Vec<_>, _>>()
                .map_err(ExternError::from)?;
            let value_keys = make::list(
                ctx.arena_mut(),
                typ::make::list(typ_key).node.into(),
                values_key,
                Span::default(),
            )
            .map_err(ExternError::from)?;
            func::table_object_add_entry(
                ctx,
                value_ctx,
                value_table,
                value_priority,
                value_keys,
                value_action,
            )?
            .ok_or_else(|| ExternError::Failure("table entry rejected".to_owned()))?
        }
    };
    // Update arch with modified table object
    update_table(ctx, value_arch, value_name, value_table)
}

pub fn add_default_action<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_ctx: Value,
    value_arch: Value,
    value_name: Value,
    value_action: Value,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    // Lookup table object
    let value_table = find_table(ctx, value_arch, value_name)?;
    let value_table =
        func::table_object_add_default_action(ctx, value_ctx, value_table, value_action)?;
    // Update arch with modified table object
    update_table(ctx, value_arch, value_name, value_table)
}
