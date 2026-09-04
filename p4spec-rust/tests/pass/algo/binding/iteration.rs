use super::super::*;

#[test]
fn test_iteration_context_commits_a_successful_iteration_scope() {
    let mut iter_ctx = ICtx::from_iterations(vec![Iteration {
        iter: ast::Iter::Opt,
        vars_bound: vec![],
        vars_bind: vec![],
    }]);
    let iteration = Iteration {
        iter: ast::Iter::List,
        vars_bound: vec![],
        vars_bind: vec![],
    };

    let mut iter_scope = iter_ctx.scope(iteration);
    iter_scope.add_var_bound(id("x", 1), typ::make::bool(), vec![]);
    let iteration = iter_scope.finish();

    assert_eq!(iteration.vars_bound.len(), 1);
    assert_eq!(iteration.vars_bound[0].iters, vec![]);
    let [outer] = iter_ctx.as_slice() else {
        panic!("expected the outer iteration");
    };
    assert_eq!(outer.vars_bound.len(), 1);
    assert_eq!(outer.vars_bound[0].iters, vec![ast::Iter::List]);
}

#[test]
fn test_iteration_context_rolls_back_a_failed_iteration_scope() {
    let mut iter_ctx = ICtx::from_iterations(vec![Iteration {
        iter: ast::Iter::Opt,
        vars_bound: vec![],
        vars_bind: vec![],
    }]);
    let original = iter_ctx.clone();
    let iteration = Iteration {
        iter: ast::Iter::List,
        vars_bound: vec![],
        vars_bind: vec![],
    };

    {
        let mut iter_scope = iter_ctx.scope(iteration);
        iter_scope.add_var_bound(id("x", 1), typ::make::bool(), vec![]);
    }

    assert_eq!(iter_ctx, original);
}
