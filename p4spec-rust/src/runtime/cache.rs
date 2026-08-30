//! Fixed-capacity clock-eviction cache

use std::{collections::HashMap, hash::Hash};

#[derive(Clone, Debug)]
struct Slot<K> {
    key: Option<K>,
    referenced: bool,
}

#[derive(Clone, Debug)]
pub struct ClockCache<K, V> {
    table: HashMap<K, (V, usize)>,
    clock: Vec<Slot<K>>,
    capacity: usize,
    count: usize,
    hand: usize,
    fill: usize,
    touched: usize,
}

impl<K, V> ClockCache<K, V>
where
    K: Clone + Eq + Hash,
{
    pub fn new(size: usize) -> Self {
        let capacity = size.max(1);
        Self {
            table: HashMap::with_capacity(capacity),
            clock: (0..capacity)
                .map(|_| Slot {
                    key: None,
                    referenced: false,
                })
                .collect(),
            capacity,
            count: 0,
            hand: 0,
            fill: 0,
            touched: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn clear(&mut self) {
        for index in 0..self.touched {
            let slot = &mut self.clock[index];
            if let Some(key) = slot.key.take() {
                self.table.remove(&key);
                slot.referenced = false;
            }
        }
        self.count = 0;
        self.hand = 0;
        self.fill = 0;
        self.touched = 0;
    }

    pub fn find(&mut self, key: &K) -> Option<&V> {
        let index = self.table.get(key).map(|(_, index)| *index)?;
        self.clock[index].referenced = true;
        self.table.get(key).map(|(value, _)| value)
    }

    fn evict(&mut self) -> usize {
        loop {
            let index = self.hand;
            self.hand = (index + 1) % self.capacity;
            let slot = &mut self.clock[index];
            let Some(key) = slot.key.as_ref() else {
                continue;
            };
            if slot.referenced {
                slot.referenced = false;
                continue;
            }
            self.table.remove(key);
            slot.key = None;
            self.count -= 1;
            return index;
        }
    }

    pub fn insert(&mut self, key: K, value: V) {
        if let Some(index) = self.table.get(&key).map(|(_, index)| *index) {
            self.table.insert(key, (value, index));
            self.clock[index].referenced = true;
            return;
        }

        let index = if self.count < self.capacity {
            let index = self.fill;
            self.fill = (self.fill + 1) % self.capacity;
            self.touched = self.touched.max(index + 1);
            index
        } else {
            self.evict()
        };
        self.clock[index].key = Some(key.clone());
        self.clock[index].referenced = true;
        self.table.insert(key, (value, index));
        self.count += 1;
    }
}
