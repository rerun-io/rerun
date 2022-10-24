use std::{
    borrow::Cow,
    path::{Path, PathBuf},
};

use ahash::{HashMap, HashMapExt};
use anyhow::{anyhow, ensure, Context as _};
use clean_path::Clean as _;

// ---

/// A very limited filesystem, just enough for our internal needs.
pub trait FileSystem {
    // fn read(&self, path: impl AsRef<Path>) -> anyhow::Result<Cow<'static, [u8]>>;
    fn read_to_string(&self, path: impl AsRef<Path>) -> anyhow::Result<Cow<'static, str>>;
    fn canonicalize(&self, path: impl AsRef<Path>) -> anyhow::Result<PathBuf>;
    fn exists(&self, path: impl AsRef<Path>) -> bool;

    fn create_dir_all(&mut self, _path: impl AsRef<Path>) -> anyhow::Result<()> {
        unimplemented!("create_dir_all() is not supported on this backend")
    }
    fn create_file(
        &mut self,
        _path: impl AsRef<Path>,
        _contents: Cow<'static, str>,
    ) -> anyhow::Result<()> {
        unimplemented!("create_file() is not supported on this backend")
    }
}

// ---

/// A [`FileSystem`] implementation that simply delegates to `std::fs`.
///
/// Used only for native debug builds, where shader hot-reloading is a thing.
#[derive(Default)]
pub struct OsFileSystem;

impl FileSystem for OsFileSystem {
    // fn read(&self, path: impl AsRef<Path>) -> anyhow::Result<Cow<'static, [u8]>> {
    //     let path = path.as_ref();
    //     std::fs::read(path)
    //         .with_context(|| format!("failed to read file at {path:?}"))
    //         .map(Into::into)
    // }

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
#[derive(Default)]
pub struct MemFileSystem {
    files: HashMap<PathBuf, Cow<'static, str>>,
}

impl FileSystem for MemFileSystem {
    // fn read(&self, path: impl AsRef<Path>) -> anyhow::Result<Cow<'static, [u8]>> {
    //     let path = path.as_ref().clean();
    //     self.files
    //         .get(&path)
    //         // NOTE: This is calling `Cow::clone`, which doesn't actually clone anything
    //         // if `self` is `Cow::Borrowed`!
    //         .cloned()
    //         // .map(|s| s.as_bytes().into())
    //         .ok_or_else(|| anyhow!("file does not exist at {path:?}"))
    // }

    fn read_to_string(&self, path: impl AsRef<Path>) -> anyhow::Result<Cow<'static, str>> {
        let path = path.as_ref().clean();
        self.files
            .get(&path)
            // NOTE: This is calling `Cow::clone`, which doesn't actually clone anything
            // if `self` is `Cow::Borrowed`!
            .cloned()
            .ok_or_else(|| anyhow!("file does not exist at {path:?}"))
    }

    fn canonicalize(&self, path: impl AsRef<Path>) -> anyhow::Result<PathBuf> {
        let path = path.as_ref().clean();
        ensure!(
            self.files.contains_key(&path),
            "file does not exist at {path:?}",
        );
        Ok(path)
    }

    fn exists(&self, path: impl AsRef<Path>) -> bool {
        self.files.contains_key(&path.as_ref().clean())
    }

    fn create_dir_all(&mut self, _: impl AsRef<Path>) -> anyhow::Result<()> {
        Ok(())
    }

    fn create_file(
        &mut self,
        path: impl AsRef<Path>,
        contents: Cow<'static, str>,
    ) -> anyhow::Result<()> {
        self.files.insert(path.as_ref().to_owned(), contents);
        Ok(())
    }
}
