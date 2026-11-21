//! [`KnownServers`]: the catalog servers the user has added, shared by every Rerun program on this
//! machine.

use std::io::Write as _;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use re_uri::Origin;

/// How long a change waits for another program to finish its own.
///
/// Changes take milliseconds, so a lock held this long belongs to a stuck program, and the viewer
/// changes the list on its UI thread.
const LOCK_TIMEOUT: Duration = Duration::from_secs(2);

/// How often a waiting change checks the lock again.
const LOCK_RETRY_INTERVAL: Duration = Duration::from_millis(10);

/// Why the list of known catalog servers could not be read or written.
#[derive(Debug, thiserror::Error)]
pub enum KnownServersError {
    #[error("Failed to read the catalog server list: {source}. Path: {path:?}")]
    Read {
        source: std::io::Error,
        path: PathBuf,
    },

    #[error(
        "The catalog server list is not valid JSON; fix or delete it: {source}. Path: {path:?}"
    )]
    Parse {
        source: serde_json::Error,
        path: PathBuf,
    },

    #[error("Failed to encode the catalog server list: {source}")]
    Encode { source: serde_json::Error },

    #[error("Failed to write the catalog server list: {source}. Path: {path:?}")]
    Write {
        source: std::io::Error,
        path: PathBuf,
    },

    #[error(
        "Another program has been changing the catalog server list for over {} s. Path: {path:?}",
        waited.as_secs()
    )]
    Locked { path: PathBuf, waited: Duration },
}

/// The file format: URLs rather than parsed origins, so that it stays editable by hand.
///
/// Fields and entries this version does not know are kept as they are, so that programs of
/// different versions can share the file.
#[derive(Default, serde::Serialize, serde::Deserialize)]
struct ServersFile {
    servers: Vec<ServerEntry>,

    #[serde(flatten)]
    other: serde_json::Map<String, serde_json::Value>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct ServerEntry {
    /// E.g. `rerun://example.com:443`.
    url: String,

    #[serde(flatten)]
    other: serde_json::Map<String, serde_json::Value>,
}

impl ServerEntry {
    fn new(origin: &Origin) -> Self {
        Self {
            url: origin.to_string(),
            other: serde_json::Map::new(),
        }
    }

    /// `None` for a URL this version cannot parse, e.g. one with a newer scheme.
    fn origin(&self) -> Option<Origin> {
        self.url.parse().ok()
    }
}

/// The catalog servers the user has added, in a file that the viewer, the `rerun` CLI, and
/// `rerun mcp` all read and update, so a server added in one is known to the others.
///
/// Changes take a file lock and re-read the file first, so programs that change the list at the
/// same time add to each other's changes rather than overwrite them.
#[derive(Clone, Debug)]
pub struct KnownServers {
    path: PathBuf,

    /// How long a change waits for another program to finish its own.
    lock_timeout: Duration,
}

impl KnownServers {
    /// The list in Rerun's config directory, next to the stored login.
    ///
    /// `None` when there is no home directory to find it in.
    pub fn in_config_dir() -> Option<Self> {
        re_auth::rerun_config_dir().map(|dir| Self::at(dir.join("servers.json")))
    }

    /// The list in the file at `path`.
    pub fn at(path: PathBuf) -> Self {
        Self {
            path,
            lock_timeout: LOCK_TIMEOUT,
        }
    }

    /// The known servers, in the order they were added. Empty if the file does not exist yet.
    ///
    /// Leaves out URLs it cannot parse, with a warning, rather than fail for the whole list.
    pub fn load(&self) -> Result<Vec<Origin>, KnownServersError> {
        re_tracing::profile_function!();
        Ok(self
            .read()?
            .servers
            .iter()
            .filter_map(|entry| {
                let origin = entry.origin();
                if origin.is_none() {
                    re_log::warn_once!(
                        "Skipping the catalog server {:?} in {:?}: not a valid URL",
                        entry.url,
                        self.path
                    );
                }
                origin
            })
            .collect())
    }

    /// Add `origin` to the list. Returns whether it was new.
    pub fn add(&self, origin: &Origin) -> Result<bool, KnownServersError> {
        Ok(self.add_all(std::slice::from_ref(origin))? == 1)
    }

    /// Add each of `origins` that is not on the list yet. Returns how many were new.
    pub fn add_all(&self, origins: &[Origin]) -> Result<usize, KnownServersError> {
        let _lock = self.lock()?;
        let mut file = self.read()?;
        let mut num_added = 0;
        for origin in origins {
            if !file
                .servers
                .iter()
                .any(|entry| entry.origin().as_ref() == Some(origin))
            {
                file.servers.push(ServerEntry::new(origin));
                num_added += 1;
            }
        }
        if 0 < num_added {
            self.store(&file)?;
        }
        Ok(num_added)
    }

    /// Remove `origin` from the list. Returns whether it was there.
    pub fn remove(&self, origin: &Origin) -> Result<bool, KnownServersError> {
        let _lock = self.lock()?;
        let mut file = self.read()?;
        let num_before = file.servers.len();
        file.servers
            .retain(|entry| entry.origin().as_ref() != Some(origin));
        if file.servers.len() == num_before {
            return Ok(false);
        }
        self.store(&file)?;
        Ok(true)
    }

    /// The file's contents. Empty if it does not exist yet, or is empty.
    fn read(&self) -> Result<ServersFile, KnownServersError> {
        let data = match std::fs::read_to_string(&self.path) {
            Ok(data) => data,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(source) => {
                return Err(KnownServersError::Read {
                    source,
                    path: self.path.clone(),
                });
            }
        };
        if data.trim().is_empty() {
            return Ok(ServersFile::default());
        }
        serde_json::from_str(&data).map_err(|source| KnownServersError::Parse {
            source,
            path: self.path.clone(),
        })
    }

