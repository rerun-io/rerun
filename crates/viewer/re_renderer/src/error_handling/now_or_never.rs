//! Future utility copied from Bevy
//! <https://github.com/bevyengine/bevy/blob/4b1865f8bda3c3d6b3f399fb6727635b3ffcbb41/crates/bevy_utils/src/futures.rs>
//!
//! It is used there for a very similar purpose: catching errors on native wgpu which are known to be non-asynchronous.

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

/// Consumes the future, polls it once, and immediately returns the output
/// or returns `None` if it wasn't ready yet.
///
/// This will cancel the future if it's not ready.
#[expect(unsafe_code)]
pub fn now_or_never<F: Future>(mut future: F) -> Option<F::Output> {
    let noop_waker = noop_waker();
    let mut cx = Context::from_waker(&noop_waker);

    // SAFETY: `future` is not moved and the original value is shadowed
    let future = unsafe { Pin::new_unchecked(&mut future) };

    match future.poll(&mut cx) {
        Poll::Ready(x) => Some(x),
        Poll::Pending => None,
    }
}

#[expect(unsafe_code)]
unsafe fn noop_clone(_data: *const ()) -> RawWaker {
    noop_raw_waker()
}

#[expect(unsafe_code)]
unsafe fn noop(_data: *const ()) {}

const NOOP_WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(noop_clone, noop, noop, noop);

fn noop_raw_waker() -> RawWaker {
    RawWaker::new(std::ptr::null(), &NOOP_WAKER_VTABLE)
}

#[expect(unsafe_code)]
fn noop_waker() -> Waker {
    // SAFETY: the `RawWakerVTable` is just a big noop and doesn't violate any of the rules in `RawWakerVTable`s documentation
    // (which talks about retaining and releasing any "resources", of which there are none in this case)
    unsafe { Waker::from_raw(noop_raw_waker()) }
}
