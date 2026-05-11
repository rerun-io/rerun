//! Wrappers around `parking_lot` locks, with a simple deadlock detection mechanism.

use std::panic::Location;

// ----------------------------------------------------------------------------

const DEADLOCK_DURATION: std::time::Duration = std::time::Duration::from_secs(10);

/// Provides interior mutability.
///
/// Only use this for locks that are expected to be held for at most a few milliseconds,
/// e.g. as part of the main GUI loop.
///
/// This is a thin wrapper around [`parking_lot::Mutex`].
#[derive(Default)]
pub struct Mutex<T> {
    #[cfg(debug_assertions)]
    last_lock_location: parking_lot::Mutex<Option<&'static Location<'static>>>,
    lock: parking_lot::Mutex<T>,
}

/// The lock you get from [`Mutex`].
pub use parking_lot::MutexGuard;

impl<T> Mutex<T> {
    #[inline(always)]
    pub fn new(val: T) -> Self {
        Self {
            #[cfg(debug_assertions)]
            last_lock_location: parking_lot::Mutex::new(None),
            lock: parking_lot::Mutex::new(val),
        }
    }

    /// Try to acquire the lock.
    ///
    /// Will log a warning in debug builds if the lock can't be acquired within 10 seconds.
    #[inline(always)]
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn lock(&self) -> MutexGuard<'_, T> {
        cfg_if::cfg_if! {
            if #[cfg(debug_assertions)] {
                let loc = Location::caller();
                let guard = self
                    .lock
                    .try_lock_for(DEADLOCK_DURATION)
                    .unwrap_or_else(|| {
                        re_log::warn_once!(
                            "[DEBUG] Failed to acquire Mutex after {}s. Deadlock?\n Already held lock location: {}\n Blocked lock location: {loc}",
                            DEADLOCK_DURATION.as_secs(),
                            self.last_lock_location.lock().expect(
                                "We set this each time we lock, and we can't get here without a lock already existing."
                            ),
                        );

                        self.lock.lock()
                    });

                *self.last_lock_location.lock() = Some(Location::caller());

                guard
            } else {
                self.lock.lock()
            }
        }
    }

    /// Returns a mutable reference to the underlying data.
    ///
    /// Since this call borrows the `Mutex` mutably, no actual locking needs to
    /// take place---the mutable borrow statically guarantees no locks exist.
    #[inline(always)]
    pub fn get_mut(&mut self) -> &mut T {
        self.lock.get_mut()
    }
}

// ----------------------------------------------------------------------------

/// The lock you get from [`RwLock::read`].
pub use parking_lot::RwLockReadGuard;

/// The lock you get from [`RwLock::write`].
pub use parking_lot::RwLockWriteGuard;

/// The lock you get from [`RwLock::read_upgradable`].
pub struct RwLockUpgradableReadGuard<'a, T: ?Sized>(parking_lot::RwLockUpgradableReadGuard<'a, T>);

impl<'a, T: ?Sized> RwLockUpgradableReadGuard<'a, T> {
    /// Atomically upgrades this upgradable read lock into an exclusive write lock.
    ///
    /// Will log a warning in debug builds if the lock can't be upgraded within 10 seconds.
    #[inline(always)]
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn upgrade(self) -> RwLockWriteGuard<'a, T> {
        if cfg!(debug_assertions) {
            let loc = Location::caller();
            parking_lot::RwLockUpgradableReadGuard::try_upgrade_for(self.0, DEADLOCK_DURATION)
                .unwrap_or_else(|guard| {
                    re_log::warn_once!(
                        "[DEBUG] Failed to upgrade RWLock after {}s. Deadlock?\n Latest upgrade location: {loc}",
                        DEADLOCK_DURATION.as_secs(),
                    );

                    parking_lot::RwLockUpgradableReadGuard::upgrade(guard)
                })
        } else {
            parking_lot::RwLockUpgradableReadGuard::upgrade(self.0)
        }
    }
}

impl<T: ?Sized> std::ops::Deref for RwLockUpgradableReadGuard<'_, T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Provides interior mutability.
///
/// Only use this for locks that are expected to be held for at most a few milliseconds,
/// e.g. as part of the main GUI loop.
///
/// This is a thin wrapper around [`parking_lot::RwLock`].
#[derive(Default)]
pub struct RwLock<T: ?Sized>(parking_lot::RwLock<T>);

impl<T> RwLock<T> {
    #[inline(always)]
    pub const fn new(val: T) -> Self {
        Self(parking_lot::RwLock::new(val))
    }
}

