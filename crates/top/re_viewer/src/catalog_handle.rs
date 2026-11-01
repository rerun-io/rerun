#[cfg(not(target_arch = "wasm32"))]
use anyhow::Context as _;
use re_async::AsyncRuntimeHandle;
use re_catalog_client::CatalogClient;
use re_protos::cloud::v1alpha1::ext::ObjectKey;
use re_redap_client::ConnectionRegistryHandle;

/// The Viewer-owned catalog that runs in this process.
#[derive(Clone, Debug)]
pub struct InternalCatalog {
    catalog_client: CatalogClient,

    /// Where it keeps the files it owns.
    storage_dir: std::path::PathBuf,
}

impl InternalCatalog {
    /// Makes `file` available to the catalog and returns its credential-free URL.
    ///
    /// On native, the catalog reads the file in place instead of copying it.
    /// In the browser, it reuses an existing OPFS copy with the same fingerprint and size.
    #[cfg_attr(
        not(target_arch = "wasm32"),
        expect(
            clippy::unused_async,
            reason = "the shared interface is asynchronous because the wasm implementation awaits OPFS"
        )
    )]
    pub async fn stage_file(
        &self,
        #[cfg_attr(not(target_arch = "wasm32"), expect(unused_variables))]
        fingerprint: re_log_encoding::RrdFingerprint,
        #[cfg(not(target_arch = "wasm32"))] file: std::path::PathBuf,
        #[cfg(target_arch = "wasm32")] file: web_sys::File,
    ) -> anyhow::Result<url::Url> {
        cfg_select! {
            target_arch = "wasm32" => {
                write_to_opfs(&self.storage_dir, object_key(fingerprint)?, file).await
            }
            _ => {
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
        }
    }

    /// The internal catalog, if it is running.
    pub fn new(registry: &ConnectionRegistryHandle, runtime: AsyncRuntimeHandle) -> Option<Self> {
        Some(Self {
            catalog_client: CatalogClient::new(registry.internal_connection_handle()?, runtime),
            storage_dir: registry.internal_storage_dir()?.to_owned(),
        })
    }

    pub fn client(&self) -> &CatalogClient {
        &self.catalog_client
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
        write_internal_bytes(&self.storage_dir, &key, bytes).await
    }
}

/// The key under which the internal catalog stores its copy of an RRD.
///
/// Derived from the RRD fingerprint, so re-opening the same file addresses the existing object.
#[cfg(target_arch = "wasm32")]
fn object_key(fingerprint: re_log_encoding::RrdFingerprint) -> anyhow::Result<ObjectKey> {
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
