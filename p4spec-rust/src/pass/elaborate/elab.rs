//! Elaboration-language validation and conversion to intermediate syntax

use crate::{
    lang::{
        common::{
            Id,
            ds::map::ArityMismatch,
            notation::mixfix::Mixfix,
            source::{Span, Spanned},
        },
        el::ast as el,
        il::ast as il,
    },
    runtime::types::{
        Theta, TypeArityMismatch, TypeDef, TypeError, TypeErrorKind, expand_typ, subst_typ_case,
    },
};

use super::{ElabError, ElabErrorKind, TypeShape, attempt::Attempt, context::Context};

fn type_result<T>(result: Result<T, TypeError>) -> Attempt<T> {
    match result {
        Ok(value) => Attempt::ok(value),
        Err(error) => Attempt::fail(error.into()),
    }
}

fn elab_iter(iter: el::Iter) -> il::Iter {
    match iter {
        el::Iter::Opt => il::Iter::Opt,
        el::Iter::List => il::Iter::List,
    }
}

fn destruct_error(shape: TypeShape, span: Span) -> ElabError {
    ElabError::new(
        ElabErrorKind::CannotDestructure(shape),
        span,
        format!("cannot destruct type as {shape}"),
    )
}

fn as_text_typ(ctx: &Context, typ: &il::Typ) -> Attempt<()> {
    type_result(expand_typ(&ctx.tdenv, typ)).and_then(|typ| match typ.node {
        il::TypKind::Text => Attempt::ok(()),
        _ => Attempt::fail(destruct_error(TypeShape::Text, typ.span)),
    })
}

fn as_iter_typ(ctx: &Context, typ: &il::Typ) -> Attempt<(il::Typ, il::Iter)> {
    type_result(expand_typ(&ctx.tdenv, typ)).and_then(|typ| match typ.node {
        il::TypKind::Iter(typ, iter) => Attempt::ok((*typ, iter)),
        _ => Attempt::fail(destruct_error(TypeShape::Iteration, typ.span)),
    })
}

fn as_tuple_typ(ctx: &Context, typ: &il::Typ) -> Attempt<Vec<il::Typ>> {
    type_result(expand_typ(&ctx.tdenv, typ)).and_then(|typ| match typ.node {
        il::TypKind::Tuple(typs) => Attempt::ok(typs),
        _ => Attempt::fail(destruct_error(TypeShape::Tuple, typ.span)),
    })
}

fn as_list_typ(ctx: &Context, typ: &il::Typ) -> Attempt<il::Typ> {
    type_result(expand_typ(&ctx.tdenv, typ)).and_then(|typ| match typ.node {
        il::TypKind::Iter(typ, il::Iter::List) => Attempt::ok(*typ),
        _ => Attempt::fail(destruct_error(TypeShape::List, typ.span)),
    })
}

fn as_struct_typ(ctx: &Context, typ: &il::Typ) -> Attempt<Vec<il::TypField>> {
    type_result(expand_typ(&ctx.tdenv, typ)).and_then(|typ| {
        let il::TypKind::Var(id, _) = &typ.node else {
            return Attempt::fail(destruct_error(TypeShape::Struct, typ.span));
        };
        let Some(TypeDef::Defined(_, def_typ)) = ctx.find_typdef_opt(id) else {
            return Attempt::fail(destruct_error(TypeShape::Struct, typ.span));
        };
        match &def_typ.node {
            il::DefTypKind::Struct(fields) => Attempt::ok(fields.clone()),
            _ => Attempt::fail(destruct_error(TypeShape::Struct, typ.span)),
        }
    })
}

fn arity_error(expected: usize, actual: usize, span: Span) -> ElabError {
    let mismatch = ArityMismatch::new(expected, actual);
    let mismatch = TypeArityMismatch::TypeArgument(mismatch);
    let type_error = TypeErrorKind::ArityMismatch(mismatch);
    ElabError::new(ElabErrorKind::ArityMismatch, span, type_error.to_string())
}

