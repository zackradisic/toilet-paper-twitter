use std::ops::{Deref, DerefMut};

#[repr(transparent)]
pub struct ResetGuard<'a, T> {
    memoized: &'a mut Memoized<T>,
}

impl<'a, T> DerefMut for ResetGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.memoized.deref_mut()
    }
}

impl<'a, T> Deref for ResetGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.memoized.deref()
    }
}

impl<'a, T> Drop for ResetGuard<'a, T> {
    fn drop(&mut self) {
        self.memoized.reset();
    }
}

pub struct Memoized<T> {
    value: T,
    updated: bool,
}

impl<T> From<T> for Memoized<T> {
    fn from(value: T) -> Self {
        Self {
            value,
            updated: false,
        }
    }
}

impl<T> DerefMut for Memoized<T> {
    fn deref_mut(&mut self) -> &mut T {
        self.updated = true;
        &mut self.value
    }
}

impl<T> Deref for Memoized<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.value
    }
}

impl<T> Memoized<T> {
    pub fn reset(&mut self) {
        self.updated = false;
    }

    pub fn handle_updated(&mut self) -> Option<ResetGuard<T>> {
        if self.updated {
            Some(ResetGuard { memoized: self })
        } else {
            None
        }
    }

    pub fn updated(&self) -> bool {
        self.updated
    }
}
