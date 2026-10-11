//! Resolve AL syntax to slots and interned mixops
//!
//! The `Prepare` implementations rebuild source nodes as prepared syntax,
//! reserving slots in the callable's frame layout.
//! Extern and builtin definitions have no body and pass through unchanged.

use std::rc::Rc;

use crate::lang::data::notation::MixopArena;

use crate::lang::al::{ast as source, prepared::*};

use crate::runtime::envs::interp::shared::{callable::Callable, frame::FrameLayout};

use crate::interp::shared::prepare::{Prepare, il};

// - Preparation context

/// Records AL slots and interned mixops.
pub struct PrepareContext<'a> {
    /// The specification's notation arena.
    pub arena_mixop: &'a mut MixopArena,
    /// The callable's frame layout.
    pub layout: &'a mut FrameLayout,
}

impl PrepareContext<'_> {
    fn il(&mut self) -> il::PrepareContext<'_> {
        il::PrepareContext { arena_mixop: &mut *self.arena_mixop, layout: &mut *self.layout }
    }

    /// Prepares a definition and collects its frame layout.
    pub fn prepare_callable<T, S>(arena_mixop: &mut MixopArena, source: S) -> Callable<T>
    where
        S: for<'a> Prepare<PrepareContext<'a>, Output = T>,
    {
        let mut layout = FrameLayout::default();
        let mut ctx = PrepareContext { arena_mixop, layout: &mut layout };
        let def = source.prepare(&mut ctx);
        Callable { def, layout: Rc::new(layout) }
    }
}

// - Premises

impl Prepare<PrepareContext<'_>> for source::PremKind {
    type Output = PremKind;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        match self {
            source::PremKind::Rule(prem_inner) => PremKind::Rule(prem_inner.prepare(ctx)),
            source::PremKind::If(prem_inner) => PremKind::If(prem_inner.prepare(ctx)),
            source::PremKind::IfHold(prem_inner) => PremKind::IfHold(prem_inner.prepare(ctx)),
            source::PremKind::IfNotHold(prem_inner) => PremKind::IfNotHold(prem_inner.prepare(ctx)),
            source::PremKind::Let(prem_inner) => PremKind::Let(prem_inner.prepare(ctx)),
            source::PremKind::Iter(prem_inner) => PremKind::Iter(prem_inner.prepare(ctx)),
            source::PremKind::Debug(prem_inner) => PremKind::Debug(prem_inner.prepare(ctx)),
        }
    }
}

// - Rule premise

impl Prepare<PrepareContext<'_>> for source::RulePrem {
    type Output = RulePrem;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        RulePrem {
            id: self.id,
            not_exp: self.not_exp.prepare(&mut ctx.il()),
            input_hint: self.input_hint,
        }
    }
}

// - If premise

impl Prepare<PrepareContext<'_>> for source::IfPrem {
    type Output = IfPrem;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        IfPrem { exp: self.exp.prepare(&mut ctx.il()) }
    }
}

// - If-hold premise

impl Prepare<PrepareContext<'_>> for source::IfHoldPrem {
    type Output = IfHoldPrem;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        IfHoldPrem { id: self.id, not_exp: self.not_exp.prepare(&mut ctx.il()) }
    }
}

// - If-not-hold premise

impl Prepare<PrepareContext<'_>> for source::IfNotHoldPrem {
    type Output = IfNotHoldPrem;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        IfNotHoldPrem { id: self.id, not_exp: self.not_exp.prepare(&mut ctx.il()) }
    }
}

// - Let premise

impl Prepare<PrepareContext<'_>> for source::LetPrem {
    type Output = LetPrem;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        LetPrem {
            exp_l: self.exp_l.prepare(&mut ctx.il()),
            exp_r: self.exp_r.prepare(&mut ctx.il()),
        }
    }
}

// - Iterated premise

impl Prepare<PrepareContext<'_>> for source::IterPrem {
    type Output = IterPrem;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        IterPrem { prem: self.prem.prepare(ctx), prem_iter: self.prem_iter.prepare(&mut ctx.il()) }
    }
}

// - Debug premise

impl Prepare<PrepareContext<'_>> for source::DebugPrem {
    type Output = DebugPrem;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        DebugPrem { exp: self.exp.prepare(&mut ctx.il()) }
    }
}

// - Rules

// - Rule group

impl Prepare<PrepareContext<'_>> for source::RuleGroupKind {
    type Output = RuleGroupKind;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        RuleGroupKind {
            id: self.id,
            rule_match: self.rule_match.prepare(ctx),
            rule_paths: self.rule_paths.prepare(ctx),
        }
    }
}