impl<T: ?Sized> RwLock<T> {
    /// Try to acquire read-access to the lock.
    ///
    /// Will log a warning in debug builds if the lock can't be acquired within 10 seconds.
    #[inline(always)]
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn read(&self) -> RwLockReadGuard<'_, T> {
        if cfg!(debug_assertions) {
            let loc = Location::caller();
            self.0.try_read_for(DEADLOCK_DURATION).unwrap_or_else(|| {
                re_log::warn_once!(
                    "[DEBUG] Failed to acquire RWLock read after {}s. Deadlock?\n Latest read location: {loc}",
                    DEADLOCK_DURATION.as_secs(),
                );

                self.0.read()
            })
        } else {
            self.0.read()
        }
    }

    /// Try to acquire upgradable read-access to the lock.
    ///
    /// Will log a warning in debug builds if the lock can't be acquired within 10 seconds.
    #[inline(always)]
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn read_upgradable(&self) -> RwLockUpgradableReadGuard<'_, T> {
        let guard = if cfg!(debug_assertions) {
            let loc = Location::caller();
            self.0.try_upgradable_read_for(DEADLOCK_DURATION).unwrap_or_else(|| {
                re_log::warn_once!(
                    "[DEBUG] Failed to acquire RWLock upgradable read after {}s. Deadlock?\n Latest upgradable read location: {loc}",
                    DEADLOCK_DURATION.as_secs(),
                );

                self.0.upgradable_read()
            })
        } else {
            self.0.upgradable_read()
        };
        RwLockUpgradableReadGuard(guard)
    }

    /// Try to acquire upgradable read-access to the lock.
    ///
    /// Alias for [`Self::read_upgradable`].
    #[inline(always)]
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn upgradable_read(&self) -> RwLockUpgradableReadGuard<'_, T> {
        self.read_upgradable()
    }

    /// Try to acquire write-access to the lock.
    ///
    /// Will log a warning in debug builds if the lock can't be acquired within 10 seconds.
    #[inline(always)]
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn write(&self) -> RwLockWriteGuard<'_, T> {
        if cfg!(debug_assertions) {
            let loc = Location::caller();
            self.0.try_write_for(DEADLOCK_DURATION).unwrap_or_else(|| {
                re_log::warn_once!(
                    "[DEBUG] Failed to acquire RWLock write after {}s. Deadlock?\n Latest write location: {loc}",
                    DEADLOCK_DURATION.as_secs(),
                );

                self.0.write()
            })
        } else {
            self.0.write()
        }
    }

    /// Returns a mutable reference to the underlying data.
    ///
    /// Since this call borrows the `RwLock` mutably, no actual locking needs to
    /// take place---the mutable borrow statically guarantees no locks exist.
    #[inline(always)]
    pub fn get_mut(&mut self) -> &mut T {
        self.0.get_mut()
    }
}

// ----------------------------------------------------------------------------

impl<T> Clone for Mutex<T>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        Self::new(self.lock().clone())
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for Mutex<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.lock.fmt(f)
    }
}

// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    #![allow(clippy::disallowed_methods)] // Ok for tests

    use crate::Mutex;
    use std::time::Duration;

    #[test]
    fn lock_two_different_mutexes_single_thread() {
        let one = Mutex::new(());
        let two = Mutex::new(());
        let _a = one.lock();
        let _b = two.lock();
    }

    #[test]
    fn lock_multiple_threads() {
        use std::sync::Arc;
        let one = Arc::new(Mutex::new(()));
        let our_lock = one.lock();
        let other_thread = {
            let one = Arc::clone(&one);
            std::thread::spawn(move || {
                let _lock = one.lock();
            })
        };
        std::thread::sleep(Duration::from_millis(200));
        drop(our_lock);
        other_thread.join().unwrap();
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests_rwlock {
    #![allow(clippy::disallowed_methods)] // Ok for tests

    use crate::RwLock;
    use std::time::Duration;

    #[test]
    fn lock_two_different_rwlocks_single_thread() {
        let one = RwLock::new(());
        let two = RwLock::new(());
        let _a = one.write();
        let _b = two.write();
    }

    #[test]
    fn rwlock_multiple_threads() {
        use std::sync::Arc;
        let one = Arc::new(RwLock::new(()));
        let our_lock = one.write();
        let other_thread1 = {
            let one = Arc::clone(&one);
            std::thread::spawn(move || {
                let _lock = one.write();
            })
        };
        let other_thread2 = {
            let one = Arc::clone(&one);
            std::thread::spawn(move || {
                let _lock = one.read();
            })
        };
        std::thread::sleep(Duration::from_millis(200));
        drop(our_lock);
        other_thread1.join().unwrap();
        other_thread2.join().unwrap();
    }

    #[test]
    fn rwlock_read_read_reentrancy() {
        let one = RwLock::new(());
        let _a1 = one.read();
        // This is legal: this test suite specifically targets native, which relies
        // on parking_lot's rw-locks, which are reentrant.
        let _a2 = one.read();
    }

    #[test]
    fn rwlock_short_read_foreign_read_write_reentrancy() {
        use std::sync::Arc;

        let lock = Arc::new(RwLock::new(()));

        // Thread #0 grabs a read lock
        let t0r0 = lock.read();

        // Thread #1 grabs the same read lock
        let other_thread = {
            let lock = Arc::clone(&lock);
            std::thread::spawn(move || {
                let _t1r0 = lock.read();
            })
        };
        other_thread.join().unwrap();

        // Thread #0 releases its read lock
        drop(t0r0);

        // Thread #0 now grabs a write lock, which is legal
        let _t0w0 = lock.write();
    }

    #[test]
    fn rwlock_upgradable_read() {
        let lock = RwLock::new(1);
        let guard = lock.read_upgradable();
        assert_eq!(*guard, 1);

        let mut guard = guard.upgrade();
        *guard = 2;
        drop(guard);

        assert_eq!(*lock.read(), 2);
    }
}
