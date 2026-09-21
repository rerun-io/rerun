use anyhow::Context as _;
use re_protos::cloud::v1alpha1::ext::ObjectKey;
use re_redap_client::{ConnectionHandle, ConnectionRegistryHandle};

/// A [`ConnectionHandle`] that knows whether its origin is the internal catalog.
///
/// The distinction matters for writes: the internal catalog runs in-process, so it can read a
/// local file where it already lies on native and out of OPFS in the browser, while a remote
/// catalog hands out write access grants (see [`ConnectionHandle::write_object`]).
#[derive(Clone, Debug)]
pub enum CatalogHandle {
    Internal {
        connection: ConnectionHandle,
        storage_dir: std::path::PathBuf,
    },
    Remote(ConnectionHandle),
}

impl CatalogHandle {
    /// The connection to `origin`, tagged with whether it is the internal catalog.
    #[expect(dead_code, reason = "remote catalog registration is not wired up yet")]
    pub fn for_origin(registry: &ConnectionRegistryHandle, origin: &re_uri::Origin) -> Self {
        let connection = registry.connection_handle(origin.clone());
        if registry.is_internal_origin(origin)
            && let Some(storage_dir) = registry.internal_storage_dir()
        {
            Self::Internal {
                connection,
                storage_dir: storage_dir.to_owned(),
            }
        } else {
            Self::Remote(connection)
        }
    }

    /// The internal catalog, if it is running.
    pub fn internal(registry: &ConnectionRegistryHandle) -> Option<Self> {
        Some(Self::Internal {
            connection: registry.internal_connection_handle()?,
            storage_dir: registry.internal_storage_dir()?.to_owned(),
        })
    }

    pub fn connection(&self) -> &ConnectionHandle {
        match self {
            Self::Internal { connection, .. } | Self::Remote(connection) => connection,
        }
    }

    /// Makes `file` available to the catalog and returns its credential-free URL.
    ///
    /// On native, the internal catalog reads the file in place instead of copying it.
    pub async fn write_file(
        &self,
        #[cfg(not(target_arch = "wasm32"))] file: std::path::PathBuf,
        #[cfg(target_arch = "wasm32")] file: web_sys::File,
    ) -> anyhow::Result<url::Url> {
        cfg_select! {
            target_arch = "wasm32" => {
                let source = re_web::fs::File::from(file.clone());
                let key = object_key(&source, &file.name()).await?;
                match self {
                    Self::Internal { storage_dir, .. } => {
                        write_to_opfs(storage_dir, key, file).await
                    }
                    // TODO(RR-5489, RR-5490): Implement `GetWriteAccessGrant` on the server.
                    Self::Remote(connection) => {
                        connection.write_object(key, source).await.map_err(|err| {
                            anyhow::anyhow!(
                                "failed to upload file to {}: {err}",
                                connection.origin()
                            )
                        })
                    }
                }
            }
            _ => {
                match self {
                    // The catalog runs in this process, so it reads the file where it already is.
                    Self::Internal { .. } => {
                        let file = std::path::absolute(&file).with_context(|| {
                            format!(
                                "failed to resolve absolute path\nFile path: {}",
                                file.display()
                            )
                        })?;
                        url::Url::from_file_path(&file).map_err(|()| {
                            anyhow::anyhow!(
                                "failed to create file URL\nFile path: {}",
                                file.display()
                            )
                        })
                    }
                    // TODO(RR-5489, RR-5490): Implement `GetWriteAccessGrant` on the server.
                    Self::Remote(connection) => {
                        let source = std::fs::File::open(&file).map_err(|err| {
                            anyhow::anyhow!(
                                "failed to open file for upload: {err}\nFile path: {}",
                                file.display()
                            )
                        })?;
                        let key = object_key(&source, &file.to_string_lossy()).await?;
                        connection.write_object(key, source).await.map_err(|err| {
                            anyhow::anyhow!(
                                "failed to upload file to {}: {err}",
                                connection.origin()
                            )
                        })
                    }
                }
            }
        }
    }

    /// Makes `bytes` available under `key` and returns its credential-free URL.
    ///
    /// Existing objects are replaced.
    #[expect(dead_code, reason = "no one is using this yet")]
    pub async fn write_bytes(
        &self,
        key: ObjectKey,
        bytes: bytes::Bytes,
    ) -> anyhow::Result<url::Url> {
        match self {
            Self::Internal { storage_dir, .. } => {
                write_internal_bytes(storage_dir, &key, bytes).await
            }
            Self::Remote(connection) => anyhow::bail!(
                "writing bytes to remote catalog {} is not implemented",
                connection.origin()
            ),
        }
    }
}

/// The key under which a catalog stores its copy of an RRD.
///
/// Derived from the RRD fingerprint, so re-opening the same file addresses the existing object.
async fn object_key(source: &impl re_async::AsyncReadAt, name: &str) -> anyhow::Result<ObjectKey> {
    let fingerprint = re_log_encoding::RrdFingerprint::compute_for_rrd(source)
        .await
        .with_context(|| format!("failed to fingerprint RRD\nFile path: {name}"))?;
    let fingerprint = re_chunk_index::sha256_to_hex(fingerprint.as_bytes());
    Ok(ObjectKey::try_new(format!(
        "uploads/{fingerprint}/recording.rrd"
    ))?)
}

