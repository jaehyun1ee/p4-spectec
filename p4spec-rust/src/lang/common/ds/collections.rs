//! Syntax-key wrappers shared by language collections

use std::{borrow::Borrow, cmp::Ordering};

use crate::lang::{common::Id, traits::cmp::SyntaxCmp};

#[repr(transparent)]
#[derive(Clone, Debug)]
pub(crate) struct ByKey<K: ?Sized>(pub(crate) K);

impl Borrow<String> for ByKey<Id> {
    fn borrow(&self) -> &String {
        &self.0.node
    }
}

impl<K: SyntaxCmp + ?Sized> PartialEq for ByKey<K> {
    fn eq(&self, key_other: &Self) -> bool {
        self.cmp(key_other) == Ordering::Equal
    }
}

impl<K: SyntaxCmp + ?Sized> Eq for ByKey<K> {}

impl<K: SyntaxCmp + ?Sized> PartialOrd for ByKey<K> {
    fn partial_cmp(&self, key_other: &Self) -> Option<Ordering> {
        Some(self.cmp(key_other))
    }
}

impl<K: SyntaxCmp + ?Sized> Ord for ByKey<K> {
    fn cmp(&self, key_other: &Self) -> Ordering {
        self.0.syntax_cmp(&key_other.0)
    }
}
