//! Explicit guard insertion for partial algorithmic expressions:
//!
//! - `a[n]` requires `n < |a|`
//! - `e*{x <- x*, y <- y*, z <- z*}` requires
//!   `(|x*| = |y*|) /\ (|y*| = |z*|)`
//! - `e?{x <- x?, y <- y?}` requires `(x? = eps) <=> (y? = eps)`
//!
//! Binding sites also produce must-premises. For example,
//! `(let (x, y) = z){x -> x*, y -> y*, z <- z*}` establishes the list-length
//! equalities above. Guards entailed by these earlier premises are omitted.

use crate::{
    lang::{
        al::{self, ast},
        common::source::Span,
        traits::{eq::SyntaxEq, free::Free},
        xl,
    },
    note_phrase, phrase,
};

// == Equivalence filtering

// - Equivalence classes

#[derive(Clone, Copy)]
enum ClassKind {
    Equals,
    Equiv,
}

#[allow(clippy::large_enum_variant)]
enum Class<'a> {
    Equals(Vec<&'a ast::Exp>),
    Equiv(Vec<&'a ast::Exp>),
    Singleton(&'a ast::Exp),
}

impl<'a> Class<'a> {
    fn conditions(&self, kind: ClassKind) -> Option<&[&'a ast::Exp]> {
        match (kind, self) {
            (ClassKind::Equals, Self::Equals(conditions))
            | (ClassKind::Equiv, Self::Equiv(conditions)) => Some(conditions),
            _ => None,
        }
    }

    fn new(kind: ClassKind, conditions: Vec<&'a ast::Exp>) -> Self {
        match kind {
            ClassKind::Equals => Self::Equals(conditions),
            ClassKind::Equiv => Self::Equiv(conditions),
        }
    }
}

// - Equivalence table

#[derive(Default)]
struct EquivalenceTable<'a> {
    classes: Vec<Class<'a>>,
}

impl<'a> EquivalenceTable<'a> {
    fn from_prems(prems_al: impl IntoIterator<Item = &'a ast::Prem>) -> Self {
        let mut table = Self::default();
        for prem_al in prems_al {
            table.add_prem(prem_al);
        }
        table
    }

    fn add_prem(&mut self, prem_al: &'a ast::Prem) {
        if let ast::PremKind::If(if_prem) = &prem_al.node {
            self.add_if_exp(&if_prem.exp);
        }
    }

    fn add_if_exp(&mut self, exp: &'a ast::Exp) {
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
            _ => self.classes.insert(0, Class::Singleton(exp)),
        }
    }

    fn find(&self, kind: ClassKind, condition: &ast::Exp) -> Option<usize> {
        self.classes.iter().position(|class| {
            let Some(conditions) = class.conditions(kind) else {
                return false;
            };
            conditions
                .iter()
                .any(|candidate| condition.syntax_eq(candidate))
        })
    }

    fn take_conditions(&mut self, kind: ClassKind, index: usize) -> Vec<&'a ast::Exp> {
        let class = self.classes.remove(index);
        match (kind, class) {
            (ClassKind::Equals, Class::Equals(conditions))
            | (ClassKind::Equiv, Class::Equiv(conditions)) => conditions,
            _ => unreachable!("class lookup preserves the equivalence kind"),
        }
    }

    fn union(&mut self, kind: ClassKind, condition_a: &'a ast::Exp, condition_b: &'a ast::Exp) {
        let index_a = self.find(kind, condition_a);
        let index_b = self.find(kind, condition_b);
        match (index_a, index_b) {
            (Some(index_a), Some(index_b)) if index_a == index_b => {}
            (Some(index_a), Some(index_b)) => {
                let index_high = index_a.max(index_b);
                let index_low = index_a.min(index_b);
                let conditions_high = self.take_conditions(kind, index_high);
                let conditions_low = self.take_conditions(kind, index_low);
                let (mut conditions_a, mut conditions_b) = if index_a == index_low {
                    (conditions_low, conditions_high)
                } else {
                    (conditions_high, conditions_low)
                };
                conditions_a.append(&mut conditions_b);
                let class = Class::new(kind, conditions_a);
                self.classes.insert(0, class);
            }
            (Some(index), None) | (None, Some(index)) => {
                let condition_new = if index_a.is_some() {
                    condition_b
                } else {
                    condition_a
                };
                let mut conditions = self.take_conditions(kind, index);
                conditions.insert(0, condition_new);
                let class = Class::new(kind, conditions);
                self.classes.insert(0, class);
            }
            (None, None) => {
                let conditions = vec![condition_a, condition_b];
                let class = Class::new(kind, conditions);
                self.classes.insert(0, class);
            }
        }
    }

    fn contains(&self, kind: ClassKind, condition_a: &ast::Exp, condition_b: &ast::Exp) -> bool {
        if condition_a.syntax_eq(condition_b) {
            return true;
        }
        let Some(index) = self.find(kind, condition_a) else {
            return false;
        };
        let Some(conditions) = self.classes[index].conditions(kind) else {
            return false;
        };
        conditions
            .iter()
            .any(|condition| condition.syntax_eq(condition_b))
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

    fn implies(&self, prem_al: &ast::Prem) -> bool {
        let ast::PremKind::If(if_prem) = &prem_al.node else {
            return false;
        };
        self.implies_exp(&if_prem.exp)
    }
}

