/// A macro to read the contents of a file on disk, and resolve #import clauses as required.
///
/// - If `load_shaders_from_disk` is disabled, this will behave like the standard [`include_str`]
///   macro.
/// - If `load_shaders_from_disk` is enabled, this will actually load the specified path through
///   our [`FileServer`], and keep watching for changes in the background (both the root file
///   and whatever direct and indirect dependencies it may have through #import clauses).
#[macro_export]
#[cfg(load_shaders_from_disk)]
macro_rules! include_file {
    ($path:expr $(,)?) => {{
        // Native debug build, we have access to the disk both while building and while
        // running, we just need to interpolated the relative paths passed into the macro.

        // TODO(andreas): Creating the resolver here again wasteful and hacky - should use resolver from render context for shaders.
        let mut resolver = $crate::new_recommended_file_resolver();

        let root_path = ::std::path::PathBuf::from(file!());

        // If we're building from within the workspace, `file!()` will return a relative path
        // starting at the workspace root.
        // We're packing shaders using the re_renderer crate as root instead (to avoid nasty
        // problems when publishing: as we lose workspace information when publishing!), so we
        // need to make sure to strip the path down.
        let path = if let Ok(root_path) = root_path.strip_prefix("crates/viewer/re_renderer") {
            let path = root_path.parent().unwrap().join($path);

            // If we're building from outside the workspace, `path` is an absolute path already and
            // we're good to go; but if we're building from within, `path` is currently a relative
            // path that assumes the CWD is the root of re_renderer, we need to make it absolute as
            // there is no guarantee that this is where `cargo run` is being run from.
            let manifest_path = ::std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            manifest_path.join(path)
        } else {
            // If we're not inside re_renderer, just take the path as-is, ignoring any implications of publishing.
            // TODO(andreas): Sounds like an problem waiting to happen! Need a better solution for this.
            root_path.parent().unwrap().join($path)
        };

        use $crate::external::anyhow::Context as _;
        $crate::FileServer::get_mut(|fs| fs.watch(&mut resolver, &path, false))
            .with_context(|| format!("include_file!({}) (rooted at {:?}) failed while trying to import physical path {path:?}", $path, root_path))
            .unwrap()
    }};
}

#[macro_export]
#[cfg(not(load_shaders_from_disk))]
macro_rules! include_file {
    ($path:expr $(,)?) => {{
        // On windows file!() will return '\'-style paths, but this code may end up
        // running in wasm where '\\' will cause issues. If we're actually running on
        // windows, `Path` will do the right thing for us.
        let path = ::std::path::Path::new(&file!().replace('\\', "/"))
            .parent()
            .unwrap()
            .join($path);

        // If we're building from within the workspace, `file!()` will return a relative path
        // starting at the workspace root.
        // We're packing shaders using the re_renderer crate as root instead (to avoid nasty
        // problems when publishing: as we lose workspace information when publishing!), so we
        // need to make sure to strip the path down.
        let path = path
            .strip_prefix("crates/viewer/re_renderer")
            .map_or_else(|_| path.clone(), ToOwned::to_owned);

        // If we're building from outside the workspace, `file!()` will return an absolute path
        // that might point to anywhere: it doesn't matter, just strip it down to a relative
        // re_renderer path no matter what.
        let manifest_path = ::std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let path = path
            .strip_prefix(&manifest_path)
            .map_or_else(|_| path.clone(), ToOwned::to_owned);

        // At this point our path is guaranteed to be hermetic, and we pre-load
        // our run-time virtual filesystem using the exact same hermetic prefix.
        //
        // Therefore, the in-memory filesystem will actually be able to find this path,
        // and canonicalize it.
        use $crate::external::anyhow::Context as _;
        use $crate::FileSystem as _;
        $crate::get_filesystem().canonicalize(&path)
            .with_context(|| format!("include_file!({}) (rooted at {:?}) failed while trying to import virtual path {path:?}", $path, file!()))
            .unwrap()
    }};
}

// ---

pub use self::file_server_impl::FileServer;

#[cfg(load_shaders_from_disk)]
mod file_server_impl {
    use std::path::{Path, PathBuf};