/// Copies `file` into the OPFS object addressed by `key`, unless it is already there.
#[cfg(target_arch = "wasm32")]
async fn write_to_opfs(
    storage_dir: &std::path::Path,
    key: ObjectKey,
    file: web_sys::File,
) -> anyhow::Result<url::Url> {
    let file_url = internal_object_url(storage_dir, &key)?;
    let size = file.size() as u64;

    match existing_object_len(&file_url).await? {
        // Keys are content fingerprints, so an object of the expected size is that object.
        Some(len) if len == size => return Ok(file_url),
        Some(len) => re_log::warn!(
            "Overwriting `{}`: expected {size} bytes, found {len}",
            file_url.path()
        ),
        None => {}
    }

    re_log::info!("Writing file to OPFS…");
    let path = std::path::Path::new(file_url.path());
    map_opfs_write_error(re_web::fs::write_file(path, file).await)?;
    Ok(file_url)
}

/// The size of the object at `file_url`, or `None` if there is nothing there.
#[cfg(target_arch = "wasm32")]
async fn existing_object_len(file_url: &url::Url) -> anyhow::Result<Option<u64>> {
    match re_web::fs::metadata(std::path::Path::new(file_url.path())).await {
        Ok(metadata) => Ok(metadata.is_file().then(|| metadata.len())),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err.into()),
    }
}

#[cfg_attr(
    not(target_arch = "wasm32"),
    expect(
        clippy::unused_async,
        reason = "the shared interface is asynchronous because the wasm implementation awaits OPFS"
    )
)]
async fn write_internal_bytes(
    storage_dir: &std::path::Path,
    key: &ObjectKey,
    bytes: bytes::Bytes,
) -> anyhow::Result<url::Url> {
    let file_url = internal_object_url(storage_dir, key)?;

    cfg_select! {
        target_arch = "wasm32" => {
            if existing_object_len(&file_url).await?.is_some() {
                re_log::warn!("Overwriting `{}`", file_url.path());
            }
            map_opfs_write_error(re_web::fs::write_bytes(file_url.path(), bytes).await)?;
        }
        _ => {
            use std::io::Write as _;

            let path = storage_dir.join(key.to_string());
            if path.exists() {
                re_log::warn!("Overwriting `{}`", path.display());
            }
            let parent = path.parent().ok_or_else(|| {
                anyhow::anyhow!("object path has no parent\nFile path: {}", path.display())
            })?;
            std::fs::create_dir_all(parent)?;

            let mut staged = tempfile::NamedTempFile::new_in(parent)?;
            staged.write_all(&bytes)?;
            staged.persist(path)?;
        }
    }

    Ok(file_url)
}

fn internal_object_url(storage_dir: &std::path::Path, key: &ObjectKey) -> anyhow::Result<url::Url> {
    let path = storage_dir.join(key.to_string());
    cfg_select! {
        target_arch = "wasm32" => {
            let path_str = path.to_str().ok_or_else(|| {
                anyhow::anyhow!(
                    "file path is not valid UTF-8\nFile path: {}",
                    path.display()
                )
            })?;
            let mut url = url::Url::parse("file:///")?;
            url.set_path(path_str);
            Ok(url)
        }
        _ => url::Url::from_file_path(&path).map_err(|()| {
            anyhow::anyhow!("not an absolute file path\nFile path: {}", path.display())
        }),
    }
}

#[cfg(target_arch = "wasm32")]
fn map_opfs_write_error(result: std::io::Result<()>) -> anyhow::Result<()> {
    result.map_err(|err| {
        if err.kind() == std::io::ErrorKind::StorageFull {
            anyhow::Error::new(err).context(
                "Viewer catalog storage quota exceeded. In Settings, under Origin private filesystem, select \"Request persistence\", then try again.",
            )
        } else {
            err.into()
        }
    })
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn internal_bytes_replace_existing_object() {
        let storage_dir = tempfile::tempdir().expect("temporary directory should be created");
        let key = ObjectKey::try_new("generated/nested/object.bin").expect("key should be valid");

        let url = write_internal_bytes(
            storage_dir.path(),
            &key,
            bytes::Bytes::from_static(b"first"),
        )
        .await
        .expect("initial write should succeed");
        let path = url.to_file_path().expect("URL should name a file");
        assert_eq!(
            std::fs::read(&path).expect("object should be readable"),
            b"first"
        );

        write_internal_bytes(
            storage_dir.path(),
            &key,
            bytes::Bytes::from_static(b"other"),
        )
        .await
        .expect("same-size replacement should succeed");
        assert_eq!(
            std::fs::read(path).expect("object should be readable"),
            b"other"
        );
    }
}