// == Collection result

// Must-premises hold before evaluation; insert-premises guard partial operations

struct Collected {
    prems_must: Vec<ast::Prem>,
    prems_insert: Vec<ast::Prem>,
}

impl Collected {
    fn compose(mut self, mut other: Self) -> Self {
        self.prems_must.append(&mut other.prems_must);
        self.prems_insert.append(&mut other.prems_insert);
        self
    }
}

fn filter_prems_insert(
    prems_base: &[&[ast::Prem]],
    prems_derived: &[ast::Prem],
    prems_output: &[ast::Prem],
    prems_insert: Vec<ast::Prem>,
) -> Vec<ast::Prem> {
    let prems_must = prems_base
        .iter()
        .flat_map(|prems| prems.iter())
        .chain(prems_derived)
        .chain(prems_output);
    let table = EquivalenceTable::from_prems(prems_must);
    prems_insert
        .into_iter()
        .filter(|prem_al| {
            let implied = table.implies(prem_al);
            let duplicated = prems_base
                .iter()
                .flat_map(|prems| prems.iter())
                .chain(prems_derived)
                .chain(prems_output)
                .any(|prem_must_al| prem_al.syntax_eq(prem_must_al));
            !implied && !duplicated
        })
        .collect()
}

fn iterate_prem(iter: ast::Iter, vars: &[ast::Var], prem_al: ast::Prem) -> Option<ast::Prem> {
    let frees = prem_al.free();
    let vars_bound = vars
        .iter()
        .filter(|var| frees.contains(&var.id))
        .cloned()
        .collect::<Vec<_>>();
    if vars_bound.is_empty() {
        return None;
    }
    let span = prem_al.span.clone();
    let prem_iter = ast::PremIter {
        iter,
        vars_bound,
        vars_bind: vec![],
    };
    let prem_kind = ast::PremKind::Iter(ast::IterPrem {
        prem: Box::new(prem_al),
        prem_iter,
    });
    let prem_al = phrase!(node: prem_kind, span: span);
    Some(prem_al)
}

fn iterate_prems(iter: ast::Iter, vars: &[ast::Var], prems_al: Vec<ast::Prem>) -> Vec<ast::Prem> {
    prems_al
        .into_iter()
        .filter_map(|prem_al| iterate_prem(iter, vars, prem_al))
        .collect()
}

fn iterate_collected(
    iter: ast::Iter,
    vars_must: &[ast::Var],
    vars_insert: &[ast::Var],
    collected: Collected,
) -> Collected {
    Collected {
        prems_must: iterate_prems(iter, vars_must, collected.prems_must),
        prems_insert: iterate_prems(iter, vars_insert, collected.prems_insert),
    }
}

// == Guard generation

fn gen_index_guard(
    exp_al: &ast::Exp,
    exp_base_al: &ast::Exp,
    exp_index_al: &ast::Exp,
) -> Vec<ast::Prem> {
    let span = exp_al.span.clone();
    let exp_len_al = note_phrase! {
        node: ast::ExpKind::Len(Box::new(exp_base_al.clone())),
        note: ast::TypKind::Num(xl::num::Typ::Nat),
        span: span.clone(),
    };
    let exp_guard_al = note_phrase! {
        node: ast::ExpKind::Cmp(
            ast::CmpOp::Num(xl::num::CmpOp::Lt),
            ast::OpTyp::Bool,
            Box::new(exp_index_al.clone()),
            Box::new(exp_len_al),
        ),
        note: ast::TypKind::Bool,
        span: span.clone(),
    };
    let prem_kind = ast::PremKind::If(ast::IfPrem { exp: exp_guard_al });
    let prem_guard_al = phrase!(node: prem_kind, span: span);
    vec![prem_guard_al]
}

