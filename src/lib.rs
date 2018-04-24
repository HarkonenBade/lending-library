/* Notice
lib.rs: lending-library

Copyright 2018 Thomas Bytheway <thomas.bytheway@cl.cam.ac.uk>

This file is part of the lending-library open-source project: github.com/harkonenbade/lending-library;
Its licensing is governed by the LICENSE file at the root of the project.
*/

pub mod iter;

use std::{cmp::Eq,
          collections::{hash_map::Entry, HashMap},
          fmt::{Debug, Error as FmtError, Formatter},
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

pub struct Loan<K, V>
where
    K: Hash + Eq + Copy,
{
    owner: *mut LendingLibrary<K, V>,
    key: Option<K>,
    inner: Option<V>,
}

impl<K, V> Debug for Loan<K, V>
where
    K: Hash + Eq + Copy,
    V: Debug,
{
    fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError> {
        <V as Debug>::fmt(self, f)
    }
}

impl<K, V> PartialEq for Loan<K, V>
where
    K: Hash + Eq + Copy,
    V: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl<K, V> Drop for Loan<K, V>
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

impl<K, V> Deref for Loan<K, V>
where
    K: Hash + Eq + Copy,
{
    type Target = V;

    fn deref(&self) -> &V {
        self.inner.as_ref().unwrap()
    }
}

impl<K, V> DerefMut for Loan<K, V>
where
    K: Hash + Eq + Copy,
{
    fn deref_mut(&mut self) -> &mut V {
        self.inner.as_mut().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::Loan;
    use super::LendingLibrary;
    use super::Ordering;
    #[test]
    fn basic_use() {
        let mut s: LendingLibrary<i64, String> = LendingLibrary::new();
        assert_eq!(s.outstanding.load(Ordering::SeqCst), 0);

        assert_eq!(s.lend(25), None);
        assert!(!s.remove(25));

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

            let first = s.lend(1).unwrap();
            assert_eq!(s.outstanding.load(Ordering::SeqCst), 1);
            assert_eq!(*first, "test-even more");

            assert_eq!(format!("{:?}", first), format!("{:?}", "test-even more"));

            s.insert(2, String::from("insert test"));
            assert!(s.remove(2));
            assert!(!s.contains_key(2));
        }
        assert_eq!(s.outstanding.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn iters() {
        let mut s: LendingLibrary<i64, i64> = LendingLibrary::new();
        s.insert(1, 1);
        s.insert(2, 1);
        s.insert(3, 1);

        for (_k, v) in s.iter_mut() {
            assert_eq!(*v, 1);
            *v = 2;
        }

        for (_k, v) in s.iter() {
            assert_eq!(*v, 2);
        }
    }

    #[test]
    fn capacity() {
        let mut s: LendingLibrary<i64, i64> = LendingLibrary::new();
        assert_eq!(s.capacity(), 0);
        s.reserve(10);
        assert!(s.capacity() >= 10);
        s = LendingLibrary::with_capacity(10);
        assert!(s.capacity() >= 10);
        s.shrink_to_fit();
        assert_eq!(s.capacity(), 0);
    }

    #[test]
    fn lengths() {
        let mut s: LendingLibrary<i64, i64> = LendingLibrary::new();
        assert_eq!(s.len(), 0);
        assert!(s.is_empty());
        s.insert(1, 1);
        s.insert(2, 1);
        assert_eq!(s.len(), 2);
        assert!(!s.is_empty());
        {
            let _v = s.lend(1);
            assert_eq!(s.len(), 2);
            assert!(!s.is_empty());
            s.remove(1);
            assert_eq!(s.len(), 1);
            assert!(!s.is_empty());
            s.clear();
        }
        assert_eq!(s.len(), 0);
        assert!(s.is_empty());
    }

    #[test]
    #[should_panic(expected = "Trying to clear while values loaned.")]
    fn clear_while_loan() {
        let mut s: LendingLibrary<i64, i64> = LendingLibrary::new();
        s.insert(1, 1);
        let _v = s.lend(1);
        s.clear();
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
            let _v2 = Loan {
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
            let _v = Loan {
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
        assert!(s.contains_key(1));
        let _a = s.lend(1).unwrap();
        assert!(s.contains_key(1));
        assert!(s.remove(1));
        assert!(!s.contains_key(1));
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

    #[test]
    #[should_panic(expected = "Trying to iterate over a store with loaned items.")]
    fn no_iter_mut_loaned() {
        let mut s: LendingLibrary<i64, String> = LendingLibrary::new();
        s.insert(1, String::from("test"));
        let _v = s.lend(1);
        for _ in &mut s {
            println!("a");
        }
    }
}
