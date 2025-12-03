//! Keeps track of global and thread-local [`RecordingStream`]s and handles fallback logic between
//! them.

use std::cell::RefCell;
use std::sync::OnceLock;

use parking_lot::RwLock;

use crate::{RecordingStream, StoreKind};

// ---

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RecordingScope {
    Global,
    ThreadLocal,
}

impl std::fmt::Display for RecordingScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Global => "global",
            Self::ThreadLocal => "thread-local",
        })
    }
}

// ---

/// Required to work-around <https://github.com/rerun-io/rerun/issues/2889>
#[derive(Default)]
struct ThreadLocalRecording {
    stream: Option<RecordingStream>,
}

impl ThreadLocalRecording {
    fn replace(&mut self, stream: Option<RecordingStream>) -> Option<RecordingStream> {
        std::mem::replace(&mut self.stream, stream)
    }

    fn get(&self) -> Option<RecordingStream> {
        self.stream.clone()
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
impl Drop for ThreadLocalRecording {
    fn drop(&mut self) {
        if let Some(stream) = self.stream.take() {
            // Work-around for https://github.com/rerun-io/rerun/issues/2889
            // Calling drop on `self.stream` will panic the calling thread.
            // But we want to make sure we don't loose the data in the stream.
            // So how?
            re_log::warn!(
                "Using thread-local RecordingStream on macOS & Windows can result in data loss because of https://github.com/rerun-io/rerun/issues/3937"
            );

            // Give the batcher and sink threads a chance to process the data.
            std::thread::sleep(std::time::Duration::from_millis(500));

            #[expect(clippy::mem_forget)] // Intentionally not calling `drop`
            std::mem::forget(stream);
        }
    }
}

static GLOBAL_DATA_RECORDING: OnceLock<RwLock<Option<RecordingStream>>> = OnceLock::new();
thread_local! {
    static LOCAL_DATA_RECORDING: RefCell<ThreadLocalRecording> = Default::default();
}

static GLOBAL_BLUEPRINT_RECORDING: OnceLock<RwLock<Option<RecordingStream>>> = OnceLock::new();
thread_local! {
    static LOCAL_BLUEPRINT_RECORDING: RefCell<ThreadLocalRecording> = Default::default();
}

/// Check whether we are the child of a fork.
///
/// If so, then our globals need to be cleaned up because they don't have associated batching
/// or sink threads. The parent of the fork will continue to process any data in the original
/// globals so nothing is being lost by doing this.
pub fn cleanup_if_forked_child() {
    if let Some(global_recording) = RecordingStream::global(StoreKind::Recording)
        && global_recording.is_forked_child()
    {
        re_log::debug!("Fork detected. Forgetting global recording");
        RecordingStream::forget_global(StoreKind::Recording);
    }

    if let Some(global_blueprint) = RecordingStream::global(StoreKind::Blueprint)
        && global_blueprint.is_forked_child()
    {
        re_log::debug!("Fork detected. Forgetting global blueprint");
        RecordingStream::forget_global(StoreKind::Recording);
    }

    if let Some(thread_recording) = RecordingStream::thread_local(StoreKind::Recording)
        && thread_recording.is_forked_child()
    {
        re_log::debug!("Fork detected. Forgetting thread-local recording");
        RecordingStream::forget_thread_local(StoreKind::Recording);
    }

    if let Some(thread_blueprint) = RecordingStream::thread_local(StoreKind::Blueprint)
        && thread_blueprint.is_forked_child()
    {
        re_log::debug!("Fork detected. Forgetting thread-local blueprint");
        RecordingStream::forget_thread_local(StoreKind::Blueprint);
    }
}

impl RecordingStream {
    /// Returns `overrides` if it exists, otherwise returns the most appropriate active recording
    /// of the specified type (i.e. thread-local first, then global scope), if any.
    #[inline]
    pub fn get(kind: StoreKind, overrides: Option<Self>) -> Option<Self> {
        let rec = overrides.or_else(|| {
            Self::get_any(RecordingScope::ThreadLocal, kind)
                .or_else(|| Self::get_any(RecordingScope::Global, kind))
        });

        if rec.is_none() {
            // NOTE: This is the one and only place where a warning about missing active recording
            // should be printed, don't stutter!
            re_log::warn_once!(
                "There is no currently active {kind} stream available \
                for the current thread ({:?}): have you called `set_global()` and/or \
                `set_thread_local()` first?",
                std::thread::current().id(),
            );
        }

        rec
    }

    // Internal implementation of `get()` that doesn't print a warning if no recording is found.
    // Used from python-bridge.
    #[inline]
    #[doc(hidden)]
    pub fn get_quiet(kind: StoreKind, overrides: Option<Self>) -> Option<Self> {
        let rec = overrides.or_else(|| {
            Self::get_any(RecordingScope::ThreadLocal, kind)
                .or_else(|| Self::get_any(RecordingScope::Global, kind))
        });

        if rec.is_none() {
            // NOTE: This is the one and only place where a warning about missing active recording
            // should be printed, don't stutter!
            re_log::debug_once!(
                "There is no currently active {kind} stream available \
                for the current thread ({:?}): have you called `set_global()` and/or \
                `set_thread_local()` first?",
                std::thread::current().id(),
            );
        }

        rec
    }

    // --- Global ---

    /// Returns the currently active recording of the specified type in the global scope, if any.
    #[inline]
    pub fn global(kind: StoreKind) -> Option<Self> {
        Self::get_any(RecordingScope::Global, kind)
    }

    /// Replaces the currently active recording of the specified type in the global scope with
    /// the specified one.
    ///
    /// Returns the previous one, if any.
    #[inline]
    pub fn set_global(kind: StoreKind, rec: Option<Self>) -> Option<Self> {
        Self::set_any(RecordingScope::Global, kind, rec)
    }

    /// Forgets the currently active recording of the specified type in the global scope.
    ///
    /// WARNING: this intentionally bypasses any drop/flush logic. This should only ever be used in
    /// cases where you know the batcher/sink threads have been lost such as in a forked process.
    #[inline]
    pub fn forget_global(kind: StoreKind) {
        Self::forget_any(RecordingScope::Global, kind);
    }

    // --- Thread local ---

    /// Returns the currently active recording of the specified type in the thread-local scope,
    /// if any.
    #[inline]
    pub fn thread_local(kind: StoreKind) -> Option<Self> {
        Self::get_any(RecordingScope::ThreadLocal, kind)
    }

    /// Replaces the currently active recording of the specified type in the thread-local scope
    /// with the specified one.
    #[inline]
    pub fn set_thread_local(kind: StoreKind, rec: Option<Self>) -> Option<Self> {
        Self::set_any(RecordingScope::ThreadLocal, kind, rec)
    }

    /// Forgets the currently active recording of the specified type in the thread-local scope.
    ///
    /// WARNING: this intentionally bypasses any drop/flush logic. This should only ever be used in
    /// cases where you know the batcher/sink threads have been lost such as in a forked process.
    #[inline]
    pub fn forget_thread_local(kind: StoreKind) {
        Self::forget_any(RecordingScope::ThreadLocal, kind);
    }

    // --- Internal helpers ---

    fn get_any(scope: RecordingScope, kind: StoreKind) -> Option<Self> {
        match kind {
            StoreKind::Recording => match scope {
                RecordingScope::Global => GLOBAL_DATA_RECORDING
                    .get_or_init(Default::default)
                    .read()
                    .clone(),
                RecordingScope::ThreadLocal => LOCAL_DATA_RECORDING.with(|rec| rec.borrow().get()),
            },
            StoreKind::Blueprint => match scope {
                RecordingScope::Global => GLOBAL_BLUEPRINT_RECORDING
                    .get_or_init(Default::default)
                    .read()
                    .clone(),
                RecordingScope::ThreadLocal => {
                    LOCAL_BLUEPRINT_RECORDING.with(|rec| rec.borrow().get())
                }
            },
        }
    }

    fn set_any(scope: RecordingScope, kind: StoreKind, rec: Option<Self>) -> Option<Self> {
        match kind {
            StoreKind::Recording => match scope {
                RecordingScope::Global => std::mem::replace(
                    &mut *GLOBAL_DATA_RECORDING.get_or_init(Default::default).write(),
                    rec,
                ),
                RecordingScope::ThreadLocal => {
                    LOCAL_DATA_RECORDING.with(|cell| cell.borrow_mut().replace(rec))
                }
            },
            StoreKind::Blueprint => match scope {
                RecordingScope::Global => std::mem::replace(
                    &mut *GLOBAL_BLUEPRINT_RECORDING
                        .get_or_init(Default::default)
                        .write(),
                    rec,
                ),
                RecordingScope::ThreadLocal => {
                    LOCAL_BLUEPRINT_RECORDING.with(|cell| cell.borrow_mut().replace(rec))
                }
            },
        }
    }

    fn forget_any(scope: RecordingScope, kind: StoreKind) {
        #![expect(clippy::mem_forget)] // Intentionally leak memory and bypass drop cleanup
        match kind {
            StoreKind::Recording => match scope {
                RecordingScope::Global => {
                    if let Some(global) = GLOBAL_DATA_RECORDING.get() {
                        std::mem::forget(global.write().take());
                    }
                }
                RecordingScope::ThreadLocal => LOCAL_DATA_RECORDING.with(|cell| {
                    std::mem::forget(cell.take());
                }),
            },
            StoreKind::Blueprint => match scope {
                RecordingScope::Global => {
                    if let Some(global) = GLOBAL_BLUEPRINT_RECORDING.get() {
                        std::mem::forget(global.write().take());
                    }
                }
                RecordingScope::ThreadLocal => LOCAL_BLUEPRINT_RECORDING.with(|cell| {
                    std::mem::forget(cell.take());
                }),
            },
        }
    }
}

// ---

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RecordingStreamBuilder;

    #[test]
    fn fallbacks() {
        fn check_store_id(expected: &RecordingStream, got: Option<RecordingStream>) {
            assert_eq!(
                expected.store_info().unwrap().store_id,
                got.unwrap().store_info().unwrap().store_id
            );
        }

        // nothing is set
        assert!(RecordingStream::get(StoreKind::Recording, None).is_none());
        assert!(RecordingStream::get(StoreKind::Blueprint, None).is_none());

        // nothing is set -- explicit wins
        let explicit = RecordingStreamBuilder::new("rerun_example_explicit")
            .buffered()
            .unwrap();
        check_store_id(
            &explicit,
            RecordingStream::get(StoreKind::Recording, explicit.clone().into()),
        );
        check_store_id(
            &explicit,
            RecordingStream::get(StoreKind::Blueprint, explicit.clone().into()),
        );

        let global_data = RecordingStreamBuilder::new("rerun_example_global_data")
            .buffered()
            .unwrap();
        assert!(
            RecordingStream::set_global(StoreKind::Recording, Some(global_data.clone())).is_none()
        );

        let global_blueprint = RecordingStreamBuilder::new("rerun_example_global_blueprint")
            .buffered()
            .unwrap();
        assert!(
            RecordingStream::set_global(StoreKind::Blueprint, Some(global_blueprint.clone()))
                .is_none()
        );

        // globals are set, no explicit -- globals win
        check_store_id(
            &global_data,
            RecordingStream::get(StoreKind::Recording, None),
        );
        check_store_id(
            &global_blueprint,
            RecordingStream::get(StoreKind::Blueprint, None),
        );

        // overwrite globals with themselves -- we expect to get the same value back
        check_store_id(
            &global_data,
            RecordingStream::set_global(StoreKind::Recording, Some(global_data.clone())),
        );
        check_store_id(
            &global_blueprint,
            RecordingStream::set_global(StoreKind::Blueprint, Some(global_blueprint.clone())),
        );

        std::thread::Builder::new()
            .spawn({
                let global_data = global_data.clone();
                let global_blueprint = global_blueprint.clone();
                move || {
                    // globals are still set, no explicit -- globals still win
                    check_store_id(
                        &global_data,
                        RecordingStream::get(StoreKind::Recording, None),
                    );
                    check_store_id(
                        &global_blueprint,
                        RecordingStream::get(StoreKind::Blueprint, None),
                    );

                    let local_data = RecordingStreamBuilder::new("rerun_example_local_data")
                        .buffered()
                        .unwrap();
                    assert!(
                        RecordingStream::set_thread_local(
                            StoreKind::Recording,
                            Some(local_data.clone())
                        )
                        .is_none()
                    );

                    let local_blueprint =
                        RecordingStreamBuilder::new("rerun_example_local_blueprint")
                            .buffered()
                            .unwrap();
                    assert!(
                        RecordingStream::set_thread_local(
                            StoreKind::Blueprint,
                            Some(local_blueprint.clone())
                        )
                        .is_none()
                    );

                    // locals are set for this thread -- locals win
                    check_store_id(
                        &local_data,
                        RecordingStream::get(StoreKind::Recording, None),
                    );
                    check_store_id(
                        &local_blueprint,
                        RecordingStream::get(StoreKind::Blueprint, None),
                    );

                    // explicit still outsmarts everyone no matter what
                    check_store_id(
                        &explicit,
                        RecordingStream::get(StoreKind::Recording, explicit.clone().into()),
                    );
                    check_store_id(
                        &explicit,
                        RecordingStream::get(StoreKind::Blueprint, explicit.clone().into()),
                    );
                }
            })
            .unwrap()
            .join()
            .unwrap();

        // locals should not exist in this thread -- global wins
        check_store_id(
            &global_data,
            RecordingStream::get(StoreKind::Recording, None),
        );
        check_store_id(
            &global_blueprint,
            RecordingStream::get(StoreKind::Blueprint, None),
        );

        let local_data = RecordingStreamBuilder::new("rerun_example_local_data")
            .buffered()
            .unwrap();
        assert!(
            RecordingStream::set_thread_local(StoreKind::Recording, Some(local_data.clone()))
                .is_none()
        );

        let local_blueprint = RecordingStreamBuilder::new("rerun_example_local_blueprint")
            .buffered()
            .unwrap();
        assert!(
            RecordingStream::set_thread_local(StoreKind::Blueprint, Some(local_blueprint.clone()))
                .is_none()
        );

        check_store_id(
            &global_data,
            RecordingStream::set_global(StoreKind::Recording, None),
        );
        check_store_id(
            &global_blueprint,
            RecordingStream::set_global(StoreKind::Blueprint, None),
        );

        // locals still win
        check_store_id(
            &local_data,
            RecordingStream::get(StoreKind::Recording, None),
        );
        check_store_id(
            &local_blueprint,
            RecordingStream::get(StoreKind::Blueprint, None),
        );
    }
}
