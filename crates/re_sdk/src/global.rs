//! Keeps track of global and thread-local [`RecordingStream`]s and handles fallback logic between
//! them.

use std::cell::RefCell;

use once_cell::sync::OnceCell;
use parking_lot::RwLock;

use crate::{RecordingStream, RecordingType};

// ---

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
enum RecordingScope {
    Global,
    ThreadLocal,
}

impl std::fmt::Display for RecordingScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            RecordingScope::Global => "global",
            RecordingScope::ThreadLocal => "thread-local",
        })
    }
}

// ---

static GLOBAL_DATA_RECORDING: OnceCell<RwLock<Option<RecordingStream>>> = OnceCell::new();
thread_local! {
    static LOCAL_DATA_RECORDING: RefCell<Option<RecordingStream>> = RefCell::new(None);
}

static GLOBAL_BLUEPRINT_RECORDING: OnceCell<RwLock<Option<RecordingStream>>> = OnceCell::new();
thread_local! {
    static LOCAL_BLUEPRINT_RECORDING: RefCell<Option<RecordingStream>> = RefCell::new(None);
}

impl RecordingStream {
    /// Returns `overrides` if it exists, otherwise returns the most appropriate active recording
    /// of the specified type (i.e. thread-local first, then global scope), if any.
    #[inline]
    pub fn get(
        which: RecordingType,
        overrides: Option<RecordingStream>,
    ) -> Option<RecordingStream> {
        let rec = overrides.or_else(|| {
            Self::get_any(RecordingScope::ThreadLocal, which)
                .or_else(|| Self::get_any(RecordingScope::Global, which))
        });

        if rec.is_none() {
            // NOTE: This is the one and only place where a warning about missing active recording
            // should be printed, don't stutter!
            re_log::warn_once!(
                "There is no currently active {which} recording available \
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
    pub fn get_quiet(
        which: RecordingType,
        overrides: Option<RecordingStream>,
    ) -> Option<RecordingStream> {
        let rec = overrides.or_else(|| {
            Self::get_any(RecordingScope::ThreadLocal, which)
                .or_else(|| Self::get_any(RecordingScope::Global, which))
        });

        if rec.is_none() {
            // NOTE: This is the one and only place where a warning about missing active recording
            // should be printed, don't stutter!
            re_log::debug_once!(
                "There is no currently active {which} recording available \
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
    pub fn global(which: RecordingType) -> Option<RecordingStream> {
        Self::get_any(RecordingScope::Global, which)
    }

    /// Replaces the currently active recording of the specified type in the global scope with
    /// the specified one.
    ///
    /// Returns the previous one, if any.
    #[inline]
    pub fn set_global(
        which: RecordingType,
        rec: Option<RecordingStream>,
    ) -> Option<RecordingStream> {
        Self::set_any(RecordingScope::Global, which, rec)
    }

    // --- Thread local ---

    /// Returns the currently active recording of the specified type in the thread-local scope,
    /// if any.
    #[inline]
    pub fn thread_local(which: RecordingType) -> Option<RecordingStream> {
        Self::get_any(RecordingScope::ThreadLocal, which)
    }

    /// Replaces the currently active recording of the specified type in the thread-local scope
    /// with the specified one.
    #[inline]
    pub fn set_thread_local(
        which: RecordingType,
        rec: Option<RecordingStream>,
    ) -> Option<RecordingStream> {
        Self::set_any(RecordingScope::ThreadLocal, which, rec)
    }

    // --- Internal helpers ---

    fn get_any(scope: RecordingScope, which: RecordingType) -> Option<RecordingStream> {
        match which {
            RecordingType::Data => match scope {
                RecordingScope::Global => GLOBAL_DATA_RECORDING
                    .get_or_init(Default::default)
                    .read()
                    .clone(),
                RecordingScope::ThreadLocal => {
                    LOCAL_DATA_RECORDING.with(|rec| rec.borrow().clone())
                }
            },
            RecordingType::Blueprint => match scope {
                RecordingScope::Global => GLOBAL_BLUEPRINT_RECORDING
                    .get_or_init(Default::default)
                    .read()
                    .clone(),
                RecordingScope::ThreadLocal => {
                    LOCAL_BLUEPRINT_RECORDING.with(|rec| rec.borrow().clone())
                }
            },
        }
    }

    fn set_any(
        scope: RecordingScope,
        which: RecordingType,
        rec: Option<RecordingStream>,
    ) -> Option<RecordingStream> {
        match which {
            RecordingType::Data => match scope {
                RecordingScope::Global => std::mem::replace(
                    &mut *GLOBAL_DATA_RECORDING.get_or_init(Default::default).write(),
                    rec,
                ),
                RecordingScope::ThreadLocal => LOCAL_DATA_RECORDING.with(|cell| {
                    let mut cell = cell.borrow_mut();
                    std::mem::replace(&mut *cell, rec)
                }),
            },
            RecordingType::Blueprint => match scope {
                RecordingScope::Global => std::mem::replace(
                    &mut *GLOBAL_BLUEPRINT_RECORDING
                        .get_or_init(Default::default)
                        .write(),
                    rec,
                ),
                RecordingScope::ThreadLocal => LOCAL_BLUEPRINT_RECORDING.with(|cell| {
                    let mut cell = cell.borrow_mut();
                    std::mem::replace(&mut *cell, rec)
                }),
            },
        }
    }
}

// ---

#[cfg(test)]
mod tests {
    use crate::RecordingStreamBuilder;

    use super::*;

    #[test]
    fn fallbacks() {
        fn check_recording_id(expected: &RecordingStream, got: Option<RecordingStream>) {
            assert_eq!(
                expected.recording_info().unwrap().recording_id,
                got.unwrap().recording_info().unwrap().recording_id
            );
        }

        // nothing is set
        assert!(RecordingStream::get(RecordingType::Data, None).is_none());
        assert!(RecordingStream::get(RecordingType::Blueprint, None).is_none());

        // nothing is set -- explicit wins
        let explicit = RecordingStreamBuilder::new("explicit").buffered().unwrap();
        check_recording_id(
            &explicit,
            RecordingStream::get(RecordingType::Data, explicit.clone().into()),
        );
        check_recording_id(
            &explicit,
            RecordingStream::get(RecordingType::Blueprint, explicit.clone().into()),
        );

        let global_data = RecordingStreamBuilder::new("global_data")
            .buffered()
            .unwrap();
        assert!(
            RecordingStream::set_global(RecordingType::Data, Some(global_data.clone())).is_none()
        );

        let global_blueprint = RecordingStreamBuilder::new("global_blueprint")
            .buffered()
            .unwrap();
        assert!(RecordingStream::set_global(
            RecordingType::Blueprint,
            Some(global_blueprint.clone())
        )
        .is_none());

        // globals are set, no explicit -- globals win
        check_recording_id(
            &global_data,
            RecordingStream::get(RecordingType::Data, None),
        );
        check_recording_id(
            &global_blueprint,
            RecordingStream::get(RecordingType::Blueprint, None),
        );

        // overwrite globals with themselves -- we expect to get the same value back
        check_recording_id(
            &global_data,
            RecordingStream::set_global(RecordingType::Data, Some(global_data.clone())),
        );
        check_recording_id(
            &global_blueprint,
            RecordingStream::set_global(RecordingType::Blueprint, Some(global_blueprint.clone())),
        );

        std::thread::Builder::new()
            .spawn({
                let global_data = global_data.clone();
                let global_blueprint = global_blueprint.clone();
                move || {
                    // globals are still set, no explicit -- globals still win
                    check_recording_id(
                        &global_data,
                        RecordingStream::get(RecordingType::Data, None),
                    );
                    check_recording_id(
                        &global_blueprint,
                        RecordingStream::get(RecordingType::Blueprint, None),
                    );

                    let local_data = RecordingStreamBuilder::new("local_data")
                        .buffered()
                        .unwrap();
                    assert!(RecordingStream::set_thread_local(
                        RecordingType::Data,
                        Some(local_data.clone())
                    )
                    .is_none());

                    let local_blueprint = RecordingStreamBuilder::new("local_blueprint")
                        .buffered()
                        .unwrap();
                    assert!(RecordingStream::set_thread_local(
                        RecordingType::Blueprint,
                        Some(local_blueprint.clone())
                    )
                    .is_none());

                    // locals are set for this thread -- locals win
                    check_recording_id(
                        &local_data,
                        RecordingStream::get(RecordingType::Data, None),
                    );
                    check_recording_id(
                        &local_blueprint,
                        RecordingStream::get(RecordingType::Blueprint, None),
                    );

                    // explicit still outsmarts everyone no matter what
                    check_recording_id(
                        &explicit,
                        RecordingStream::get(RecordingType::Data, explicit.clone().into()),
                    );
                    check_recording_id(
                        &explicit,
                        RecordingStream::get(RecordingType::Blueprint, explicit.clone().into()),
                    );
                }
            })
            .unwrap()
            .join()
            .unwrap();

        // locals should not exist in this thread -- global wins
        check_recording_id(
            &global_data,
            RecordingStream::get(RecordingType::Data, None),
        );
        check_recording_id(
            &global_blueprint,
            RecordingStream::get(RecordingType::Blueprint, None),
        );

        let local_data = RecordingStreamBuilder::new("local_data")
            .buffered()
            .unwrap();
        assert!(
            RecordingStream::set_thread_local(RecordingType::Data, Some(local_data.clone()))
                .is_none()
        );

        let local_blueprint = RecordingStreamBuilder::new("local_blueprint")
            .buffered()
            .unwrap();
        assert!(RecordingStream::set_thread_local(
            RecordingType::Blueprint,
            Some(local_blueprint.clone())
        )
        .is_none());

        check_recording_id(
            &global_data,
            RecordingStream::set_global(RecordingType::Data, None),
        );
        check_recording_id(
            &global_blueprint,
            RecordingStream::set_global(RecordingType::Blueprint, None),
        );

        // locals still win
        check_recording_id(&local_data, RecordingStream::get(RecordingType::Data, None));
        check_recording_id(
            &local_blueprint,
            RecordingStream::get(RecordingType::Blueprint, None),
        );
    }
}