fn gen_exp_eq_epsilon(iter: ast::Iter, var: &ast::Var) -> ast::Exp {
    let mut var = var.clone();
    var.iters.push(iter);
    let exp_al = al::var::as_exp(true, &var);
    let span = exp_al.span.clone();
    let typ = exp_al.note.clone();
    let exp_epsilon_al = note_phrase! {
        node: ast::ExpKind::Opt(None),
        note: typ,
        span: span.clone(),
    };
    note_phrase! {
        node: ast::ExpKind::Cmp(
            ast::CmpOp::Bool(xl::bool::CmpOp::Eq),
            ast::OpTyp::Bool,
            Box::new(exp_al),
            Box::new(exp_epsilon_al),
        ),
        note: ast::TypKind::Bool,
        span: span,
    }
}

fn gen_exp_len(iter: ast::Iter, var: &ast::Var) -> ast::Exp {
    let mut var = var.clone();
    var.iters.push(iter);
    let exp_al = al::var::as_exp(true, &var);
    let span = exp_al.span.clone();
    note_phrase! {
        node: ast::ExpKind::Len(Box::new(exp_al)),
        note: ast::TypKind::Num(xl::num::Typ::Nat),
        span: span,
    }
}

fn gen_exp_pair(iter: ast::Iter, exp_l_al: ast::Exp, exp_r_al: ast::Exp) -> ast::Exp {
    let span = Span::over(&[exp_l_al.span.clone(), exp_r_al.span.clone()]);
    let exp_kind = match iter {
        ast::Iter::Opt => ast::ExpKind::Bin(
            ast::BinOp::Bool(xl::bool::BinOp::Equiv),
            ast::OpTyp::Bool,
            Box::new(exp_l_al),
            Box::new(exp_r_al),
        ),
        ast::Iter::List => ast::ExpKind::Cmp(
            ast::CmpOp::Bool(xl::bool::CmpOp::Eq),
            ast::OpTyp::Bool,
            Box::new(exp_l_al),
            Box::new(exp_r_al),
        ),
    };
    note_phrase!(node: exp_kind, note: ast::TypKind::Bool, span: span)
}

fn gen_exp_and(exp_l_al: ast::Exp, exp_r_al: ast::Exp) -> ast::Exp {
    let span = Span::over(&[exp_l_al.span.clone(), exp_r_al.span.clone()]);
    note_phrase! {
        node: ast::ExpKind::Bin(
            ast::BinOp::Bool(xl::bool::BinOp::And),
            ast::OpTyp::Bool,
            Box::new(exp_l_al),
            Box::new(exp_r_al),
        ),
        note: ast::TypKind::Bool,
        span: span,
    }
}

fn gen_iter_guard(iter_exp: &ast::ExpIter) -> Vec<ast::Prem> {
    let (iter, vars) = iter_exp;
    if vars.len() < 2 {
        return vec![];
    }
    let mut exps_al = vars.iter().map(|var| match iter {
        ast::Iter::Opt => gen_exp_eq_epsilon(*iter, var),
        ast::Iter::List => gen_exp_len(*iter, var),
    });
    let Some(exp_first_al) = exps_al.next() else {
        return vec![];
    };
    let Some(mut exp_prev_al) = exps_al.next() else {
        return vec![];
    };
    let mut exp_guard_al = gen_exp_pair(*iter, exp_first_al, exp_prev_al.clone());
    for exp_al in exps_al {
        let exp_pair_al = gen_exp_pair(*iter, exp_prev_al, exp_al.clone());
        exp_guard_al = gen_exp_and(exp_guard_al, exp_pair_al);
        exp_prev_al = exp_al;
    }
    let span = exp_guard_al.span.clone();
    let prem_kind = ast::PremKind::If(ast::IfPrem { exp: exp_guard_al });
    let prem_guard_al = phrase!(node: prem_kind, span: span);
    vec![prem_guard_al]
}

// == Guard collection

// - Expressions

