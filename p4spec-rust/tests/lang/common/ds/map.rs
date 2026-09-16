use p4spec_rust::lang::common::{Iter, ds::map::VarMap, var::var::Variable};

use super::super::id;

#[test]
fn test_var_map_lookup_and_mutation_ignore_spans() {
    let var = Variable::new(id("x", "first"), vec![Iter::List]);
    let var_other = Variable::new(id("x", "second"), vec![Iter::List]);
    let var_opt = Variable::new(id("x", "first"), vec![Iter::Opt]);
    let mut map = VarMap::from_lists(&[var.clone(), var_opt.clone()], &[1, 2]).unwrap();
    let snapshot = map.clone();
    assert_eq!(map.get(&var_other), Some(&1));
    assert_eq!(map.insert(var_other, 3), Some(1));
    assert_eq!(snapshot.get(&var), Some(&1));
    assert_eq!(map.get(&var), Some(&3));
    assert_eq!(map.get(&var_opt), Some(&2));
}

#[test]
fn test_var_map_replaces_equivalent_keys_and_orders_iterator_paths() {
    let var_list = Variable::new(id("x", "first"), vec![Iter::List]);
    let var_list_other = Variable::new(id("x", "second"), vec![Iter::List]);
    let var_plain = Variable::new(id("x", "first"), vec![]);
    let mut map = VarMap::new();
    map.insert(var_list.clone(), 1);
    map.insert(var_plain.clone(), 2);
    assert_eq!(map.insert(var_list_other, 3), Some(1));
    assert_eq!(map.keys().collect::<Vec<_>>(), [&var_plain, &var_list]);
    assert_eq!(map.iter().map(|(_, value)| *value).collect::<Vec<_>>(), [2, 3]);
}
