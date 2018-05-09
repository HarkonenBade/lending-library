/* Notice
loan.rs: lending-library

Copyright 2018 Thomas Bytheway <thomas.bytheway@cl.cam.ac.uk>

This file is part of the lending-library open-source project: github.com/harkonenbade/lending-library;
Its licensing is governed by the LICENSE file at the root of the project.
*/

use super::LendingLibrary;
use std::{fmt::{Debug, Error as FmtError, Formatter},
          hash::Hash,
          ops::{Deref, DerefMut},
          thread};

/// A smart pointer representing the loan of a key/value pair from a `LendingLibrary` instance.
pub struct Loan<K, V>
where
    K: Hash,
{
    pub(super) owner: *mut LendingLibrary<K, V>,
    pub(super) key: u64,
    pub(super) inner: Option<V>,
}

impl<K, V> Debug for Loan<K, V>
where
    K: Hash,
    V: Debug,
{
    fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError> {
        <V as Debug>::fmt(self, f)
    }
}

impl<K, V> PartialEq for Loan<K, V>
where
    K: Hash,
    V: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl<K, V> Drop for Loan<K, V>
where
    K: Hash,
{
    fn drop(&mut self) {
        if self.inner.is_some() && !thread::panicking() {
            unsafe {
                (*self.owner).checkin(self.key, self.inner.take().unwrap());
            }
        }
    }
}

impl<K, V> Deref for Loan<K, V>
where
    K: Hash,
{
    type Target = V;

    fn deref(&self) -> &V {
        self.inner.as_ref().unwrap()
    }
}

impl<K, V> DerefMut for Loan<K, V>
where
    K: Hash,
{
    fn deref_mut(&mut self) -> &mut V {
        self.inner.as_mut().unwrap()
    }
}