fn collect_exp(exp_al: &ast::Exp) -> Vec<ast::Prem> {
    match &exp_al.node {
        ast::ExpKind::Bool(_)
        | ast::ExpKind::Num(_)
        | ast::ExpKind::Text(_)
        | ast::ExpKind::Var(_) => vec![],
        ast::ExpKind::Un(_, _, exp_inner_al)
        | ast::ExpKind::UpCast(_, exp_inner_al)
        | ast::ExpKind::DownCast(_, exp_inner_al)
        | ast::ExpKind::Sub(exp_inner_al, _, _)
        | ast::ExpKind::Match(exp_inner_al, _)
        | ast::ExpKind::Len(exp_inner_al)
        | ast::ExpKind::Dot(exp_inner_al, _) => collect_exp(exp_inner_al),
        ast::ExpKind::Bin(_, _, exp_l, exp_r)
        | ast::ExpKind::Cmp(_, _, exp_l, exp_r)
        | ast::ExpKind::Cons(exp_l, exp_r)
        | ast::ExpKind::Cat(exp_l, exp_r)
        | ast::ExpKind::Mem(exp_l, exp_r) => {
            let mut prems_insert = collect_exp(exp_l);
            let prems_r_insert = collect_exp(exp_r);
            prems_insert.extend(prems_r_insert);
            prems_insert
        }
        ast::ExpKind::Tuple(exps) | ast::ExpKind::List(exps) => collect_exps(exps.iter()),
        ast::ExpKind::Case(not_exp) => collect_exps(not_exp.args()),
        ast::ExpKind::Str(fields) => collect_exps(fields.iter().map(|(_, exp)| exp)),
        ast::ExpKind::Opt(Some(exp_inner_al)) => collect_exp(exp_inner_al),
        ast::ExpKind::Opt(None) => vec![],
        ast::ExpKind::Idx(exp_base, exp_index) => {
            let mut prems_insert = collect_exp(exp_base);
            let prems_index_insert = collect_exp(exp_index);
            let prems_guard = gen_index_guard(exp_al, exp_base, exp_index);
            prems_insert.extend(prems_index_insert);
            prems_insert.extend(prems_guard);
            prems_insert
        }
        ast::ExpKind::Slice(exp_base, exp_l, exp_h) => {
            let mut prems_insert = collect_exp(exp_base);
            let prems_l_insert = collect_exp(exp_l);
            let prems_h_insert = collect_exp(exp_h);
            prems_insert.extend(prems_l_insert);
            prems_insert.extend(prems_h_insert);
            prems_insert
        }
        ast::ExpKind::Upd(exp_base, path, exp_field) => {
            let mut prems_insert = collect_exp(exp_base);
            let prems_path_insert = collect_path(path);
            let prems_field_insert = collect_exp(exp_field);
            prems_insert.extend(prems_path_insert);
            prems_insert.extend(prems_field_insert);
            prems_insert
        }
        ast::ExpKind::Call(_, _, args) => collect_args(args),
        ast::ExpKind::Iter(exp_inner, iter_exp) => {
            let prems_inner_insert = collect_exp(exp_inner);
            let mut prems_insert = iterate_prems(iter_exp.0, &iter_exp.1, prems_inner_insert);
            let prems_guard = gen_iter_guard(iter_exp);
            prems_insert.extend(prems_guard);
            prems_insert
        }
    }
}

fn collect_exps<'a>(exps: impl IntoIterator<Item = &'a ast::Exp>) -> Vec<ast::Prem> {
    let mut prems_insert = Vec::new();
    for exp_al in exps {
        let prems_exp_insert = collect_exp(exp_al);
        prems_insert.extend(prems_exp_insert);
    }
    prems_insert
}

// - Paths

fn collect_path(path: &ast::Path) -> Vec<ast::Prem> {
    match &path.node {
        ast::PathKind::Root => vec![],
        ast::PathKind::Idx(path, exp_al) => {
            let mut prems_insert = collect_path(path);
            let prems_exp_insert = collect_exp(exp_al);
            prems_insert.extend(prems_exp_insert);
            prems_insert
        }
        ast::PathKind::Slice(path, exp_l, exp_h) => {
            let mut prems_insert = collect_path(path);
            let prems_l_insert = collect_exp(exp_l);
            let prems_h_insert = collect_exp(exp_h);
            prems_insert.extend(prems_l_insert);
            prems_insert.extend(prems_h_insert);
            prems_insert
        }
        ast::PathKind::Dot(path, _) => collect_path(path),
    }
}

// - Arguments

fn collect_arg(arg: &ast::Arg) -> Vec<ast::Prem> {
    match &arg.node {
        ast::ArgKind::Exp(exp_al) => collect_exp(exp_al),
        ast::ArgKind::Def(_) => vec![],
    }
}

