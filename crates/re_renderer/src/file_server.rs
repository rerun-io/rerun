/// A macro to read the contents of a file on disk, and resolve #import clauses as required.
///
/// - On Wasm and/or release builds, this will behave like the standard [`include_str`]
///   macro.
/// - On native debug builds, this will actually load the specified path through
///   our [`FileServer`], and keep watching for changes in the background (both the root file
///   and whatever direct and indirect dependencies it may have through #import clauses).
#[macro_export]
macro_rules! include_file {
    ($path:expr $(,)?) => {{
        #[cfg(all(not(target_arch = "wasm32"), debug_assertions))] // non-wasm + debug build
        {
            let mut resolver = $crate::new_recommended_file_resolver();

            // There's no guarantee that users will `cargo run` from the workspace root, so
            // we need to know where that is and make sure we look for shaders from there.
            //
            // Note that we grab the env-var at _compile time_, that way this will work for
            // all cases: `cargo run`, `./rerun`, `python example.py`.
            //
            // `CARGO_WORKSPACE_DIR` is instantiated by our workspace's cargo config, see
            // `.cargo/config.toml`.
            let workspace_path = env!("CARGO_WORKSPACE_DIR");

            // The path returned by the `file!()` macro is always hermetic, which is actually
            // an issue for us in this case since we allow non-hermetic imports in debug
            // builds (we encourage them, even!).
            //
            // Thus, we need to do an actual OS canonicalization here, but it turns out that
            // `FileServer::watch()` already does it for us, so we're covered.
            let file_path = ::std::path::Path::new(file!())
                .parent()
                .unwrap()
                .join($path);

            let path = ::std::path::Path::new(&workspace_path).join(&file_path);

            $crate::FileServer::get_mut(|fs| fs.watch(&mut resolver, &path, false)).unwrap()
        }

        #[cfg(not(all(not(target_arch = "wasm32"), debug_assertions)))] // otherwise
        {
            // Make sure `workspace_shaders::init()` is called at least once, which will
            // register all shaders defined in the workspace into the run-time in-memory
            // filesystem.
            $crate::workspace_shaders::init();

            // On windows file!() will return '\'-style paths, but this code may end up
            // running in wasm where '\\' will cause issues. If we're actually running on
            // windows, `Path` will do the right thing for us.
            let path = ::std::path::Path::new(&file!().replace('\\', "/"))
                .parent()
                .unwrap()
                .join($path);

            // The path returned by the `file!()` macro is always hermetic, and we pre-load
            // our run-time virtual filesystem using the exact same hermetic prefix.
            //
            // Therefore, the in-memory filesystem will actually be able to find this path,
            // and canonicalize it.
            $crate::get_filesystem().canonicalize(&path).unwrap()
        }
    }};
}

// ---

pub use self::file_server_impl::FileServer;

#[cfg(all(not(target_arch = "wasm32"), debug_assertions))] // non-wasm + debug build
mod file_server_impl {
    use ahash::HashSet;
    use anyhow::Context as _;
    use crossbeam::channel::Receiver;
    use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
    use parking_lot::RwLock;
    use std::path::{Path, PathBuf};

    use crate::{FileResolver, FileSystem};

    /// The global [`FileServer`].
    static FILE_SERVER: RwLock<Option<FileServer>> = RwLock::new(None);

    /// A file server capable of watching filesystem events in the background and
    /// resolve #import clauses in files.
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
        ///
        /// The given `path` is canonicalized.
        pub fn watch<Fs: FileSystem>(
            &mut self,
            resolver: &mut FileResolver<Fs>,
            path: impl AsRef<Path>,
            recursive: bool,
        ) -> anyhow::Result<PathBuf> {
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
                    .populate(&path)
                    .with_context(|| format!("couldn't resolve imports for file at {path:?}"))?
                    .imports;

                for path in imports {
                    self.watcher
                        .watch(path.as_ref(), RecursiveMode::NonRecursive)
                        .with_context(|| format!("couldn't watch file at {path:?}"))?;
                }
            }

            Ok(path)
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
            _resolver: &mut FileResolver<Fs>,
        ) -> HashSet<PathBuf> {
            Default::default()
        }
    }
}
