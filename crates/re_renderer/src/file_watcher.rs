use std::path::PathBuf;

#[cfg(all(not(target_arch = "wasm32"), debug_assertions))] // non-wasm + debug build
#[macro_export]
macro_rules! include_file {
    ($path:expr $(,)?) => {{
        let path = ::std::path::Path::new(file!())
            .parent()
            .unwrap()
            .join($path);
        $crate::FileWatcher::get_mut(|fw| fw.watch(&path, false)).unwrap()
    }};
}

#[cfg(not(all(not(target_arch = "wasm32"), debug_assertions)))] // otherwise
#[macro_export]
macro_rules! include_file {
    ($path:expr $(,)?) => {{
        $crate::FileContents::Inlined(include_str!($path).to_owned())
    }};
}

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub enum FileContents {
    Inlined(String),
    Path(PathBuf),
}
impl FileContents {
    pub fn contents(&self) -> anyhow::Result<String> {
        match self {
            Self::Inlined(data) => Ok(data.clone()),
            Self::Path(path) => {
                std::fs::read_to_string(path).with_context(|| "failed to read file at {path:?}")
            }
        }
    }
}

use anyhow::Context;

pub use self::file_watcher_impl::{FileWatcher, FILE_WATCHER};

#[cfg(all(not(target_arch = "wasm32"), debug_assertions))] // non-wasm + debug build
mod file_watcher_impl {
    use std::path::{Path, PathBuf};

    use ahash::{HashMap, HashSet, HashSetExt};
    use anyhow::Context;
    use crossbeam::channel::Receiver;
    use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
    use parking_lot::RwLock;

    use super::FileContents;

    pub static FILE_WATCHER: RwLock<Option<FileWatcher>> = RwLock::new(None);

    pub struct FileWatcher {
        watcher: RecommendedWatcher,
        events_rx: Receiver<Event>,
    }

    impl FileWatcher {
        fn new() -> anyhow::Result<Self> {
            let (events_tx, events_rx) = crossbeam::channel::unbounded();

            let watcher = notify::recommended_watcher(move |res| match res {
                Ok(event) => {
                    if let Err(err) = events_tx.send(event) {
                        re_log::debug!(%err, "filesystem watcher shutting down");
                        return; // receiver disconnected
                    }
                }
                Err(err) => {
                    re_log::error!(%err, "filesystem watcher failure");
                }
            })?;

            Ok(Self { watcher, events_rx })
        }

        pub fn get<T>(mut f: impl FnMut(&FileWatcher) -> T) -> T {
            if let Some(fw) = FILE_WATCHER.read().as_ref() {
                return f(fw);
            }

            {
                let mut global = FILE_WATCHER.write();
                if global.is_none() {
                    let fw = Self::new().unwrap(); // TODO: handle err
                    *global = Some(fw);
                }
            }

            f(FILE_WATCHER.read().as_ref().unwrap())
        }
        pub fn get_mut<T>(mut f: impl FnMut(&mut FileWatcher) -> T) -> T {
            let mut global = FILE_WATCHER.write();

            if global.is_none() {
                let fw = Self::new().unwrap(); // TODO: handle err
                *global = Some(fw);
            }

            f(global.as_mut().unwrap())
        }

        pub fn watch(
            &mut self,
            path: impl AsRef<Path>,
            recursive: bool,
        ) -> anyhow::Result<FileContents> {
            let path = std::fs::canonicalize(path.as_ref())?;

            self.watcher
                .watch(
                    path.as_ref(),
                    recursive
                        .then_some(RecursiveMode::Recursive)
                        .unwrap_or(RecursiveMode::NonRecursive),
                )
                .with_context(|| "couldn't watch file at {path:?}")?;

            Ok(FileContents::Path(path))
        }

        pub fn unwatch(&mut self, path: impl AsRef<Path>) -> anyhow::Result<()> {
            let path = std::fs::canonicalize(path.as_ref())?;
            self.watcher
                .unwatch(path.as_ref())
                .with_context(|| "couldn't watch file at {path:?}")
        }

        /// Reads all pending events.
        // TODO: what an awful name
        pub fn dequeue(&mut self) -> HashSet<PathBuf> {
            self.events_rx
                .try_iter()
                .flat_map(|ev| {
                    use notify::EventKind::*;
                    match ev.kind {
                        Access(_) | Create(_) | Modify(_) | Any => ev
                            .paths
                            .into_iter()
                            .filter_map(|path| match std::fs::canonicalize(path) {
                                Ok(path) => Some(path),
                                Err(err) => {
                                    re_log::error!(%err, "couldn't canonicalize path");
                                    None
                                }
                            })
                            .collect::<Vec<_>>(),
                        Remove(_) | Other => vec![],
                    }
                })
                .collect()
        }
    }
}

#[cfg(not(all(not(target_arch = "wasm32"), debug_assertions)))] // otherwise
mod file_watcher_impl {
    use std::path::{Path, PathBuf};

    use ahash::{HashMap, HashSet, HashSetExt};
    use crossbeam::channel::Receiver;
    use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};

    pub static FILE_WATCHER: FileWatcher = FileWatcher;

    pub struct FileWatcher;

    impl FileWatcher {
        fn new() -> anyhow::Result<Self> {
            Ok(Self)
        }

        pub fn get<T>(mut f: impl FnMut(&FileWatcher) -> T) -> T {
            f(&Self)
        }
        pub fn get_mut<T>(mut f: impl FnMut(&mut FileWatcher) -> T) -> T {
            f(&mut Self)
        }

        pub fn watch(
            &mut self,
            _frame_index: u64,
            _path: impl AsRef<Path>,
            _recursive: bool,
        ) -> anyhow::Result<()> {
            Ok(())
        }

        pub fn unwatch(&mut self, _path: impl AsRef<Path>) -> anyhow::Result<()> {
            Ok(())
        }

        /// Reads all pending events.
        pub fn dequeue(&mut self) -> anyhow::Result<HashSet<PathBuf>> {
            Ok(HashSet::new())
        }
    }
}
