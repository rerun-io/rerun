use std::ops::{Deref, DerefMut};

/// A wrapper that tracks the last modification time of a value.
///
/// Provides immutable access via `Deref`, and mutable access via `modify()`,
/// which returns a guard that automatically updates the timestamp when dropped.
pub struct Tracked<T> {
    value: T,
    updated_at: jiff::Timestamp,
}

/// A guard that provides mutable access to a tracked value and updates
/// the timestamp when dropped.
pub struct TrackedGuard<'a, T> {
    tracked: &'a mut Tracked<T>,
}

impl<T> Tracked<T> {
    /// Create a new tracked value with the current timestamp.
    pub fn new(value: T) -> Self {
        Self {
            value,
            updated_at: jiff::Timestamp::now(),
        }
    }

    /// Get the last update timestamp.
    #[inline]
    pub fn updated_at(&self) -> jiff::Timestamp {
        self.updated_at
    }

    /// Get a guard for mutable access to the inner value.
    ///
    /// The timestamp is updated when the guard is dropped, ensuring it reflects
    /// the actual end time of the modification.
    #[inline]
    pub fn modify(&mut self) -> TrackedGuard<'_, T> {
        TrackedGuard { tracked: self }
    }
}

impl<T> Deref for Tracked<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> Deref for TrackedGuard<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.tracked.value
    }
}

impl<T> DerefMut for TrackedGuard<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.tracked.value
    }
}

impl<T> Drop for TrackedGuard<'_, T> {
    fn drop(&mut self) {
        self.tracked.updated_at = jiff::Timestamp::now();
    }
}

impl<T: Clone> Clone for Tracked<T> {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
            updated_at: self.updated_at,
        }
    }
}
