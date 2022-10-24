use std::path::{Path, PathBuf};

use ahash::HashMap;
use anyhow::{anyhow, Context as _};
use clean_path::Clean as _;

// ---

pub trait FileSystem {
    fn read(&self, path: impl AsRef<Path>) -> anyhow::Result<Vec<u8>>;
    fn read_to_string(&self, path: impl AsRef<Path>) -> anyhow::Result<String>;
    fn canonicalize(&self, path: impl AsRef<Path>) -> anyhow::Result<PathBuf>;
    fn exists(&self, path: impl AsRef<Path>) -> bool;

    fn create_dir_all(&mut self, _path: impl AsRef<Path>) -> anyhow::Result<()> {
        unimplemented!("not supported")
    }
    fn create_file(
        &mut self,
        _path: impl AsRef<Path>,
        _buf: impl AsRef<[u8]>,
    ) -> anyhow::Result<()> {
        unimplemented!("not supported")
    }
}

// ---

#[derive(Default)]
pub struct OsFileSystem;

impl FileSystem for OsFileSystem {
    fn read(&self, path: impl AsRef<Path>) -> anyhow::Result<Vec<u8>> {
        let path = path.as_ref();
        std::fs::read(path).with_context(|| format!("failed to read file at {path:?}"))
    }

    fn read_to_string(&self, path: impl AsRef<Path>) -> anyhow::Result<String> {
        let path = path.as_ref();
        std::fs::read_to_string(path).with_context(|| format!("failed to read file at {path:?}"))
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

// TODO: need a Cow in there
#[derive(Default)]
pub struct MemFileSystem {
    files: HashMap<PathBuf, Vec<u8>>,
}

impl FileSystem for MemFileSystem {
    fn read(&self, path: impl AsRef<Path>) -> anyhow::Result<Vec<u8>> {
        let path = path.as_ref().clean();
        self.files
            .get(&path)
            .cloned()
            .ok_or_else(|| anyhow!("file does not exist"))
            .with_context(|| format!("failed to read file at {path:?}"))
    }

    fn read_to_string(&self, path: impl AsRef<Path>) -> anyhow::Result<String> {
        let path = path.as_ref().clean();
        let file = self
            .files
            .get(&path)
            .ok_or_else(|| anyhow!("file does not exist"))
            .with_context(|| format!("failed to read file at {path:?}"))?;
        String::from_utf8(file.clone()).with_context(|| format!("failed to read file at {path:?}"))
    }

    fn canonicalize(&self, path: impl AsRef<Path>) -> anyhow::Result<PathBuf> {
        let path = path.as_ref().clean();
        self.files
            .get(&path)
            .ok_or_else(|| anyhow!("file does not exist at {path:?}"))
            .map(|_| path)
        // .with_context(|| format!("failed to canonicalize path at {path:?}"))
    }

    fn exists(&self, path: impl AsRef<Path>) -> bool {
        self.files.contains_key(&path.as_ref().clean())
    }

    fn create_dir_all(&mut self, _: impl AsRef<Path>) -> anyhow::Result<()> {
        Ok(())
    }

    fn create_file(&mut self, path: impl AsRef<Path>, buf: impl AsRef<[u8]>) -> anyhow::Result<()> {
        self.files
            .insert(path.as_ref().to_owned(), buf.as_ref().to_owned());
        Ok(())
    }
}
