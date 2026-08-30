//! Explicit guards for partial algorithmic expressions

use crate::{
    lang::{
        al,
        il::ast,
        traits::{eq::SyntaxEq, free::Free},
        xl,
    },
    note_phrase, phrase,
};

#[derive(Default)]
struct Collected {
    must: Vec<ast::Prem>,
    insert: Vec<ast::Prem>,
}

impl Collected {
    fn compose(mut self, other: Self) -> Self {
        self.must.extend(other.must);
        self.insert.extend(other.insert);
        self
    }
}

#[derive(Clone, Copy)]
enum ClassKind {
    Equals,
    Equiv,
}

#[allow(clippy::large_enum_variant)]
enum Class {
    Equals(Vec<ast::Exp>),
    Equiv(Vec<ast::Exp>),
    Singleton(ast::Exp),
}

impl Class {
    fn conditions(&self, kind: ClassKind) -> Option<&[ast::Exp]> {
        match (kind, self) {
            (ClassKind::Equals, Self::Equals(conditions))
            | (ClassKind::Equiv, Self::Equiv(conditions)) => Some(conditions),
            _ => None,
        }
    }
}

#[derive(Default)]
struct EquivalenceTable {
    classes: Vec<Class>,
}

impl EquivalenceTable {
    fn from_prems(prems: &[ast::Prem]) -> Self {
        let mut table = Self::default();
        for prem in prems {
            if let ast::PremKind::If(if_prem) = &prem.node {
                table.add_if_exp(&if_prem.exp);
            }
        }
        table
    }

    fn add_if_exp(&mut self, exp: &ast::Exp) {
        match &exp.node {
            ast::ExpKind::Cmp(ast::CmpOp::Bool(xl::bool::CmpOp::Eq), _, exp_l, exp_r) => {
                self.union(ClassKind::Equals, exp_l, exp_r)
            }
            ast::ExpKind::Bin(ast::BinOp::Bool(xl::bool::BinOp::Equiv), _, exp_l, exp_r) => {
                self.union(ClassKind::Equiv, exp_l, exp_r)
            }
            ast::ExpKind::Bin(ast::BinOp::Bool(xl::bool::BinOp::And), _, exp_l, exp_r) => {
                self.add_if_exp(exp_l);
                self.add_if_exp(exp_r);
            }
            _ => self.classes.insert(0, Class::Singleton(exp.clone())),
        }
    }

    fn find(&self, kind: ClassKind, condition: &ast::Exp) -> Option<usize> {
        self.classes.iter().position(|class| {
            class.conditions(kind).is_some_and(|conditions| {
                conditions
                    .iter()
                    .any(|candidate| condition.syntax_eq(candidate))
            })
        })
    }

    fn wrap(kind: ClassKind, conditions: Vec<ast::Exp>) -> Class {
        match kind {
            ClassKind::Equals => Class::Equals(conditions),
            ClassKind::Equiv => Class::Equiv(conditions),
        }
    }

    fn union(&mut self, kind: ClassKind, condition_a: &ast::Exp, condition_b: &ast::Exp) {
        let index_a = self.find(kind, condition_a);
        let index_b = self.find(kind, condition_b);
        match (index_a, index_b) {
            (Some(index_a), Some(index_b)) if index_a == index_b => {}
            (Some(index_a), Some(index_b)) => {
                let Some(conditions_a) = self
                    .classes
                    .get(index_a)
                    .and_then(|class| class.conditions(kind))
                    .map(<[ast::Exp]>::to_vec)
                else {
                    return;
                };
                let Some(conditions_b) = self
                    .classes
                    .get(index_b)
                    .and_then(|class| class.conditions(kind))
                    .map(<[ast::Exp]>::to_vec)
                else {
                    return;
                };
                let mut conditions = conditions_a;
                conditions.extend(conditions_b);
                let index_high = index_a.max(index_b);
                let index_low = index_a.min(index_b);
                self.classes.remove(index_high);
                self.classes.remove(index_low);
                self.classes.insert(0, Self::wrap(kind, conditions));
            }
            (Some(index), None) | (None, Some(index)) => {
                let condition = if index_a.is_some() {
                    condition_b
                } else {
                    condition_a
                };
                let Some(mut conditions) = self
                    .classes
                    .get(index)
                    .and_then(|class| class.conditions(kind))
                    .map(<[ast::Exp]>::to_vec)
                else {
                    return;
                };
                conditions.insert(0, condition.clone());
                self.classes.remove(index);
                self.classes.insert(0, Self::wrap(kind, conditions));
            }
            (None, None) => self.classes.insert(
                0,
                Self::wrap(kind, vec![condition_a.clone(), condition_b.clone()]),
            ),
        }
    }

