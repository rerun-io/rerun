use std::borrow::Cow;
use std::path::{Path, PathBuf};

use ahash::{HashMap, HashMapExt as _};
use anyhow::{anyhow, ensure};
use clean_path::Clean as _;
use parking_lot::RwLock;

#[cfg(load_shaders_from_disk)]
use anyhow::Context as _;

// ---

/// A very limited filesystem, just enough for our internal needs.
pub trait FileSystem {
    fn read_to_string(&self, path: impl AsRef<Path>) -> anyhow::Result<Cow<'static, str>>;

    fn canonicalize(&self, path: impl AsRef<Path>) -> anyhow::Result<PathBuf>;

    fn exists(&self, path: impl AsRef<Path>) -> bool;

    #[expect(clippy::panic)]
    fn create_dir_all(&self, _path: impl AsRef<Path>) -> anyhow::Result<()> {
        panic!("create_dir_all() is not supported on this backend")
    }

    #[expect(clippy::panic)]
    fn create_file(
        &self,
        _path: impl AsRef<Path>,
        _contents: Cow<'static, str>,
    ) -> anyhow::Result<()> {
        panic!("create_file() is not supported on this backend")
    }
}

/// Returns the recommended filesystem handle for the current platform.
#[cfg(load_shaders_from_disk)]
pub fn get_filesystem() -> OsFileSystem {
    OsFileSystem
}

/// Returns the recommended filesystem handle for the current platform.
#[cfg(not(load_shaders_from_disk))]
pub fn get_filesystem() -> &'static MemFileSystem {
    MemFileSystem::get()
}

// ---

/// A [`FileSystem`] implementation that simply delegates to `std::fs`.
///
/// Used only for native debug builds, where shader hot-reloading is a thing.
#[cfg(load_shaders_from_disk)]
#[derive(Default)]
pub struct OsFileSystem;

#[cfg(load_shaders_from_disk)]
impl FileSystem for OsFileSystem {
    fn read_to_string(&self, path: impl AsRef<Path>) -> anyhow::Result<Cow<'static, str>> {
        let path = path.as_ref();
        std::fs::read_to_string(path)
            .with_context(|| format!("failed to read file at {path:?}"))
            .map(Into::into)
    }

    fn canonicalize(&self, path: impl AsRef<Path>) -> anyhow::Result<PathBuf> {
        let path = path.as_ref();
        std::fs::canonicalize(path)
            .with_context(|| format!("failed to canonicalize path at {path:?}"))
    }

    fn exists(&self, path: impl AsRef<Path>) -> bool {
        path.as_ref().exists()
    }
}

// ---

/// A [`FileSystem`] implementation backed by an hash map.
///
/// Used in release and web builds, where shaders are embedded in our executable and
/// hot-reloading is disabled.
///
/// Also used when running unit tests.
pub struct MemFileSystem {
    files: RwLock<Option<HashMap<PathBuf, Cow<'static, str>>>>,
}

/// The global [`MemFileSystem`].
static MEM_FILE_SYSTEM: MemFileSystem = MemFileSystem::new_uninit();

impl MemFileSystem {
    const fn new_uninit() -> Self {
        Self {
            files: RwLock::new(None),
        }
    }
}

// Singleton API
impl MemFileSystem {
    /// Returns a reference to the global `MemFileSystem`.
    pub fn get() -> &'static Self {
        if MEM_FILE_SYSTEM.files.read().is_some() {
            return &MEM_FILE_SYSTEM;
        }

        {
            let mut files = MEM_FILE_SYSTEM.files.write();
            if files.is_none() {
                *files = Some(HashMap::new());
            }
        }

        &MEM_FILE_SYSTEM
    }
}

impl FileSystem for &'static MemFileSystem {
    fn read_to_string(&self, path: impl AsRef<Path>) -> anyhow::Result<Cow<'static, str>> {
        let path = path.as_ref().clean();
        let files = self.files.read();
        let files = files.as_ref().unwrap();
        files
            .get(&path)
            // NOTE: This is calling `Cow::clone`, which doesn't actually clone anything
            // if `self` is `Cow::Borrowed`!
            .cloned()
            .ok_or_else(|| anyhow!("file does not exist at {path:?}"))
    }

    fn canonicalize(&self, path: impl AsRef<Path>) -> anyhow::Result<PathBuf> {
        let path = path.as_ref().clean();
        let files = self.files.read();
        let files = files.as_ref().unwrap();
        ensure!(files.contains_key(&path), "file does not exist at {path:?}",);
        Ok(path)
    }

    fn exists(&self, path: impl AsRef<Path>) -> bool {
        let files = self.files.read();
        let files = files.as_ref().unwrap();
        files.contains_key(&path.as_ref().clean())
    }

    fn create_dir_all(&self, _: impl AsRef<Path>) -> anyhow::Result<()> {
        Ok(())
    }

    fn create_file(
        &self,
        path: impl AsRef<Path>,
        contents: Cow<'static, str>,
    ) -> anyhow::Result<()> {
        let mut files = self.files.write();
        let files = files.as_mut().unwrap();
        files.insert(path.as_ref().to_owned(), contents);
        Ok(())
    }
}
