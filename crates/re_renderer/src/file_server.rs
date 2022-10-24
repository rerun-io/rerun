use anyhow::Context;
use std::{borrow::Cow, path::PathBuf};

// ---

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
            // TODO: need some explanations
            let fs = $crate::OsFileSystem::default();
            let mut resolver = $crate::FileResolver::with_search_path(fs, {
                let mut search_path = $crate::SearchPath::default();
                // TODO: fill up search path
                search_path
            });

            let path = ::std::path::Path::new(file!())
                .parent()
                .unwrap()
                .join($path);

            $crate::FileServer::get_mut(|fs| fs.watch(&mut resolver, &path, false)).unwrap()
        }

        #[cfg(not(all(not(target_arch = "wasm32"), debug_assertions)))] // otherwise
        {
            use anyhow::Context as _;

            let path = ::std::path::Path::new(file!())
                .parent()
                .unwrap()
                .join($path);

            let path = if cfg!(not(target_arch = "wasm32")) {
                // It _has_ to be canonicalizable at that point, this is run-time!
                std::fs::canonicalize(&path)
                    .with_context(|| format!("failed to canonicalize path at {path:?}"))
                    .unwrap()
            } else {
                clean_path::clean(&path)
            };
            println!("adding {path:?} to whatever");

            // TODO: now we somehow gotta feed the beast

            // TODO: need some explanations
            let fs = $crate::MemFileSystem::get();
            fs.create_file(&path, include_str!($path).into()).unwrap();

            $crate::FileContentsHandle::Path(path)
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
    pub fn resolve_contents<Fs: FileSystem>(
        &self,
        resolver: &mut FileResolver<Fs>,
    ) -> anyhow::Result<Cow<'_, str>> {
        match self {
            Self::Inlined(data) => Ok(Cow::Borrowed(data)),
            // TODO: again with the cloning here
            Self::Path(path) => resolver.resolve_contents(path).map(|s| s.to_owned().into()),
        }
    }
}

use crate::{FileResolver, FileSystem};

pub use self::file_server_impl::FileServer;

// ---

#[cfg(all(not(target_arch = "wasm32"), debug_assertions))] // non-wasm + debug build
mod file_server_impl {
    use ahash::HashSet;
    use anyhow::Context;
    use crossbeam::channel::Receiver;
    use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
    use parking_lot::RwLock;
    use std::path::{Path, PathBuf};

    use crate::{FileResolver, FileSystem};

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

    // Singleton API
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
        pub fn watch<Fs: FileSystem>(
            &mut self,
            resolver: &mut FileResolver<Fs>,
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
                .with_context(|| format!("couldn't watch file at {path:?}"))?;

            // Watch all of its imported dependencies too!
            {
                let imports = resolver
                    .resolve_imports(&path)
                    .with_context(|| format!("couldn't resolve imports for file at {path:?}"))?;

                for path in imports {
                    self.watcher
                        .watch(
                            path.as_ref(),
                            if recursive {
                                RecursiveMode::Recursive
                            } else {
                                RecursiveMode::NonRecursive
                            },
                        )
                        .with_context(|| format!("couldn't watch file at {path:?}"))?;
                }
            }

            Ok(FileContentsHandle::Path(path))
        }

        /// Stops watching for file events at the given `path`.
        pub fn unwatch(&mut self, path: impl AsRef<Path>) -> anyhow::Result<()> {
            let path = std::fs::canonicalize(path.as_ref())?;
            self.watcher
                .unwatch(path.as_ref())
                .with_context(|| format!("couldn't unwatch file at {path:?}"))
        }

        /// Coalesces all filesystem events since the last call to `collect`,
        /// and returns a set of all modified paths.
        pub fn collect<Fs: FileSystem>(
            &mut self,
            resolver: &mut FileResolver<Fs>,
        ) -> HashSet<PathBuf> {
            fn canonicalize_opt(path: impl AsRef<Path>) -> Option<PathBuf> {
                let path = path.as_ref();
                std::fs::canonicalize(path)
                    .map_err(|err| re_log::error!(%err, ?path, "couldn't canonicalize path"))
                    .ok()
            }

            let paths = self
                .events_rx
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
                .collect();

            // A file has been modified, which means it might have new import clauses now!
            //
            // On the other hand, we don't care whether a file has dropped one of its imported
            // dependencies: worst case we'll watch a file that's not used anymore, that's
            // not an issue.
            for path in &paths {
                if let Err(err) = self.watch(resolver, path, false) {
                    re_log::error!(err=%re_error::format(err), "couldn't watch imported dependency");
                }
            }

            paths
        }
    }
}

#[cfg(not(all(not(target_arch = "wasm32"), debug_assertions)))] // otherwise
mod file_server_impl {
    use ahash::HashSet;
    use std::path::PathBuf;

    use crate::{FileResolver, FileSystem};

    /// A noop implementation of a `FileServer`.
    pub struct FileServer;

    impl FileServer {
        pub fn get<T>(mut f: impl FnMut(&FileServer) -> T) -> T {
            f(&Self)
        }

        pub fn get_mut<T>(mut f: impl FnMut(&mut FileServer) -> T) -> T {
            f(&mut Self)
        }

        pub fn collect<Fs: FileSystem>(
            &mut self,
            resolver: &mut FileResolver<Fs>,
        ) -> HashSet<PathBuf> {
            Default::default()
        }
    }
}
