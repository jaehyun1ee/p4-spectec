use std::{fmt, rc::Rc};

use p4spectec::lang::data::intern::RcInterner;

#[test]
fn deferred_rc_entries_keep_eager_indices_and_distinct_allocations() {
    let mut interner = RcInterner::new();
    let mut interner_reference = RcInterner::new();
    let template = Rc::new(String::from("shared syntax"));
    let value_shared = Rc::new(String::from("ordinary"));
    let id_shared = interner.intern(value_shared.clone()).unwrap();
    assert_eq!(id_shared, interner_reference.intern(value_shared.clone()).unwrap());
    let mut ids = Vec::new();
    // Every fresh note takes the same index before any deferred contents are read
    for _ in 0..130 {
        let id = interner.intern_fresh_clone(template.clone()).unwrap();
        assert_eq!(
            id,
            interner_reference
                .intern(Rc::new(template.as_ref().clone()))
                .unwrap()
        );
        ids.push(id);
    }
    let id_template = interner.intern(template.clone()).unwrap();
    assert_eq!(id_template, interner_reference.intern(template.clone()).unwrap());
    assert_eq!(interner.intern(value_shared).unwrap(), id_shared);
    // Reading in reverse order cannot change the reserved identities
    let mut values = Vec::new();
    for id in ids.iter().rev() {
        let value = interner.get(*id).clone();
        assert_eq!(&value, interner_reference.get(*id));
        assert!(Rc::ptr_eq(&value, interner.get(*id)));
        assert!(!Rc::ptr_eq(&value, &template));
        assert_eq!(interner.intern(value.clone()).unwrap(), *id);
        for value_prior in &values {
            assert!(!Rc::ptr_eq(value_prior, &value));
        }
        values.push(value);
    }
    let sentinel = Rc::new(String::from("sentinel"));
    assert_eq!(
        interner.intern(sentinel.clone()).unwrap(),
        interner_reference.intern(sentinel).unwrap()
    );
    assert!(Rc::ptr_eq(interner.get(id_template), &template));
}

#[test]
fn eager_rc_entries_keep_nonclone_types_unwind_traits_and_debug_shape() {
    #[derive(Debug)]
    struct NoClone(u32);
    fn assert_unwind<T: std::panic::RefUnwindSafe + std::panic::UnwindSafe>() {}
    assert_unwind::<RcInterner<NoClone>>();
    let mut interner = RcInterner::new();
    let value = Rc::new(NoClone(7));
    let id = interner.intern(value.clone()).unwrap();
    assert_eq!(interner.get(id).0, 7);
    assert!(Rc::ptr_eq(interner.get(id), &value));
    let text = format!("{interner:?}");
    assert!(text.starts_with("RcInterner { items: [NoClone(7)], table: {"));
    assert!(!text.contains("Mutex"));
    assert!(!text.contains("Shared"));
}

#[test]
fn rc_debug_allows_a_guarded_reentrant_formatter() {
    struct Reentrant<'a> {
        interner: &'a RcInterner<String>,
        entered: bool,
    }
    impl fmt::Write for Reentrant<'_> {
        fn write_str(&mut self, text: &str) -> fmt::Result {
            if !self.entered && text.starts_with("0x") {
                self.entered = true;
                let text = format!("{:?}", self.interner);
                assert!(text.contains("syntax"));
            }
            Ok(())
        }
    }
    let mut interner = RcInterner::new();
    let id = interner
        .intern_fresh_clone(Rc::new(String::from("syntax")))
        .unwrap();
    let mut output = Reentrant { interner: &interner, entered: false };
    fmt::write(&mut output, format_args!("{interner:?}")).unwrap();
    assert!(output.entered);
    let value = interner.get(id).clone();
    assert_eq!(interner.intern(value).unwrap(), id);
}