    fn contains(&self, kind: ClassKind, condition_a: &ast::Exp, condition_b: &ast::Exp) -> bool {
        condition_a.syntax_eq(condition_b)
            || self.find(kind, condition_a).is_some_and(|index| {
                self.classes
                    .get(index)
                    .and_then(|class| class.conditions(kind))
                    .is_some_and(|conditions| {
                        conditions
                            .iter()
                            .any(|condition| condition.syntax_eq(condition_b))
                    })
            })
    }

    fn implies_exp(&self, exp: &ast::Exp) -> bool {
        match &exp.node {
            ast::ExpKind::Cmp(ast::CmpOp::Bool(xl::bool::CmpOp::Eq), _, exp_l, exp_r) => {
                self.contains(ClassKind::Equals, exp_l, exp_r)
            }
            ast::ExpKind::Bin(ast::BinOp::Bool(xl::bool::BinOp::Equiv), _, exp_l, exp_r) => {
                self.contains(ClassKind::Equiv, exp_l, exp_r)
            }
            ast::ExpKind::Bin(ast::BinOp::Bool(xl::bool::BinOp::And), _, exp_l, exp_r) => {
                self.implies_exp(exp_l) && self.implies_exp(exp_r)
            }
            _ => self.classes.iter().any(
                |class| matches!(class, Class::Singleton(condition) if exp.syntax_eq(condition)),
            ),
        }
    }

    fn implies(&self, prem: &ast::Prem) -> bool {
        matches!(&prem.node, ast::PremKind::If(if_prem) if self.implies_exp(&if_prem.exp))
    }
}

fn filter_insert(must: &[ast::Prem], insert: Vec<ast::Prem>) -> Vec<ast::Prem> {
    let table = EquivalenceTable::from_prems(must);
    insert
        .into_iter()
        .filter(|prem| {
            !table.implies(prem) && !must.iter().any(|prem_must| prem.syntax_eq(prem_must))
        })
        .collect()
}

fn iterate_prem(iter: ast::Iter, vars: &[ast::Var], prem: ast::Prem) -> Option<ast::Prem> {
    let frees = prem.free();
    let vars_bound = vars
        .iter()
        .filter(|var| frees.contains(&var.id))
        .cloned()
        .collect::<Vec<_>>();
    if vars_bound.is_empty() {
        return None;
    }
    let span = prem.span.clone();
    let iter_prem = ast::IterPrem {
        iter,
        vars_bound,
        vars_bind: vec![],
    };
    let prem = ast::PremKind::Iter(ast::IteratedPrem {
        prem: Box::new(prem),
        iter_prem,
    });
    Some(phrase!(node: prem, span: span))
}

fn iterate_prems(iter: ast::Iter, vars: &[ast::Var], prems: Vec<ast::Prem>) -> Vec<ast::Prem> {
    prems
        .into_iter()
        .filter_map(|prem| iterate_prem(iter, vars, prem))
        .collect()
}

fn iterate_collected(
    iter: ast::Iter,
    vars_must: &[ast::Var],
    vars_insert: &[ast::Var],
    collected: Collected,
) -> Collected {
    Collected {
        must: iterate_prems(iter, vars_must, collected.must),
        insert: iterate_prems(iter, vars_insert, collected.insert),
    }
}

fn gen_index_guard(exp: &ast::Exp, exp_base: &ast::Exp, exp_index: &ast::Exp) -> Vec<ast::Prem> {
    let span = exp.span.clone();
    let exp_len = note_phrase! {
        node: ast::ExpKind::Len(Box::new(exp_base.clone())),
        note: ast::TypKind::Num(xl::num::Typ::Nat),
        span: span.clone(),
    };
    let exp_if = note_phrase! {
        node: ast::ExpKind::Cmp(
            ast::CmpOp::Num(xl::num::CmpOp::Lt),
            ast::OpTyp::Bool,
            Box::new(exp_index.clone()),
            Box::new(exp_len),
        ),
        note: ast::TypKind::Bool,
        span: span.clone(),
    };
    vec![phrase!(node: ast::PremKind::If(ast::IfPrem { exp: exp_if }), span: span)]
}

