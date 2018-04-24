use super::LendingLibrary;
use std::{fmt::{Debug, Error as FmtError, Formatter},
          hash::Hash,
          ops::{Deref, DerefMut},
          thread};

pub struct Loan<K, V>
where
    K: Hash + Eq + Copy,
{
    pub(super) owner: *mut LendingLibrary<K, V>,
    pub(super) key: Option<K>,
    pub(super) inner: Option<V>,
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
