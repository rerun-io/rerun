use std::path::{Path, PathBuf};

use tempfile::TempDir;

// --

/// Gets removed on `Drop` by default. Call [`TempPath::keep`] to prevent that.
pub struct TempPath {
    dir: Option<TempDir>, // option so we can `take()` in `Drop`
    path: PathBuf,
    delete_on_drop: bool,
}

impl TempPath {
    pub fn new(dir: TempDir, path: PathBuf) -> Self {
        Self {
            dir: Some(dir),
            path,
            delete_on_drop: true,
        }
    }

    pub fn keep(&mut self) {
        self.delete_on_drop = false;
    }

    pub fn as_path(&self) -> &Path {
        self.path.as_path()
    }
}

impl std::ops::Drop for TempPath {
    fn drop(&mut self) {
        let dir = self.dir.take().expect("directory expected to be Some");
        if self.delete_on_drop {
            _ = dir.close();
        } else {
            _ = dir.keep();
        }
    }
}

impl std::ops::Deref for TempPath {
    type Target = PathBuf;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.path
    }
}