// - Else group

impl Prepare<PrepareContext<'_>> for source::ElseGroupKind {
    type Output = ElseGroupKind;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        ElseGroupKind {
            id: self.id,
            rule_match: self.rule_match.prepare(ctx),
            rule_path: self.rule_path.prepare(ctx),
        }
    }
}

// - Rule match

impl Prepare<PrepareContext<'_>> for source::RuleMatch {
    type Output = RuleMatch;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        RuleMatch {
            exps_signature: self.exps_signature.prepare(&mut ctx.il()),
            exps_input: self.exps_input.prepare(&mut ctx.il()),
            prems: self.prems.prepare(ctx),
        }
    }
}

// - Rule path

impl Prepare<PrepareContext<'_>> for source::RulePath {
    type Output = RulePath;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        RulePath {
            id: self.id,
            prems: self.prems.prepare(ctx),
            exps_output: self.exps_output.prepare(&mut ctx.il()),
        }
    }
}

// - Clauses

impl Prepare<PrepareContext<'_>> for source::ClauseKind {
    type Output = ClauseKind;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        ClauseKind {
            args: self.args.prepare(&mut ctx.il()),
            exp: self.exp.prepare(&mut ctx.il()),
            prems: self.prems.prepare(ctx),
        }
    }
}

// - Table rows

impl Prepare<PrepareContext<'_>> for source::TableRowKind {
    type Output = TableRowKind;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        TableRowKind {
            exps_signature: self.exps_signature.prepare(&mut ctx.il()),
            args: self.args.prepare(&mut ctx.il()),
            exp: self.exp.prepare(&mut ctx.il()),
            prems: self.prems.prepare(ctx),
        }
    }
}

// == Relation definitions

// - Relation definition

impl Prepare<PrepareContext<'_>> for source::RelDef {
    type Output = RelDef;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        match self {
            source::RelDef::Extern(rel_inner) => RelDef::Extern(rel_inner),
            source::RelDef::Defined(rel_inner) => RelDef::Defined(rel_inner.prepare(ctx)),
        }
    }
}

// - Defined relation definition

impl Prepare<PrepareContext<'_>> for source::DefinedRel {
    type Output = DefinedRel;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        DefinedRel {
            id: self.id,
            not_typ: self.not_typ,
            input_hint: self.input_hint,
            rule_groups: self.rule_groups.prepare(ctx),
            else_group: self.else_group.prepare(ctx),
            hints: self.hints,
        }
    }
}

// == Meta-function definitions

// - Meta-function definition

impl Prepare<PrepareContext<'_>> for source::MetaFuncDef {
    type Output = MetaFuncDef;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        match self {
            source::MetaFuncDef::Extern(func_inner) => MetaFuncDef::Extern(func_inner),
            source::MetaFuncDef::Builtin(func_inner) => MetaFuncDef::Builtin(func_inner),
            source::MetaFuncDef::Table(func_inner) => MetaFuncDef::Table(func_inner.prepare(ctx)),
            source::MetaFuncDef::Defined(func_inner) => {
                MetaFuncDef::Defined(func_inner.prepare(ctx))
            }
        }
    }
}

// - Table function definition

impl Prepare<PrepareContext<'_>> for source::TableFunc {
    type Output = TableFunc;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        TableFunc {
            id: self.id,
            params: self.params,
            typ: self.typ,
            table_rows: self.table_rows.prepare(ctx),
            hints: self.hints,
        }
    }
}

// - Defined function definition

impl Prepare<PrepareContext<'_>> for source::DefinedFunc {
    type Output = DefinedFunc;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        DefinedFunc {
            id: self.id,
            tparams: self.tparams,
            params: self.params,
            typ: self.typ,
            clauses: self.clauses.prepare(ctx),
            else_clause: self.else_clause.prepare(ctx),
            hints: self.hints,
        }
    }
}

// == Definitions

impl Prepare<PrepareContext<'_>> for source::DefKind {
    type Output = DefKind;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        match self {
            source::DefKind::Typ(typdef) => DefKind::Typ(typdef),
            source::DefKind::Var(def_var) => DefKind::Var(def_var),
            source::DefKind::Rel(rel_inner) => DefKind::Rel(rel_inner.prepare(ctx)),
            source::DefKind::MetaFunc(func_inner) => DefKind::MetaFunc(func_inner.prepare(ctx)),
        }
    }
}