fn elab_plain_typ(ctx: &Context, plain_typ: &el::PlainTyp) -> Result<il::Typ, ElabError> {
    let kind = match &plain_typ.node {
        el::PlainTypKind::Bool => il::TypKind::Bool,
        el::PlainTypKind::Num(num_typ) => il::TypKind::Num(*num_typ),
        el::PlainTypKind::Text => il::TypKind::Text,
        el::PlainTypKind::Var(id, targs) => {
            let typdef = ctx.find_typdef(id)?;
            let tparams = typdef.tparams();
            if tparams.len() != targs.len() {
                return Err(arity_error(tparams.len(), targs.len(), id.span.clone()));
            }
            let mut targs_il = Vec::with_capacity(targs.len());
            for targ in targs {
                targs_il.push(elab_plain_typ(ctx, targ)?);
            }
            il::TypKind::Var(id.clone(), targs_il)
        }
        el::PlainTypKind::Paren(plain_typ) => elab_plain_typ(ctx, plain_typ)?.node,
        el::PlainTypKind::Tuple(plain_typs) => {
            let mut typs = Vec::with_capacity(plain_typs.len());
            for plain_typ in plain_typs {
                typs.push(elab_plain_typ(ctx, plain_typ)?);
            }
            il::TypKind::Tuple(typs)
        }
        el::PlainTypKind::Iter(plain_typ, iter) => {
            let typ = elab_plain_typ(ctx, plain_typ)?;
            il::TypKind::Iter(Box::new(typ), elab_iter(*iter))
        }
    };
    Ok(Spanned::new(kind, plain_typ.span.clone()))
}

fn elab_not_typ(ctx: &Context, typ: &el::Typ) -> Result<il::NotTyp, ElabError> {
    match typ {
        el::Typ::Plain(plain_typ) => {
            let typ = elab_plain_typ(ctx, plain_typ)?;
            Ok(Spanned::new(Mixfix::Arg(typ), plain_typ.span.clone()))
        }
        el::Typ::Notation(not_typ) => {
            let mixfix = match &not_typ.node {
                el::NotTypKind::Atom(atom) => Mixfix::Atom(atom.clone()),
                el::NotTypKind::Seq(typs) => {
                    let mut items = Vec::with_capacity(typs.len());
                    for typ in typs {
                        items.push(elab_not_typ(ctx, typ)?.node);
                    }
                    Mixfix::Seq(items)
                }
                el::NotTypKind::Infix(typ_l, atom, typ_r) => Mixfix::Infix(
                    Box::new(elab_not_typ(ctx, typ_l)?.node),
                    atom.clone(),
                    Box::new(elab_not_typ(ctx, typ_r)?.node),
                ),
                el::NotTypKind::Brack(atom_l, typ, atom_r) => Mixfix::Brack(
                    atom_l.clone(),
                    Box::new(elab_not_typ(ctx, typ)?.node),
                    atom_r.clone(),
                ),
            };
            Ok(Spanned::new(mixfix, not_typ.span.clone()))
        }
    }
}

fn elab_typ_case_plain(ctx: &Context, typ: &il::Typ) -> Result<Vec<il::TypCase>, ElabError> {
    let typ = expand_typ(&ctx.tdenv, typ)?;
    let il::TypKind::Var(id, targs) = &typ.node else {
        return Err(ElabError::new(
            ElabErrorKind::InvalidTypeExtension,
            typ.span,
            "cannot extend a non-variant type",
        ));
    };
    match ctx.find_typdef(id)? {
        TypeDef::Defining(_) => Err(ElabError::new(
            ElabErrorKind::InvalidTypeExtension,
            typ.span,
            "cannot extend an incomplete type",
        )),
        TypeDef::Defined(tparams, def_typ) => {
            let il::DefTypKind::Variant(cases) = &def_typ.node else {
                return Err(ElabError::new(
                    ElabErrorKind::InvalidTypeExtension,
                    typ.span,
                    "cannot extend a non-variant type",
                ));
            };
            let theta = Theta::from_lists(tparams, targs).map_err(|mismatch| {
                arity_error(mismatch.expected, mismatch.actual, typ.span.clone())
            })?;
            cases
                .iter()
                .map(|case| subst_typ_case(&theta, case).map_err(ElabError::from))
                .collect()
        }
        TypeDef::Parameter | TypeDef::Extern => Err(ElabError::new(
            ElabErrorKind::InvalidTypeExtension,
            typ.span,
            "cannot extend a non-variant type",
        )),
    }
}