    use ahash::{HashMap, HashSet};
    use anyhow::Context as _;
    use crossbeam::channel::Receiver;
    use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher as _};
    use parking_lot::RwLock;

    use crate::{FileResolver, FileSystem};

    /// The global [`FileServer`].
    static FILE_SERVER: RwLock<Option<FileServer>> = RwLock::new(None);

    /// A file server capable of watching filesystem events in the background and
    /// resolve #import clauses in files.
    pub struct FileServer {
        watcher: RecommendedWatcher,
        events_rx: Receiver<Event>,
        file_watch_count: HashMap<PathBuf, usize>,
    }

    // Private details
    impl FileServer {
        fn new() -> anyhow::Result<Self> {
            let (events_tx, events_rx) = crossbeam::channel::bounded(32 * 1024);

            let watcher = notify::recommended_watcher(move |res| match res {
                Ok(event) => {
                    if let Err(err) = re_quota_channel::send_crossbeam(&events_tx, event) {
                        re_log::error!(%err, "filesystem watcher disconnected, discarding event");
                    }
                }
                Err(err) => {
                    re_log::error!(%err, "filesystem watcher failure");
                }
            })?;

            Ok(Self {
                watcher,
                events_rx,
                file_watch_count: HashMap::default(),
            })
        }
    }

    // Singleton API
    impl FileServer {
        /// Returns a reference to the global `FileServer`.
        pub fn get<T>(mut f: impl FnMut(&Self) -> T) -> T {
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
        pub fn get_mut<T>(mut f: impl FnMut(&mut Self) -> T) -> T {
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
            resolver: &FileResolver<Fs>,
            path: impl AsRef<Path>,
            recursive: bool,
        ) -> anyhow::Result<PathBuf> {
            let path = std::fs::canonicalize(path.as_ref())?;

            match self.file_watch_count.entry(path.clone()) {
                std::collections::hash_map::Entry::Occupied(mut entry) => {
                    *entry.get_mut() += 1;
                    return Ok(path);
                }
                std::collections::hash_map::Entry::Vacant(entry) => entry.insert(1),
            };

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

            match self.file_watch_count.entry(path.clone()) {
                std::collections::hash_map::Entry::Occupied(mut entry) => {
                    *entry.get_mut() -= 1;
                    if *entry.get() == 0 {
                        entry.remove();
                    }
                }
                std::collections::hash_map::Entry::Vacant(_) => {
                    anyhow::bail!("The path {path:?} was not or no longer watched");
                }
            }

            self.watcher
                .unwatch(path.as_ref())
                .with_context(|| format!("couldn't unwatch file at {path:?}"))
        }

        /// Coalesces all filesystem events since the last call to `collect`,
        /// and returns a set of all modified paths.
        pub fn collect<Fs: FileSystem>(&mut self, resolver: &FileResolver<Fs>) -> HashSet<PathBuf> {
            fn canonicalize_opt(path: impl AsRef<Path>) -> Option<PathBuf> {
                let path = path.as_ref();
                std::fs::canonicalize(path)
                    .map_err(|err| re_log::error!(%err, ?path, "couldn't canonicalize path"))
                    .ok()
            }

            let paths: HashSet<PathBuf> = self
                .events_rx
                .try_iter()
                .flat_map(|ev| {
                    use notify::EventKind;
                    match ev.kind {
                        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Any => ev
                            .paths
                            .into_iter()
                            .filter_map(canonicalize_opt)
                            .collect::<Vec<_>>(),
                        EventKind::Access(_) | EventKind::Remove(_) | EventKind::Other => {
                            Vec::new()
                        }
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

#[cfg(not(load_shaders_from_disk))]
mod file_server_impl {
    use std::path::PathBuf;

    use ahash::HashSet;

    use crate::{FileResolver, FileSystem};

    /// A noop implementation of a `FileServer`.
    pub struct FileServer;

    impl FileServer {
        pub fn get<T>(mut f: impl FnMut(&Self) -> T) -> T {
            f(&Self)
        }

        pub fn get_mut<T>(mut f: impl FnMut(&mut Self) -> T) -> T {
            f(&mut Self)
        }

        #[expect(clippy::needless_pass_by_ref_mut, clippy::unused_self)]
        pub fn collect<Fs: FileSystem>(
            &mut self,
            _resolver: &FileResolver<Fs>,
        ) -> HashSet<PathBuf> {
            Default::default()
        }
    }
}