fn gen_eq_epsilon_exp(iter: ast::Iter, var: &ast::Var) -> ast::Exp {
    let mut var = var.clone();
    var.iters.push(iter);
    let exp = al::var::as_exp(true, &var);
    let span = exp.span.clone();
    let note = exp.note.clone();
    let exp_epsilon = note_phrase! {
        node: ast::ExpKind::Opt(None),
        note: note,
        span: span.clone(),
    };
    note_phrase! {
        node: ast::ExpKind::Cmp(
            ast::CmpOp::Bool(xl::bool::CmpOp::Eq),
            ast::OpTyp::Bool,
            Box::new(exp),
            Box::new(exp_epsilon),
        ),
        note: ast::TypKind::Bool,
        span: span,
    }
}

fn gen_len_exp(iter: ast::Iter, var: &ast::Var) -> ast::Exp {
    let mut var = var.clone();
    var.iters.push(iter);
    let exp = al::var::as_exp(true, &var);
    let span = exp.span.clone();
    note_phrase! {
        node: ast::ExpKind::Len(Box::new(exp)),
        note: ast::TypKind::Num(xl::num::Typ::Nat),
        span: span,
    }
}

fn pair_exp(iter: ast::Iter, exp_l: ast::Exp, exp_r: ast::Exp) -> ast::Exp {
    let span = crate::lang::common::source::Span::over(&[exp_l.span.clone(), exp_r.span.clone()]);
    let kind = match iter {
        ast::Iter::Opt => ast::ExpKind::Bin(
            ast::BinOp::Bool(xl::bool::BinOp::Equiv),
            ast::OpTyp::Bool,
            Box::new(exp_l),
            Box::new(exp_r),
        ),
        ast::Iter::List => ast::ExpKind::Cmp(
            ast::CmpOp::Bool(xl::bool::CmpOp::Eq),
            ast::OpTyp::Bool,
            Box::new(exp_l),
            Box::new(exp_r),
        ),
    };
    note_phrase!(node: kind, note: ast::TypKind::Bool, span: span)
}

fn and_exp(exp_l: ast::Exp, exp_r: ast::Exp) -> ast::Exp {
    let span = crate::lang::common::source::Span::over(&[exp_l.span.clone(), exp_r.span.clone()]);
    note_phrase! {
        node: ast::ExpKind::Bin(
            ast::BinOp::Bool(xl::bool::BinOp::And),
            ast::OpTyp::Bool,
            Box::new(exp_l),
            Box::new(exp_r),
        ),
        note: ast::TypKind::Bool,
        span: span,
    }
}

fn gen_iter_guard(iter_exp: &ast::IterExp) -> Vec<ast::Prem> {
    let (iter, vars) = iter_exp;
    if vars.len() < 2 {
        return vec![];
    }
    let exps = vars
        .iter()
        .map(|var| match iter {
            ast::Iter::Opt => gen_eq_epsilon_exp(*iter, var),
            ast::Iter::List => gen_len_exp(*iter, var),
        })
        .collect::<Vec<_>>();
    let mut exp_previous = exps[1].clone();
    let mut exp_if = pair_exp(*iter, exps[0].clone(), exp_previous.clone());
    for exp in exps.into_iter().skip(2) {
        let exp_pair = pair_exp(*iter, exp_previous, exp.clone());
        exp_if = and_exp(exp_if, exp_pair);
        exp_previous = exp;
    }
    let span = exp_if.span.clone();
    vec![phrase!(node: ast::PremKind::If(ast::IfPrem { exp: exp_if }), span: span)]
}

fn compose_inserts(mut inserts: Vec<ast::Prem>, other: Vec<ast::Prem>) -> Vec<ast::Prem> {
    inserts.extend(other);
    inserts
}