fn elab_def_typ(
    ctx: &Context,
    id: &Id,
    tparams: &[el::TParam],
    def_typ: &el::DefTyp,
) -> Result<(TypeDef, il::DefTyp), ElabError> {
    let def_typ_il = match &def_typ.node {
        el::DefTypKind::Plain(plain_typ) => {
            let typ = elab_plain_typ(ctx, plain_typ)?;
            Spanned::new(il::DefTypKind::Plain(typ), plain_typ.span.clone())
        }
        el::DefTypKind::Struct(fields) => {
            let mut fields_il = Vec::with_capacity(fields.len());
            for (atom, plain_typ, _) in fields {
                fields_il.push((atom.clone(), elab_plain_typ(ctx, plain_typ)?));
            }
            Spanned::new(il::DefTypKind::Struct(fields_il), def_typ.span.clone())
        }
        el::DefTypKind::Variant(cases) => {
            let targs = tparams
                .iter()
                .map(|tparam| {
                    Spanned::new(
                        il::TypKind::Var(tparam.clone(), vec![]),
                        tparam.span.clone(),
                    )
                })
                .collect();
            let origin = Spanned::new((id.clone(), targs), id.span.clone());
            let mut cases_il = vec![];
            for (typ, hints) in cases {
                match typ {
                    el::Typ::Plain(plain_typ) => {
                        let typ = elab_plain_typ(ctx, plain_typ)?;
                        cases_il.extend(elab_typ_case_plain(ctx, &typ)?);
                    }
                    el::Typ::Notation(_) => {
                        let not_typ = elab_not_typ(ctx, typ)?;
                        cases_il.push((not_typ, origin.clone(), hints.clone()));
                    }
                }
            }
            for (index, case) in cases_il.iter().enumerate() {
                let mixop = case.0.node.to_mixop();
                if cases_il[..index]
                    .iter()
                    .any(|case_other| case_other.0.node.to_mixop() == mixop)
                {
                    return Err(ElabError::new(
                        ElabErrorKind::AmbiguousVariant,
                        def_typ.span.clone(),
                        "variant cases are ambiguous",
                    ));
                }
            }
            Spanned::new(il::DefTypKind::Variant(cases_il), def_typ.span.clone())
        }
    };
    let typdef = TypeDef::Defined(tparams.to_vec(), Box::new(def_typ_il.clone()));
    Ok((typdef, def_typ_il))
}

#[cfg(test)]
mod tests {
    use crate::{
        lang::{
            common::{
                notation::{atom::Atom, mixfix::Mixfix},
                source::{Position, Span, Spanned},
            },
            el::ast as el,
            il::ast::{self as il, DefTypKind, TypKind},
        },
        pass::elaborate::{ElabErrorKind, EntityKind},
        runtime::types::TypeDef,
    };

    use super::{super::context::Context, elab_def_typ, elab_plain_typ};

    fn span(label: &str) -> Span {
        Span::new(Position::new(label, 4, 2), Position::new(label, 4, 6))
    }

    fn id(name: &str, label: &str) -> el::Id {
        Spanned::new(name.to_owned(), span(label))
    }

    fn plain(kind: el::PlainTypKind, label: &str) -> el::PlainTyp {
        Spanned::new(kind, span(label))
    }

    fn targ(kind: el::PlainTypKind, label: &str) -> el::Targ {
        Spanned::new(kind, span(label))
    }

    fn atom(name: &str, label: &str) -> el::Atom {
        Spanned::new(Atom::keyword(name), span(label))
    }

    #[test]
    fn plain_type_elaboration_preserves_arguments_and_source_spans() {
        let alias = id("Alias", "alias-definition");
        let tparam = id("T", "parameter");
        let alias_def = Spanned::new(
            DefTypKind::Plain(crate::runtime::types::typ::bool()),
            span("alias-body"),
        );
        let ctx = Context::new()
            .add_typdef(
                alias.clone(),
                TypeDef::Defined(vec![tparam], Box::new(alias_def)),
            )
            .expect("alias definition");
        let input_span = span("alias-use");
        let input = Spanned::new(
            el::PlainTypKind::Var(alias.clone(), vec![targ(el::PlainTypKind::Bool, "arg")]),
            input_span.clone(),
        );

        let output = elab_plain_typ(&ctx, &input).expect("elaborate type");

        assert_eq!(output.span, input_span);
        assert!(matches!(
            output.node,
            TypKind::Var(id, targs)
                if id == alias && matches!(targs.as_slice(), [arg] if arg.node == TypKind::Bool)
        ));
    }

