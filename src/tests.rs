/* Notice
tests.rs: lending-library

Copyright 2018 Thomas Bytheway <thomas.bytheway@cl.cam.ac.uk>

This file is part of the lending-library open-source project: github.com/harkonenbade/lending-library;
Its licensing is governed by the LICENSE file at the root of the project.
*/

use super::{LendingLibrary, Loan};
use std::sync::atomic::Ordering;

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
