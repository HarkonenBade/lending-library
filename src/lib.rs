/* Notice
lib.rs: lending-library

Copyright 2018 Thomas Bytheway <thomas.bytheway@cl.cam.ac.uk>

This file is part of the lending-library open-source project: github.com/harkonenbade/lending-library;
Its licensing is governed by the LICENSE file at the root of the project.
*/
#![warn(missing_docs)]

//! A data store that lends temporary ownership of stored values.

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
/// A key-value data store that allows you to loan temporary ownership of values.
///
/// # Assumptions
/// The store does it's best to ensure that no unsafe behaviour occurs, however as a result it may
/// trigger several panics rather than allow an unsafe condition to arise.
///
/// The main panic condition is that a `Loan` object derived from the `lend` method on a store may
/// never outlive the store it originated from. If this condition happens the store will generate a
/// panic as it goes out of scope, noting the number of outstanding `Loan` objects.
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
    /// Creates a new empty `LendingLibrary`.
    /// # Example
    /// ```
    /// use lending_library::LendingLibrary;
    /// let mut lib: LendingLibrary<i32, i32> = LendingLibrary::new();
    /// ```
    pub fn new() -> LendingLibrary<K, V> {
        LendingLibrary {
            store: HashMap::new(),
            outstanding: AtomicUsize::new(0),
        }
    }

    /// Creates an empty `LendingLibrary` with at least the specified capacity.
    /// The library will be able to hold at least `capacity` elements without reallocating.
    /// # Example
    /// ```
    /// use lending_library::LendingLibrary;
    /// let mut lib: LendingLibrary<i32, i32> = LendingLibrary::with_capacity(100);
    /// ```
    pub fn with_capacity(capacity: usize) -> LendingLibrary<K, V> {
        LendingLibrary {
            store: HashMap::with_capacity(capacity),
            outstanding: AtomicUsize::new(0),
        }
    }

    /// Returns the number of elements the library can store without reallocating.
    /// The same bounds as [`HashMap::capacity()`] apply.
    ///
    /// [`HashMap::capacity()`]: https://doc.rust-lang.org/stable/std/collections/struct.HashMap.html#method.capacity
    /// # Example
    /// ```
    /// use lending_library::LendingLibrary;
    /// let mut lib: LendingLibrary<i32, i32> = LendingLibrary::with_capacity(100);
    /// assert!(lib.capacity() >= 100);
    /// ```
    pub fn capacity(&self) -> usize {
        self.store.capacity()
    }

    /// Reserves space such that the library can store at least `additional` new records without reallocating.
    /// # Example
    /// ```
    /// use lending_library::LendingLibrary;
    /// let mut lib: LendingLibrary<i32, i32> = LendingLibrary::with_capacity(0);
    /// assert_eq!(lib.capacity(), 0);
    /// lib.reserve(10);
    /// assert!(lib.capacity() >= 10);
    /// ```
    pub fn reserve(&mut self, additional: usize) {
        self.store.reserve(additional)
    }

    /// Reduces the stores capacity to the minimum currently required.
    /// # Example
    /// ```
    /// use lending_library::LendingLibrary;
    /// let mut lib: LendingLibrary<i32, i32> = LendingLibrary::with_capacity(10);
    /// assert!(lib.capacity() >= 10);
    /// lib.shrink_to_fit();
    /// assert_eq!(lib.capacity(), 0);
    /// ```
    pub fn shrink_to_fit(&mut self) {
        self.store.shrink_to_fit()
    }

    /// An iterator visiting all key/value pairs in arbitary order.
    /// The item type is `(&'a K, &'a V)`
    /// # Panics
    /// The iterator will panic if it encounters an item that is currently loaned from the store,
    /// so this should only be used where you are sure you have returned all loaned items.
    pub fn iter(&self) -> iter::Iter<K, V> {
        self.into_iter()
    }

    /// An iterator visiting all key/value pairs in arbitary order, with mutable references to the
    /// values. The item type is `(&'a K, &'a mut V)`
    /// # Panics
    /// The iterator will panic if it encounters an item that is currently loaned from the store,
    /// so this should only be used where you are sure you have returned all loaned items.
    pub fn iter_mut(&mut self) -> iter::IterMut<K, V> {
        self.into_iter()
    }

    /// Returns the number of items in the store.
    /// # Example
    /// ```
    /// use lending_library::LendingLibrary;
    /// let mut lib: LendingLibrary<i32, i32> = LendingLibrary::new();
    /// lib.insert(1, 1);
    /// lib.insert(2, 1);
    /// assert_eq!(lib.len(), 2);
    /// ```
    pub fn len(&self) -> usize {
        self.store
            .iter()
            .map(|(_k, v)| match *v {
                Present(_) | Loaned => 1,
                AwaitingDrop => 0,
            })
            .sum()
    }

    /// Returns true if the store is empty and false otherwise.
    /// # Example
    /// ```
    /// use lending_library::LendingLibrary;
    /// let mut lib: LendingLibrary<i32, i32> = LendingLibrary::new();
    /// assert!(lib.is_empty());
    /// lib.insert(1, 1);
    /// lib.insert(2, 1);
    /// assert!(!lib.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Removes all items from the store.
    /// # Example
    /// ```
    /// use lending_library::LendingLibrary;
    /// let mut lib: LendingLibrary<i32, i32> = LendingLibrary::new();
    /// lib.insert(1, 1);
    /// lib.insert(2, 1);
    /// {
    ///     let v = lib.lend(2).unwrap();
    ///     assert_eq!(*v, 1);
    /// }
    /// lib.clear();
    /// assert_eq!(lib.lend(1), None);
    /// ```
    pub fn clear(&mut self) {
        let new_store = self.store
            .drain()
            .filter(|&(_k, ref v)| match *v {
                Present(_) => false,
                Loaned | AwaitingDrop => true,
            })
            .map(|(k, _v)| (k, AwaitingDrop))
            .collect();
        self.store = new_store;
    }

    /// Returns true if a record with key `key` exists in the store, and false otherwise.
    /// # Example
    /// ```
    /// use lending_library::LendingLibrary;
    /// let mut lib: LendingLibrary<i32, i32> = LendingLibrary::new();
    /// assert!(!lib.contains_key(1));
    /// lib.insert(1, 1);
    /// assert!(lib.contains_key(1));
    /// ```
    pub fn contains_key(&self, key: K) -> bool {
        match self.store.get(&key) {
            Some(v) => match *v {
                Present(_) | Loaned => true,
                AwaitingDrop => false,
            },
            None => false,
        }
    }

    /// Inserts a new key/value pair into the store. If a pair with that key already exists, the
    /// previous values will be returned as `Some(V)`, otherwise the method returns `None`.
    /// # Panics
    /// The method will panic if you attempt to overwrite a key/value pair that is currently loaned.
    /// # Example
    /// ```
    /// use lending_library::LendingLibrary;
    /// let mut lib: LendingLibrary<i32, i32> = LendingLibrary::new();
    /// lib.insert(1, 1);
    /// lib.insert(2, 1);
    /// ```
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

    /// Removes a key/value pair from the store. Returning true if the key was present in the store
    /// and false otherwise.
    /// # Example
    /// ```
    /// use lending_library::LendingLibrary;
    /// let mut lib: LendingLibrary<i32, i32> = LendingLibrary::new();
    /// assert!(!lib.remove(1));
    /// lib.insert(1, 1);
    /// assert!(lib.contains_key(1));
    /// assert!(lib.remove(1));
    /// assert!(!lib.contains_key(1));
    /// assert!(!lib.remove(1));
    /// ```
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

    /// Loans a value from the library, returning `Some(Loan<K, V>)` if the value is present, and `None` if it is not.
    /// # Panics
    /// Will panic if you try and loan a value that still has an outstanding loan.
    /// # Examples
    /// ```
    /// use lending_library::LendingLibrary;
    /// let mut lib: LendingLibrary<i32, i32> = LendingLibrary::with_capacity(0);
    /// lib.insert(1, 1);
    /// {
    ///     let mut v = lib.lend(1).unwrap();
    ///     *v += 5;
    /// }
    /// ```
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