    #[test]
    fn plain_type_elaboration_reports_lookup_and_arity_categories() {
        let missing_span = span("missing-use");
        let missing = plain(
            el::PlainTypKind::Var(id("Missing", "missing-use"), vec![]),
            "missing-use",
        );
        let error = elab_plain_typ(&Context::new(), &missing).unwrap_err();
        assert_eq!(error.kind, ElabErrorKind::Undefined(EntityKind::Type));
        assert_eq!(error.span, missing_span);

        let alias = id("Alias", "alias-definition");
        let ctx = Context::new()
            .add_typdef(alias.clone(), TypeDef::Defining(vec![id("T", "parameter")]))
            .expect("alias declaration");
        let wrong_arity = plain(el::PlainTypKind::Var(alias, vec![]), "wrong-arity");
        let error = elab_plain_typ(&ctx, &wrong_arity).unwrap_err();
        assert_eq!(error.kind, ElabErrorKind::ArityMismatch);
        assert_eq!(error.span, span("alias-definition"));
    }

    #[test]
    fn variant_extension_substitutes_arguments_and_preserves_origin() {
        let base = id("Base", "base");
        let tparam = id("T", "base-parameter");
        let not_typ = Spanned::new(
            Mixfix::Seq(vec![
                Mixfix::Atom(atom("CASE", "case")),
                Mixfix::Arg(crate::runtime::types::typ::var(tparam.clone(), vec![])),
            ]),
            span("case"),
        );
        let origin = Spanned::new(
            (
                base.clone(),
                vec![crate::runtime::types::typ::var(tparam.clone(), vec![])],
            ),
            span("base"),
        );
        let base_def = Spanned::new(
            DefTypKind::Variant(vec![(not_typ, origin, vec![])]),
            span("base-body"),
        );
        let ctx = Context::new()
            .add_typdef(
                base.clone(),
                TypeDef::Defined(vec![tparam], Box::new(base_def)),
            )
            .expect("base definition");
        let extension = Spanned::new(
            el::DefTypKind::Variant(vec![(
                el::Typ::Plain(plain(
                    el::PlainTypKind::Var(
                        base.clone(),
                        vec![targ(el::PlainTypKind::Bool, "argument")],
                    ),
                    "extension",
                )),
                vec![],
            )]),
            span("extension"),
        );

        let (_, output) =
            elab_def_typ(&ctx, &id("Derived", "derived"), &[], &extension).expect("extend variant");

        let DefTypKind::Variant(cases) = output.node else {
            panic!("variant definition")
        };
        let (not_typ, origin, _) = &cases[0];
        assert!(matches!(
            &not_typ.node,
            Mixfix::Seq(items)
                if matches!(&items[1], Mixfix::Arg(typ) if typ.node == TypKind::Bool)
        ));
        assert_eq!(origin.node.0, base);
    }

    #[test]
    fn variant_definition_rejects_duplicate_notation_shapes() {
        let case = |typ, label| {
            (
                el::Typ::Notation(Spanned::new(
                    el::NotTypKind::Seq(vec![
                        el::Typ::Notation(Spanned::new(
                            el::NotTypKind::Atom(atom("CASE", label)),
                            span(label),
                        )),
                        el::Typ::Plain(plain(typ, label)),
                    ]),
                    span(label),
                )),
                vec![],
            )
        };
        let definition = Spanned::new(
            el::DefTypKind::Variant(vec![
                case(el::PlainTypKind::Bool, "bool-case"),
                case(el::PlainTypKind::Text, "text-case"),
            ]),
            span("variant"),
        );

        let error =
            elab_def_typ(&Context::new(), &id("Choice", "choice"), &[], &definition).unwrap_err();

        assert_eq!(error.kind, ElabErrorKind::AmbiguousVariant);
        assert_eq!(error.span, span("variant"));
    }
}
