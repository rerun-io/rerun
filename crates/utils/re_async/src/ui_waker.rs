use std::sync::Arc;
use std::task::{Wake, Waker};

/// Wakes up a UI thread so that it repaints, e.g. when background work has produced a result.
///
/// Usually wraps `egui::Context::request_repaint`, but this crate does not depend on `egui`.
/// Use `re_ui::ContextExt::ui_waker` to create one from an `egui::Context`.
///
/// Cloning is cheap (a reference count increment).
#[derive(Clone, Debug)]
pub struct UiWaker(Waker);

impl UiWaker {
    /// A waker that does nothing.
    #[inline]
    pub fn noop() -> Self {
        Self(Waker::noop().clone())
    }

    /// Calls `wake` every time [`Self::wake`] is called.
    pub fn new(wake: impl Fn() + Send + Sync + 'static) -> Self {
        struct FnWake<F>(F);

        impl<F: Fn() + Send + Sync + 'static> Wake for FnWake<F> {
            fn wake(self: Arc<Self>) {
                (self.0)();
            }

            fn wake_by_ref(self: &Arc<Self>) {
                (self.0)();
            }
        }

        Self(Waker::from(Arc::new(FnWake(wake))))
    }

    /// Wake up the UI.
    ///
    /// Safe to call from any thread.
    #[inline]
    pub fn wake(&self) {
        self.0.wake_by_ref();
    }
}

impl Default for UiWaker {
    #[inline]
    fn default() -> Self {
        Self::noop()
    }
}

impl From<Waker> for UiWaker {
    #[inline]
    fn from(waker: Waker) -> Self {
        Self(waker)
    }
}

impl From<UiWaker> for Waker {
    #[inline]
    fn from(waker: UiWaker) -> Self {
        waker.0
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    #[test]
    fn wakes_every_clone() {
        let count = Arc::new(AtomicUsize::new(0));
        let waker = UiWaker::new({
            let count = count.clone();
            move || {
                count.fetch_add(1, Ordering::Relaxed);
            }
        });

        let clone = waker.clone();
        drop(waker);
        clone.wake();
        Waker::from(clone).wake();

        assert_eq!(count.load(Ordering::Relaxed), 2);
    }
}
