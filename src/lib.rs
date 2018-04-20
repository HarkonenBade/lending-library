/* Notice
lib.rs: lending-library

Copyright 2018 Thomas Bytheway <thomas.bytheway@cl.cam.ac.uk>

This file is part of the lending-library open-source project: github.com/harkonenbade/lending-library;
Its licensing is governed by the LICENSE file at the root of the project.
*/

use std::{cmp::Eq,
          collections::{hash_map::Entry, HashMap},
          hash::Hash,
          ops::{Deref, DerefMut},
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
    pub fn contains_key(&self, key: K) -> bool {
        match self.store.get(&key) {
            Some(v) => match *v {
                Present(_) | Loaned => true,
                AwaitingDrop => false,
            },
            None => false,
        }
    }
    pub fn insert(&mut self, key: K, val: V) {
        match self.store.entry(key) {
            Entry::Vacant(e) => {
                e.insert(Present(val));
            }
            Entry::Occupied(mut e) => {
                let prev = e.insert(Present(val));
                match prev {
                    Present(_) => {}
                    Loaned => panic!("Cannot overwrite loaned value"),
                    AwaitingDrop => panic!("Cannot overwrite value awaiting drop"),
                }
            }
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
    pub fn lend(&mut self, key: K) -> Option<DropGuard<K, V>> {
        let ptr: *mut Self = self;
        match self.store.entry(key) {
            Entry::Occupied(mut e) => {
                let v = e.insert(Loaned);
                match v {
                    Present(val) => {
                        self.outstanding.fetch_add(1, Ordering::Relaxed);
                        Some(DropGuard {
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

pub struct Iter<'a, K, V>
where
    K: Hash + Eq + Copy + 'a,
    V: 'a,
{
    iter: Box<Iterator<Item=(&'a K, &'a V)> + 'a>,
}

impl<'a, K, V> Iter<'a, K, V>
where
    K: Hash + Eq + Copy + 'a,
    V: 'a,
{
    fn new(val: &'a LendingLibrary<K, V>) -> Self {
        Iter {
            iter: Box::new(val.store.iter().map(|(k, v)| match *v {
                State::Present(ref v) => (k, v),
                _ => panic!("Trying to iterate over a store with loaned items."),
            })),
        }
    }
}

impl<'a, K, V> Iterator for Iter<'a, K, V>
where
    K: Hash + Eq + Copy + 'a,
    V: 'a,
{
    type Item = (&'a K, &'a V);
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

impl<'a, K, V> IntoIterator for &'a LendingLibrary<K, V>
where
    K: Hash + Eq + Copy + 'a,
    V: 'a,
{
    type Item = (&'a K, &'a V);
    type IntoIter = Iter<'a, K, V>;
    fn into_iter(self) -> Self::IntoIter {
        Iter::new(&self)
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

pub struct DropGuard<K, V>
where
    K: Hash + Eq + Copy,
{
    owner: *mut LendingLibrary<K, V>,
    key: Option<K>,
    inner: Option<V>,
}

impl<K, V> Drop for DropGuard<K, V>
where
    K: Hash + Eq + Copy,
{
    fn drop(&mut self) {
        if self.inner.is_some() && !thread::panicking() {
            unsafe {
                (*self.owner).checkin(self.key.take().unwrap(), self.inner.take().unwrap());
            }
        }
    }
}

impl<K, V> Deref for DropGuard<K, V>
where
    K: Hash + Eq + Copy,
{
    type Target = V;

    fn deref(&self) -> &V {
        self.inner.as_ref().unwrap()
    }
}

impl<K, V> DerefMut for DropGuard<K, V>
where
    K: Hash + Eq + Copy,
{
    fn deref_mut(&mut self) -> &mut V {
        self.inner.as_mut().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::DropGuard;
    use super::LendingLibrary;
    use super::Ordering;
    #[test]
    fn basic_use() {
        let mut s: LendingLibrary<i64, String> = LendingLibrary::new();
        assert_eq!(s.outstanding.load(Ordering::SeqCst), 0);
        {
            s.insert(1, String::from("test"));
            assert!(s.contains_key(1));
            s.insert(2, String::from("double test"));
            assert_eq!(s.outstanding.load(Ordering::SeqCst), 0);
            {
                let mut first = s.lend(1).unwrap();
                assert_eq!(s.outstanding.load(Ordering::SeqCst), 1);
                s.insert(3, String::from("even more test"));
                assert_eq!(*first, "test");
                first.push_str("-even more");
                assert_eq!(*first, "test-even more");
            }
            assert_eq!(s.outstanding.load(Ordering::SeqCst), 0);

            {
                let mut i = (&s).into_iter();
                i.next();
                i.next();
                i.next();
                assert_eq!(i.next(), None);
            }

            let first = s.lend(1).unwrap();
            assert_eq!(s.outstanding.load(Ordering::SeqCst), 1);
            assert_eq!(*first, "test-even more");
            s.insert(2, String::from("insert test"));
            assert!(s.remove(2));
            assert!(!s.contains_key(2));
        }
        assert_eq!(s.outstanding.load(Ordering::SeqCst), 0);
    }

    #[test]
    #[should_panic(expected = "1 value loans outlived store.")]
    fn failure_to_return() {
        {
            let mut s: LendingLibrary<i64, String> = LendingLibrary::new();
            s.insert(1, String::from("test"));
            let _v = s.lend(1).unwrap();
            drop(s);
        }
    }

    #[test]
    #[should_panic(expected = "Returning replaced item")]
    fn double_reinsert() {
        let mut s: LendingLibrary<i64, String> = LendingLibrary::new();
        s.insert(1, String::from("test"));
        {
            let _v = s.lend(1);
            let _v2 = DropGuard {
                owner: &mut s as *mut LendingLibrary<i64, String>,
                key: Some(1),
                inner: Some(String::from("test")),
            };
        }
    }

    #[test]
    #[should_panic(expected = "Returning item not from store")]
    fn returning_none_store() {
        let mut s: LendingLibrary<i64, String> = LendingLibrary::new();
        {
            let _v = DropGuard {
                owner: &mut s as *mut LendingLibrary<i64, String>,
                key: Some(1),
                inner: Some(String::from("boo")),
            };
        }
    }

    #[test]
    #[should_panic(expected = "Lending already loaned value")]
    fn double_checkout() {
        let mut s: LendingLibrary<i64, String> = LendingLibrary::new();
        s.insert(1, String::from("test"));
        let _a = s.lend(1).unwrap();
        let _b = s.lend(1).unwrap();
    }

    #[test]
    #[should_panic(expected = "Lending value awaiting drop")]
    fn double_checkout_drop() {
        let mut s: LendingLibrary<i64, String> = LendingLibrary::new();
        s.insert(1, String::from("test"));
        let _a = s.lend(1).unwrap();
        s.remove(1);
        let _b = s.lend(1).unwrap();
    }

    #[test]
    fn remove_indempotent() {
        let mut s: LendingLibrary<i64, String> = LendingLibrary::new();
        s.insert(1, String::from("test"));
        let _a = s.lend(1).unwrap();
        assert!(s.remove(1));
        for _ in 0..100 {
            assert!(!s.remove(1));
        }
    }

    #[test]
    fn double_insert() {
        let mut s: LendingLibrary<i64, String> = LendingLibrary::new();
        s.insert(1, String::from("test"));
        s.insert(1, String::from("test"));
    }

    #[test]
    #[should_panic(expected = "Cannot overwrite loaned value")]
    fn double_insert_loaned() {
        let mut s: LendingLibrary<i64, String> = LendingLibrary::new();
        s.insert(1, String::from("test"));
        let _v = s.lend(1);
        s.insert(1, String::from("test"));
    }

    #[test]
    #[should_panic(expected = "Cannot overwrite value awaiting drop")]
    fn double_insert_drop() {
        let mut s: LendingLibrary<i64, String> = LendingLibrary::new();
        s.insert(1, String::from("test"));
        let _v = s.lend(1);
        s.remove(1);
        s.insert(1, String::from("test"));
    }

    #[test]
    #[should_panic(expected = "Trying to iterate over a store with loaned items.")]
    fn no_iter_loaned() {
        let mut s: LendingLibrary<i64, String> = LendingLibrary::new();
        s.insert(1, String::from("test"));
        let _v = s.lend(1);
        for _ in &s {
            println!("a");
        }
    }
}