fn collect_exp(exp: &ast::Exp) -> Vec<ast::Prem> {
    match &exp.node {
        ast::ExpKind::Bool(_)
        | ast::ExpKind::Num(_)
        | ast::ExpKind::Text(_)
        | ast::ExpKind::Var(_) => vec![],
        ast::ExpKind::Un(_, _, exp)
        | ast::ExpKind::UpCast(_, exp)
        | ast::ExpKind::DownCast(_, exp)
        | ast::ExpKind::Sub(exp, _, _)
        | ast::ExpKind::Match(exp, _)
        | ast::ExpKind::Len(exp)
        | ast::ExpKind::Dot(exp, _) => collect_exp(exp),
        ast::ExpKind::Bin(_, _, exp_l, exp_r)
        | ast::ExpKind::Cmp(_, _, exp_l, exp_r)
        | ast::ExpKind::Cons(exp_l, exp_r)
        | ast::ExpKind::Cat(exp_l, exp_r)
        | ast::ExpKind::Mem(exp_l, exp_r) => {
            compose_inserts(collect_exp(exp_l), collect_exp(exp_r))
        }
        ast::ExpKind::Tuple(exps) | ast::ExpKind::List(exps) => collect_exps(exps.iter()),
        ast::ExpKind::Case(not_exp) => collect_exps(not_exp.args()),
        ast::ExpKind::Str(fields) => collect_exps(fields.iter().map(|(_, exp)| exp)),
        ast::ExpKind::Opt(Some(exp)) => collect_exp(exp),
        ast::ExpKind::Opt(None) => vec![],
        ast::ExpKind::Idx(exp_base, exp_index) => {
            let inserts = compose_inserts(collect_exp(exp_base), collect_exp(exp_index));
            compose_inserts(inserts, gen_index_guard(exp, exp_base, exp_index))
        }
        ast::ExpKind::Slice(exp_base, exp_l, exp_h) => {
            let inserts = compose_inserts(collect_exp(exp_base), collect_exp(exp_l));
            compose_inserts(inserts, collect_exp(exp_h))
        }
        ast::ExpKind::Upd(exp_base, path, exp_field) => {
            let inserts = compose_inserts(collect_exp(exp_base), collect_path(path));
            compose_inserts(inserts, collect_exp(exp_field))
        }
        ast::ExpKind::Call(_, _, args) => collect_args(args),
        ast::ExpKind::Iter(exp_inner, iter_exp) => {
            let inserts = iterate_prems(iter_exp.0, &iter_exp.1, collect_exp(exp_inner));
            compose_inserts(inserts, gen_iter_guard(iter_exp))
        }
    }
}

fn collect_exps<'a>(exps: impl IntoIterator<Item = &'a ast::Exp>) -> Vec<ast::Prem> {
    exps.into_iter().fold(Vec::new(), |inserts, exp| {
        compose_inserts(inserts, collect_exp(exp))
    })
}

fn collect_path(path: &ast::Path) -> Vec<ast::Prem> {
    match &path.node {
        ast::PathKind::Root => vec![],
        ast::PathKind::Idx(path, exp) => compose_inserts(collect_path(path), collect_exp(exp)),
        ast::PathKind::Slice(path, exp_l, exp_h) => {
            let inserts = compose_inserts(collect_path(path), collect_exp(exp_l));
            compose_inserts(inserts, collect_exp(exp_h))
        }
        ast::PathKind::Dot(path, _) => collect_path(path),
    }
}

fn collect_arg(arg: &ast::Arg) -> Vec<ast::Prem> {
    match &arg.node {
        ast::ArgKind::Exp(exp) => collect_exp(exp),
        ast::ArgKind::Def(_) => vec![],
    }
}

fn collect_args(args: &[ast::Arg]) -> Vec<ast::Prem> {
    args.iter().fold(Vec::new(), |inserts, arg| {
        compose_inserts(inserts, collect_arg(arg))
    })
}

fn collect_prem(prem: &ast::Prem) -> Collected {
    match &prem.node {
        ast::PremKind::Let(let_prem) => Collected {
            must: collect_exp(&let_prem.exp_l),
            insert: collect_exp(&let_prem.exp_r),
        },
        ast::PremKind::Iter(iterated) => {
            let iter_prem = &iterated.iter_prem;
            let mut vars_must = iter_prem.vars_bound.clone();
            vars_must.extend(iter_prem.vars_bind.clone());
            let collected = collect_prem(&iterated.prem);
            let collected =
                iterate_collected(iter_prem.iter, &vars_must, &iter_prem.vars_bound, collected);
            collected.compose(collect_iter_prem(iter_prem))
        }
        ast::PremKind::Rule(rule_prem) => Collected {
            must: vec![],
            insert: collect_exps(rule_prem.not_exp.args()),
        },
        ast::PremKind::If(if_prem) => Collected {
            must: vec![],
            insert: collect_exp(&if_prem.exp),
        },
        ast::PremKind::IfHold(if_prem) => Collected {
            must: vec![],
            insert: collect_exps(if_prem.not_exp.args()),
        },
        ast::PremKind::IfNotHold(if_prem) => Collected {
            must: vec![],
            insert: collect_exps(if_prem.not_exp.args()),
        },
        ast::PremKind::Debug(debug_prem) => Collected {
            must: vec![],
            insert: collect_exp(&debug_prem.exp),
        },
    }
}

