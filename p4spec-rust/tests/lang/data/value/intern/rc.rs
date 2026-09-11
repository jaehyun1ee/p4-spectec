use std::rc::Rc;

use p4spec_rust::lang::data::value::RcInterner;

struct Item(u32);

#[test]
fn test_intern_shares_allocations_and_retains_them_through_growth() {
    let mut interner = RcInterner::new();
    let item_l = Rc::new(Item(7));
    let item_r = Rc::new(Item(7));
    let weak = Rc::downgrade(&item_l);
    let id_l = interner.intern(item_l.clone()).unwrap();
    let id_r = interner.intern(item_r.clone()).unwrap();

    assert_eq!(interner.intern(item_l.clone()).unwrap(), id_l);
    assert_ne!(id_l, id_r);
    assert!(Rc::ptr_eq(interner.get(id_l), &item_l));
    assert!(Rc::ptr_eq(interner.get(id_r), &item_r));
    drop(item_l);
    drop(item_r);

    for value in 0..512 {
        interner.intern(Rc::new(Item(value))).unwrap();
    }
    assert_eq!(interner.get(id_l).0, 7);
    let item = weak.upgrade().unwrap();
    assert_eq!(interner.intern(item).unwrap(), id_l);
    drop(interner);
    assert!(weak.upgrade().is_none());
}