fn collect_args(args: &[ast::Arg]) -> Vec<ast::Prem> {
    let mut prems_insert = Vec::new();
    for arg in args {
        let prems_arg_insert = collect_arg(arg);
        prems_insert.extend(prems_arg_insert);
    }
    prems_insert
}

// - Premises

fn collect_prem(prem_al: &ast::Prem) -> Collected {
    match &prem_al.node {
        ast::PremKind::Rule(rule_prem) => collect_rule_prem(rule_prem),
        ast::PremKind::If(if_prem) => collect_if_prem(if_prem),
        ast::PremKind::IfHold(if_prem) => collect_if_hold_prem(if_prem),
        ast::PremKind::IfNotHold(if_prem) => collect_if_not_hold_prem(if_prem),
        ast::PremKind::Let(let_prem) => collect_let_prem(let_prem),
        ast::PremKind::Iter(iter_prem) => collect_iter_prem(iter_prem),
        ast::PremKind::Debug(debug_prem) => collect_debug_prem(debug_prem),
    }
}

fn collect_rule_prem(rule_prem: &ast::RulePrem) -> Collected {
    let prems_insert = collect_exps(rule_prem.not_exp.args());
    Collected {
        prems_must: vec![],
        prems_insert,
    }
}

fn collect_if_prem(if_prem: &ast::IfPrem) -> Collected {
    let prems_insert = collect_exp(&if_prem.exp);
    Collected {
        prems_must: vec![],
        prems_insert,
    }
}

fn collect_if_hold_prem(if_prem: &ast::IfHoldPrem) -> Collected {
    let prems_insert = collect_exps(if_prem.not_exp.args());
    Collected {
        prems_must: vec![],
        prems_insert,
    }
}

fn collect_if_not_hold_prem(if_prem: &ast::IfNotHoldPrem) -> Collected {
    let prems_insert = collect_exps(if_prem.not_exp.args());
    Collected {
        prems_must: vec![],
        prems_insert,
    }
}

fn collect_let_prem(let_prem: &ast::LetPrem) -> Collected {
    let prems_must = collect_exp(&let_prem.exp_l);
    let prems_insert = collect_exp(&let_prem.exp_r);
    Collected {
        prems_must,
        prems_insert,
    }
}

fn collect_iter_prem(iter_prem: &ast::IterPrem) -> Collected {
    let prem_iter = &iter_prem.prem_iter;
    let mut vars_must = prem_iter.vars_bound.clone();
    vars_must.extend(prem_iter.vars_bind.clone());

    let collected = collect_prem(&iter_prem.prem);
    let collected = iterate_collected(prem_iter.iter, &vars_must, &prem_iter.vars_bound, collected);
    let collected_guard = Collected {
        prems_must: gen_iter_guard(&(prem_iter.iter, vars_must)),
        prems_insert: gen_iter_guard(&(prem_iter.iter, prem_iter.vars_bound.clone())),
    };
    collected.compose(collected_guard)
}

fn collect_debug_prem(debug_prem: &ast::DebugPrem) -> Collected {
    let prems_insert = collect_exp(&debug_prem.exp);
    Collected {
        prems_must: vec![],
        prems_insert,
    }
}

// == Guard insertion

// - Premises

struct InsertedPrems {
    derived: Vec<ast::Prem>,
    output: Vec<ast::Prem>,
}

fn insert_prem(
    prems_base: &[&[ast::Prem]],
    prems_derived: &mut Vec<ast::Prem>,
    prems_output: &mut Vec<ast::Prem>,
    prem_al: ast::Prem,
) {
    let collected = collect_prem(&prem_al);
    let mut prems_insert = filter_prems_insert(
        prems_base,
        prems_derived,
        prems_output,
        collected.prems_insert,
    );
    prems_insert.push(prem_al);
    prems_derived.extend(collected.prems_must);
    prems_output.extend(prems_insert);
}

fn insert_prems(prems_base: &[&[ast::Prem]], prems_al: Vec<ast::Prem>) -> InsertedPrems {
    let mut prems_derived = Vec::new();
    let mut prems_output = Vec::new();
    for prem_al in prems_al {
        insert_prem(prems_base, &mut prems_derived, &mut prems_output, prem_al);
    }
    InsertedPrems {
        derived: prems_derived,
        output: prems_output,
    }
}

// - Rule groups