fn collect_iter_prem(iter_prem: &ast::IterPrem) -> Collected {
    let mut vars_must = iter_prem.vars_bound.clone();
    vars_must.extend(iter_prem.vars_bind.clone());
    Collected {
        must: gen_iter_guard(&(iter_prem.iter, vars_must)),
        insert: gen_iter_guard(&(iter_prem.iter, iter_prem.vars_bound.clone())),
    }
}

fn insert_prem(prems_must_prev: &[ast::Prem], prem: ast::Prem) -> Collected {
    let collected = collect_prem(&prem);
    let mut prems_insert = filter_insert(prems_must_prev, collected.insert);
    prems_insert.push(prem);
    let mut prems_must = prems_must_prev.to_vec();
    prems_must.extend(collected.must);
    prems_must.extend(prems_insert.clone());
    Collected {
        must: prems_must,
        insert: prems_insert,
    }
}

fn insert_prems(prems_must_prev: Vec<ast::Prem>, prems: Vec<ast::Prem>) -> Collected {
    let mut prems_must = prems_must_prev;
    let mut prems_insert = Vec::new();
    for prem in prems {
        let collected = insert_prem(&prems_must, prem);
        prems_must = collected.must;
        prems_insert.extend(collected.insert);
    }
    Collected {
        must: prems_must,
        insert: prems_insert,
    }
}

fn insert_rule_group(mut rule_group: al::ast::RuleGroup) -> al::ast::RuleGroup {
    let mut prems_must = collect_exps(rule_group.node.rule_match.exps_input.iter());
    let collected = insert_prems(
        prems_must,
        std::mem::take(&mut rule_group.node.rule_match.prems),
    );
    prems_must = collected.must;
    rule_group.node.rule_match.prems = collected.insert;
    for rule_path in &mut rule_group.node.rule_paths {
        let collected = insert_prems(prems_must.clone(), std::mem::take(&mut rule_path.prems));
        let prems_output = collect_exps(rule_path.exps_output.iter());
        rule_path.prems = collected.insert;
        rule_path
            .prems
            .extend(filter_insert(&collected.must, prems_output));
    }
    rule_group
}

fn insert_else_group(mut else_group: al::ast::ElseGroup) -> al::ast::ElseGroup {
    let prems_must = collect_exps(else_group.node.rule_match.exps_input.iter());
    let collected = insert_prems(
        prems_must,
        std::mem::take(&mut else_group.node.rule_match.prems),
    );
    else_group.node.rule_match.prems = collected.insert;
    let collected = insert_prems(
        collected.must,
        std::mem::take(&mut else_group.node.rule_path.prems),
    );
    let prems_output = collect_exps(else_group.node.rule_path.exps_output.iter());
    else_group.node.rule_path.prems = collected.insert;
    else_group
        .node
        .rule_path
        .prems
        .extend(filter_insert(&collected.must, prems_output));
    else_group
}

fn insert_clause(mut clause: al::ast::Clause) -> al::ast::Clause {
    let prems_must = collect_args(&clause.node.args);
    let collected = insert_prems(prems_must, std::mem::take(&mut clause.node.premises));
    let prems_output = collect_exp(&clause.node.expression);
    clause.node.premises = collected.insert;
    clause
        .node
        .premises
        .extend(filter_insert(&collected.must, prems_output));
    clause
}

fn insert_def(mut def: al::ast::Def) -> al::ast::Def {
    match &mut def.node {
        al::ast::DefKind::Rel(relation) => {
            relation.rule_groups = std::mem::take(&mut relation.rule_groups)
                .into_iter()
                .map(insert_rule_group)
                .collect();
            relation.else_group = relation.else_group.take().map(insert_else_group);
        }
        al::ast::DefKind::FuncDec(function) => {
            function.clauses = std::mem::take(&mut function.clauses)
                .into_iter()
                .map(insert_clause)
                .collect();
            function.else_clause = function.else_clause.take().map(insert_clause);
        }
        _ => {}
    }
    def
}

/// Inserts side-condition guards throughout an analyzed specification.
pub(in crate::pass::algo) fn insert_spec(spec: al::ast::Spec) -> al::ast::Spec {
    spec.into_iter().map(insert_def).collect()
}
