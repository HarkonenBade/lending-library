/* Notice
lib.rs: lending-library

Copyright 2018 Thomas Bytheway <thomas.bytheway@cl.cam.ac.uk>

This file is part of the lending-library open-source project: github.com/harkonenbade/lending-library;
Its licensing is governed by the LICENSE file at the root of the project.
*/

pub mod iter;
mod loan;
#[cfg(test)]
mod tests;

pub use loan::Loan;

use std::{cmp::Eq,
          collections::{hash_map::Entry, HashMap},
          hash::Hash,
          sync::atomic::{AtomicUsize, Ordering},
          thread};

enum State<V> {
    Present(V),
    Loaned,
    AwaitingDrop,
}

use self::State::{AwaitingDrop, Loaned, Present};

#[derive(Default)]
pub struct LendingLibrary<K, V>
where
    K: Hash + Eq + Copy,
{
    store: HashMap<K, State<V>>,
    outstanding: AtomicUsize,
}

impl<K, V> LendingLibrary<K, V>
where
    K: Hash + Eq + Copy,
{
    pub fn new() -> LendingLibrary<K, V> {
        LendingLibrary {
            store: HashMap::new(),
            outstanding: AtomicUsize::new(0),
        }
    }

    pub fn with_capacity(capacity: usize) -> LendingLibrary<K, V> {
        LendingLibrary {
            store: HashMap::with_capacity(capacity),
            outstanding: AtomicUsize::new(0),
        }
    }

    pub fn capacity(&self) -> usize {
        self.store.capacity()
    }

    pub fn reserve(&mut self, additional: usize) {
        self.store.reserve(additional)
    }

    pub fn shrink_to_fit(&mut self) {
        self.store.shrink_to_fit()
    }

    pub fn iter(&self) -> iter::Iter<K, V> {
        self.into_iter()
    }

    pub fn iter_mut(&mut self) -> iter::IterMut<K, V> {
        self.into_iter()
    }

    pub fn len(&self) -> usize {
        self.store
            .iter()
            .map(|(_k, v)| match *v {
                Present(_) | Loaned => 1,
                AwaitingDrop => 0,
            })
            .sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn clear(&mut self) {
        self.store.retain(|_k, v| match *v {
            Present(_) => false,
            AwaitingDrop => true,
            Loaned => panic!("Trying to clear while values loaned."),
        })
    }

    pub fn contains_key(&self, key: K) -> bool {
        match self.store.get(&key) {
            Some(v) => match *v {
                Present(_) | Loaned => true,
                AwaitingDrop => false,
            },
            None => false,
        }
    }
    pub fn insert(&mut self, key: K, val: V) -> Option<V> {
        match self.store.insert(key, Present(val)) {
            Some(v) => match v {
                Present(v) => Some(v),
                Loaned => panic!("Cannot overwrite loaned value"),
                AwaitingDrop => panic!("Cannot overwrite value awaiting drop"),
            },
            None => None,
        }
    }
    pub fn remove(&mut self, key: K) -> bool {
        match self.store.entry(key) {
            Entry::Occupied(mut e) => {
                let v = e.insert(AwaitingDrop);
                match v {
                    Present(_) => {
                        e.remove();
                        true
                    }
                    Loaned => true,
                    AwaitingDrop => false,
                }
            }
            Entry::Vacant(_) => false,
        }
    }

    pub fn lend(&mut self, key: K) -> Option<Loan<K, V>> {
        let ptr: *mut Self = self;
        match self.store.entry(key) {
            Entry::Occupied(mut e) => {
                let v = e.insert(Loaned);
                match v {
                    Present(val) => {
                        self.outstanding.fetch_add(1, Ordering::Relaxed);
                        Some(Loan {
                            owner: ptr,
                            key: Some(key),
                            inner: Some(val),
                        })
                    }
                    Loaned => panic!("Lending already loaned value"),
                    AwaitingDrop => panic!("Lending value awaiting drop"),
                }
            }
            Entry::Vacant(_) => None,
        }
    }
    fn checkin(&mut self, key: K, val: V) {
        match self.store.entry(key) {
            Entry::Occupied(mut e) => {
                self.outstanding.fetch_sub(1, Ordering::Relaxed);
                let v = e.insert(Present(val));
                match v {
                    Present(_) => panic!("Returning replaced item"),
                    Loaned => {}
                    AwaitingDrop => {
                        e.remove();
                    }
                }
            }
            Entry::Vacant(_) => panic!("Returning item not from store"),
        }
    }
}

impl<K, V> Drop for LendingLibrary<K, V>
where
    K: Hash + Eq + Copy,
{
    fn drop(&mut self) {
        if !thread::panicking() {
            let count = self.outstanding.load(Ordering::SeqCst);
            if count != 0 {
                panic!("{} value loans outlived store.", count)
            }
        }
    }
}
