use anyhow::Context;
use std::{borrow::Cow, path::PathBuf};

// ---

#[cfg_attr(predicate, attr)]

/// A macro to read the contents of a file on disk.
///
/// - On WASM and/or release builds, this will behave like the standard [`include_str`]
///   macro.
/// - On native debug builds, this will actually load the specified path through
///   our [`FileServer`], and keep watching for changes in the background.
#[macro_export]
macro_rules! include_file {
    ($path:expr $(,)?) => {{
        #[cfg(all(not(target_arch = "wasm32"), debug_assertions))] // non-wasm + debug build
        {
            let path = ::std::path::Path::new(file!())
                .parent()
                .unwrap()
                .join($path);
            $crate::FileServer::get_mut(|fs| fs.watch(&path, false)).unwrap()
        }

        #[cfg(not(all(not(target_arch = "wasm32"), debug_assertions)))] // otherwise
        {
            $crate::FileContentsHandle::Inlined(include_str!($path))
        }
    }};
}

// ---

/// A handle to the contents of a file.
#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub enum FileContentsHandle {
    /// Contents inlined as a UTF-8 string.
    Inlined(&'static str),
    /// Contents sit on disk, path is pre-canonicalized.
    Path(PathBuf),
}
impl FileContentsHandle {
    /// Resolve the contents of the handle.
    // TODO(cmc): #import support
    pub fn resolve(&self) -> anyhow::Result<Cow<'_, str>> {
        match self {
            Self::Inlined(data) => Ok(Cow::Borrowed(data)),
            Self::Path(path) => std::fs::read_to_string(path)
                .with_context(|| "failed to read file at {path:?}")
                .map(Into::into),
        }
    }
}

pub use self::file_server_impl::FileServer;

// ---

#[cfg(all(not(target_arch = "wasm32"), debug_assertions))] // non-wasm + debug build
mod file_server_impl {
    use std::path::{Path, PathBuf};

    use ahash::HashSet;
    use anyhow::Context;
    use crossbeam::channel::Receiver;
    use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
    use parking_lot::RwLock;

    use super::FileContentsHandle;

    /// The global [`FileServer`].
    static FILE_SERVER: RwLock<Option<FileServer>> = RwLock::new(None);

    /// A file server capable of watching filesystem events in the background and
    /// (soon) resolve #import clauses in files.
    pub struct FileServer {
        watcher: RecommendedWatcher,
        events_rx: Receiver<Event>,
    }

    // Private details
    impl FileServer {
        fn new() -> anyhow::Result<Self> {
            let (events_tx, events_rx) = crossbeam::channel::unbounded();

            let watcher = notify::recommended_watcher(move |res| match res {
                Ok(event) => {
                    if let Err(err) = events_tx.send(event) {
                        re_log::error!(%err, "filesystem watcher disconnected, discarding event");
                    }
                }
                Err(err) => {
                    re_log::error!(%err, "filesystem watcher failure");
                }
            })?;

            Ok(Self { watcher, events_rx })
        }
    }

    // Sigleton API
    impl FileServer {
        /// Returns a reference to the global `FileServer`.
        pub fn get<T>(mut f: impl FnMut(&FileServer) -> T) -> T {
            if let Some(fs) = FILE_SERVER.read().as_ref() {
                return f(fs);
            }

            {
                let mut global = FILE_SERVER.write();
                if global.is_none() {
                    // NOTE: expect() is more than enough here, considering this can only
                    // happen in debug builds.
                    let fs = Self::new().expect("failed to initialize FileServer singleton");
                    *global = Some(fs);
                }
            }

            f(FILE_SERVER.read().as_ref().unwrap())
        }

        /// Returns a mutable reference to the global `FileServer`.
        pub fn get_mut<T>(mut f: impl FnMut(&mut FileServer) -> T) -> T {
            let mut global = FILE_SERVER.write();

            if global.is_none() {
                // NOTE: expect() is more than enough here, considering this can only
                // happen in debug builds.
                let fs = Self::new().expect("failed to initialize FileServer singleton");
                *global = Some(fs);
            }

            f(global.as_mut().unwrap())
        }
    }

    // Public API
    impl FileServer {
        /// Starts watching for file events at the given `path`.
        pub fn watch(
            &mut self,
            path: impl AsRef<Path>,
            recursive: bool,
        ) -> anyhow::Result<FileContentsHandle> {
            let path = std::fs::canonicalize(path.as_ref())?;

            self.watcher
                .watch(
                    path.as_ref(),
                    if recursive {
                        RecursiveMode::Recursive
                    } else {
                        RecursiveMode::NonRecursive
                    },
                )
                .with_context(|| "couldn't watch file at {path:?}")?;

            Ok(FileContentsHandle::Path(path))
        }

        /// Stops watching for file events at the given `path`.
        pub fn unwatch(&mut self, path: impl AsRef<Path>) -> anyhow::Result<()> {
            let path = std::fs::canonicalize(path.as_ref())?;
            self.watcher
                .unwatch(path.as_ref())
                .with_context(|| "couldn't unwatch file at {path:?}")
        }

        /// Coalesces all filesystem events since the last call to `collect`,
        /// and returns a set of all modified paths.
        pub fn collect(&mut self) -> HashSet<PathBuf> {
            fn canonicalize_opt(path: impl AsRef<Path>) -> Option<PathBuf> {
                std::fs::canonicalize(path)
                    .map_err(|err| re_log::error!(%err, "couldn't canonicalize path"))
                    .ok()
            }

            self.events_rx
                .try_iter()
                .flat_map(|ev| {
                    #[allow(clippy::enum_glob_use)]
                    use notify::EventKind::*;
                    match ev.kind {
                        Access(_) | Create(_) | Modify(_) | Any => ev
                            .paths
                            .into_iter()
                            .filter_map(canonicalize_opt)
                            .collect::<Vec<_>>(),
                        Remove(_) | Other => Vec::new(),
                    }
                })
                .collect()
        }
    }
}

#[cfg(not(all(not(target_arch = "wasm32"), debug_assertions)))] // otherwise
mod file_server_impl {
    use ahash::HashSet;
    use std::path::PathBuf;

    /// A noop implementation of a `FileServer`.
    pub struct FileServer;

    impl FileServer {
        pub fn get<T>(mut f: impl FnMut(&FileServer) -> T) -> T {
            f(&Self)
        }

        pub fn get_mut<T>(mut f: impl FnMut(&mut FileServer) -> T) -> T {
            f(&mut Self)
        }

        pub fn collect(&mut self) -> HashSet<PathBuf> {
            Default::default()
        }
    }
}