fn insert_rule_group(mut rule_group_al: ast::RuleGroup) -> ast::RuleGroup {
    let prems_input = collect_exps(rule_group_al.node.rule_match.exps_input.iter());
    let prems_match_al = std::mem::take(&mut rule_group_al.node.rule_match.prems);
    let prems_match = insert_prems(&[&prems_input], prems_match_al);
    rule_group_al.node.rule_match.prems = prems_match.output;

    for rule_path_al in &mut rule_group_al.node.rule_paths {
        let prems_base = [
            prems_input.as_slice(),
            prems_match.derived.as_slice(),
            rule_group_al.node.rule_match.prems.as_slice(),
        ];
        let prems_path_al = std::mem::take(&mut rule_path_al.prems);
        let prems_path = insert_prems(&prems_base, prems_path_al);
        rule_path_al.prems = prems_path.output;

        let prems_output = collect_exps(rule_path_al.exps_output.iter());
        let prems_output = filter_prems_insert(
            &prems_base,
            &prems_path.derived,
            &rule_path_al.prems,
            prems_output,
        );
        rule_path_al.prems.extend(prems_output);
    }
    rule_group_al
}

fn insert_else_group(mut else_group_al: ast::ElseGroup) -> ast::ElseGroup {
    let prems_input = collect_exps(else_group_al.node.rule_match.exps_input.iter());
    let prems_match_al = std::mem::take(&mut else_group_al.node.rule_match.prems);
    let prems_match = insert_prems(&[&prems_input], prems_match_al);
    else_group_al.node.rule_match.prems = prems_match.output;

    let prems_path_al = std::mem::take(&mut else_group_al.node.rule_path.prems);
    let prems_base = [
        prems_input.as_slice(),
        prems_match.derived.as_slice(),
        else_group_al.node.rule_match.prems.as_slice(),
    ];
    let prems_path = insert_prems(&prems_base, prems_path_al);
    else_group_al.node.rule_path.prems = prems_path.output;

    let prems_output = collect_exps(else_group_al.node.rule_path.exps_output.iter());
    let prems_output = filter_prems_insert(
        &prems_base,
        &prems_path.derived,
        &else_group_al.node.rule_path.prems,
        prems_output,
    );
    else_group_al.node.rule_path.prems.extend(prems_output);
    else_group_al
}

// - Clauses

fn insert_clause(mut clause_al: ast::Clause) -> ast::Clause {
    let prems_args = collect_args(&clause_al.node.args);
    let prems_clause_al = std::mem::take(&mut clause_al.node.premises);
    let prems_clause = insert_prems(&[&prems_args], prems_clause_al);
    clause_al.node.premises = prems_clause.output;

    let prems_output = collect_exp(&clause_al.node.expression);
    let prems_output = filter_prems_insert(
        &[&prems_args],
        &prems_clause.derived,
        &clause_al.node.premises,
        prems_output,
    );
    clause_al.node.premises.extend(prems_output);
    clause_al
}

// - Definitions

fn insert_def(def_al: ast::Def) -> ast::Def {
    let span = def_al.span;
    let def_kind_al = match def_al.node {
        ast::DefKind::Rel(rel_def_al) => {
            let rel_def_al = insert_rel_def(rel_def_al);
            ast::DefKind::Rel(rel_def_al)
        }
        ast::DefKind::FuncDec(func_dec_def_al) => {
            let func_dec_def_al = insert_func_dec_def(func_dec_def_al);
            ast::DefKind::FuncDec(func_dec_def_al)
        }
        def_kind_al => def_kind_al,
    };
    phrase!(node: def_kind_al, span: span)
}

fn insert_rel_def(mut rel_def_al: ast::RelDef) -> ast::RelDef {
    rel_def_al.rule_groups = rel_def_al
        .rule_groups
        .into_iter()
        .map(insert_rule_group)
        .collect();
    rel_def_al.else_group = rel_def_al.else_group.map(insert_else_group);
    rel_def_al
}

fn insert_func_dec_def(mut func_dec_def_al: ast::FuncDecDef) -> ast::FuncDecDef {
    func_dec_def_al.clauses = func_dec_def_al
        .clauses
        .into_iter()
        .map(insert_clause)
        .collect();
    func_dec_def_al.else_clause = func_dec_def_al.else_clause.map(insert_clause);
    func_dec_def_al
}

// == Entry point

/// Inserts side-condition guards throughout an analyzed specification.
pub(in crate::pass::algo) fn insert_spec(spec_al: ast::Spec) -> ast::Spec {
    spec_al.into_iter().map(insert_def).collect()
}
