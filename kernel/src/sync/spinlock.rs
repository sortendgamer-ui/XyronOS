//! spinlock.rs — a minimal spin lock providing interior mutability for
//! kernel-wide `static`s (`Sync` is required for any `static`, and
//! this kernel has no OS underneath to provide a mutex).
//!
//! No real contention exists yet to test against — this kernel runs
//! strictly single-threaded through everything built so far (no
//! interrupts, no second CPU, no scheduler). It is still implemented
//! as a genuine spin lock (atomic compare-exchange, busy-wait), not a
//! no-op stand-in, because every subsystem after this one (interrupts,
//! the scheduler) will need real mutual exclusion, and a correct
//! primitive built now is what those subsystems will actually use
//! later — not something to revisit and rebuild at that point.

use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, Ordering};

pub struct SpinLock<T> {
    locked: AtomicBool,
    data: UnsafeCell<T>,
}

/// SAFETY: `SpinLock<T>` provides its own mutual exclusion (the
/// atomic `locked` flag, acquired before any access to `data` and
/// released after) — the same guarantee `std::sync::Mutex` relies on
/// to justify the identical bound. `T: Send` is still required: the
/// lock makes concurrent ACCESS safe, but does nothing to make a
/// non-Send `T` safe to hand across whatever different execution
/// context released vs. re-acquires the lock.
unsafe impl<T: Send> Sync for SpinLock<T> {}

impl<T> SpinLock<T> {
    /// `const fn` so this can initialize a `static` directly — every
    /// `static SpinLock<...>` in this kernel (the heap's free list,
    /// the frame allocator and VMM globals) depends on this.
    pub const fn new(data: T) -> Self {
        SpinLock {
            locked: AtomicBool::new(false),
            data: UnsafeCell::new(data),
        }
    }

    /// Acquire the lock, spinning until it's available. Returns a
    /// guard that releases the lock automatically when dropped
    /// (`Deref`/`DerefMut` give access to the wrapped `T` through it).
    pub fn lock(&self) -> SpinLockGuard<'_, T> {
        while self
            .locked
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            // A hint to the CPU that this is a spin-wait loop — on
            // x86_64 this compiles to PAUSE, which reduces power draw
            // and avoids memory-order mis-speculation penalties during
            // the spin. Not required for correctness, but skipping it
            // is exactly the kind of easy-to-omit-and-still-work
            // detail this project's coding standards ask to get right.
            core::hint::spin_loop();
        }
        SpinLockGuard { lock: self }
    }
}

pub struct SpinLockGuard<'a, T> {
    lock: &'a SpinLock<T>,
}

impl<T> Deref for SpinLockGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        // SAFETY: holding a SpinLockGuard means this lock's atomic
        // flag was successfully acquired in `lock()` and has not yet
        // been released (that only happens in Drop, below) — no other
        // guard can exist for this lock at the same time, so shared
        // access to the data is sound.
        unsafe { &*self.lock.data.get() }
    }
}

impl<T> DerefMut for SpinLockGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: same reasoning as Deref above, plus: `&mut self`
        // here proves no other reference to this specific guard
        // exists, and the guard itself proves exclusive lock
        // ownership — the two together justify a unique `&mut`.
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<T> Drop for SpinLockGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.locked.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_gives_access_to_the_wrapped_value() {
        let lock = SpinLock::new(41);
        {
            let mut guard = lock.lock();
            *guard += 1;
        }
        assert_eq!(*lock.lock(), 42);
    }

    #[test]
    fn guard_drop_releases_the_lock_for_a_subsequent_lock_call() {
        let lock = SpinLock::new(0);
        {
            let _guard = lock.lock();
            // guard dropped at the end of this block
        }
        // If drop hadn't released the lock, this would spin forever —
        // the test completing at all is the assertion.
        let guard2 = lock.lock();
        assert_eq!(*guard2, 0);
    }

    #[test]
    fn mutation_through_the_guard_is_visible_after_it_drops() {
        let lock = SpinLock::new(alloc_test_vec());
        lock.lock().push(4);
        assert_eq!(*lock.lock(), std_vec_with(4));
    }

    // Small std-backed helpers so this test can build a Vec without
    // pulling `alloc` into this module just for one test — this file
    // otherwise has no need for heap allocation itself.
    #[cfg(test)]
    fn alloc_test_vec() -> std::vec::Vec<i32> {
        std::vec![1, 2, 3]
    }
    #[cfg(test)]
    fn std_vec_with(extra: i32) -> std::vec::Vec<i32> {
        std::vec![1, 2, 3, extra]
    }
}