    /// Wait for other programs to finish changing the list. Released when dropped.
    fn lock(&self) -> Result<std::fs::File, KnownServersError> {
        let write_error = |source| KnownServersError::Write {
            source,
            path: self.path.clone(),
        };
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).map_err(write_error)?;
        }
        let lock_file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(self.path.with_extension("json.lock"))
            .map_err(write_error)?;
        let start = Instant::now();
        loop {
            match lock_file.try_lock() {
                Ok(()) => return Ok(lock_file),
                Err(std::fs::TryLockError::WouldBlock) => {
                    if self.lock_timeout <= start.elapsed() {
                        return Err(KnownServersError::Locked {
                            path: self.path.clone(),
                            waited: self.lock_timeout,
                        });
                    }
                    std::thread::sleep(LOCK_RETRY_INTERVAL);
                }
                Err(std::fs::TryLockError::Error(source)) => return Err(write_error(source)),
            }
        }
    }

    /// Replace the file in one step, so that a reader never sees half of it, nor an empty file
    /// after a crash.
    ///
    /// Callers hold [`Self::lock`], so no other program writes the temporary file meanwhile.
    fn store(&self, file: &ServersFile) -> Result<(), KnownServersError> {
        re_tracing::profile_function!();
        let data = serde_json::to_string_pretty(file)
            .map_err(|source| KnownServersError::Encode { source })?;
        let temporary = self.path.with_extension("json.tmp");
        let result = std::fs::File::create(&temporary)
            .and_then(|mut file| {
                file.write_all(data.as_bytes())?;
                file.sync_all()
            })
            .and_then(|()| std::fs::rename(&temporary, &self.path));
        if result.is_err() {
            std::fs::remove_file(&temporary).ok();
        }
        result.map_err(|source| KnownServersError::Write {
            source,
            path: self.path.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn origin(url: &str) -> Origin {
        url.parse().expect("valid origin")
    }

    fn read_json(path: &std::path::Path) -> serde_json::Value {
        serde_json::from_str(&std::fs::read_to_string(path).expect("read")).expect("json")
    }

    #[test]
    fn adds_and_removes_servers_in_order() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("config").join("servers.json");
        let servers = KnownServers::at(path.clone());
        assert_eq!(servers.load().expect("load"), []);

        let first = origin("rerun+https://first.example.com");
        let second = origin("rerun+http://127.0.0.1:51234");
        assert!(servers.add(&first).expect("add"));
        assert!(servers.add(&second).expect("add"));
        assert!(!servers.add(&first).expect("add again"));
        assert_eq!(
            servers.load().expect("load"),
            [first.clone(), second.clone()]
        );

        // Another program sees the same list.
        let other = KnownServers::at(path);
        assert!(other.remove(&first).expect("remove"));
        assert!(!other.remove(&first).expect("remove again"));
        assert_eq!(servers.load().expect("load"), [second]);
    }

    #[test]
    fn the_file_holds_urls() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("servers.json");
        KnownServers::at(path.clone())
            .add(&origin("rerun+https://example.com"))
            .expect("add");
        assert_eq!(
            read_json(&path),
            serde_json::json!({ "servers": [{ "url": "rerun://example.com:443" }] })
        );
    }

    #[test]
    fn concurrent_changes_are_all_kept() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("servers.json");
        let origins: Vec<Origin> = (0..16)
            .map(|i| origin(&format!("rerun+http://127.0.0.1:{}", 50000 + i)))
            .collect();
        std::thread::scope(|scope| {
            for origin in &origins {
                // The adds take turns, each with a full flush to disk, so the last one may wait
                // longer than a viewer should on a slow CI machine.
                let servers = KnownServers {
                    lock_timeout: Duration::from_mins(1),
                    ..KnownServers::at(path.clone())
                };
                scope.spawn(move || servers.add(origin).expect("add"));
            }
        });
        let mut loaded = KnownServers::at(path).load().expect("load");
        loaded.sort();
        let mut expected = origins;
        expected.sort();
        assert_eq!(loaded, expected);
    }

    #[test]
    fn a_broken_file_is_an_error_that_says_what_to_do() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("servers.json");
        std::fs::write(&path, "not json").expect("write");
        let err = KnownServers::at(path.clone())
            .load()
            .expect_err("broken file");
        assert_eq!(
            err.to_string(),
            format!(
                "The catalog server list is not valid JSON; fix or delete it: expected ident at \
                 line 1 column 2. Path: {path:?}"
            )
        );
    }

    #[test]
    fn an_empty_file_is_an_empty_list() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("servers.json");
        std::fs::write(&path, "").expect("write");
        assert_eq!(KnownServers::at(path).load().expect("load"), []);
    }

    #[test]
    fn unknown_urls_and_fields_survive_changes() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("servers.json");
        let newer = serde_json::json!({
            "servers": [{ "url": "ftp://newer.example.com", "name": "Newer" }],
            "version": 2,
        });
        std::fs::write(&path, newer.to_string()).expect("write");

        let servers = KnownServers::at(path.clone());
        assert_eq!(servers.load().expect("load"), []);
        let added = origin("rerun+https://example.com");
        servers.add(&added).expect("add");
        servers.remove(&added).expect("remove");
        assert_eq!(read_json(&path), newer);
    }
}
