/* Notice
iter.rs: lending-library

Copyright 2018 Thomas Bytheway <thomas.bytheway@cl.cam.ac.uk>

This file is part of the lending-library open-source project: github.com/harkonenbade/lending-library;
Its licensing is governed by the LICENSE file at the root of the project.
*/

//! Various iterator structs for `LendingLibrary`

use super::{LendingLibrary, State};
use std::hash::Hash;

/// An iterator over the key/value pairs of a `LendingLibrary`
pub struct Iter<'a, K: 'a, V: 'a> {
    iter: Box<Iterator<Item = (&'a K, &'a V)> + 'a>,
}

impl<'a, K, V> Iterator for Iter<'a, K, V> {
    type Item = (&'a K, &'a V);
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

/// A mutable iterator over the key/value pairs of a `LendingLibrary`
pub struct IterMut<'a, K: 'a, V: 'a> {
    iter: Box<Iterator<Item = (&'a K, &'a mut V)> + 'a>,
}

impl<'a, K, V> Iterator for IterMut<'a, K, V> {
    type Item = (&'a K, &'a mut V);
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

impl<'a, K, V> IntoIterator for &'a LendingLibrary<K, V>
where
    K: Hash,
{
    type Item = (&'a K, &'a V);
    type IntoIter = Iter<'a, K, V>;
    fn into_iter(self) -> Self::IntoIter {
        Iter {
            iter: Box::new(self.store.iter().map(|(_h, v)| match *v {
                State::Present(ref k, ref v) => (k, v),
                _ => panic!("Trying to iterate over a store with loaned items."),
            })),
        }
    }
}

impl<'a, K, V> IntoIterator for &'a mut LendingLibrary<K, V>
where
    K: Hash,
{
    type Item = (&'a K, &'a mut V);
    type IntoIter = IterMut<'a, K, V>;
    fn into_iter(self) -> Self::IntoIter {
        IterMut {
            iter: Box::new(self.store.iter_mut().map(|(_h, v)| match *v {
                State::Present(ref k, ref mut v) => (k, v),
                _ => panic!("Trying to iterate over a store with loaned items."),
            })),
        }
    }
}
