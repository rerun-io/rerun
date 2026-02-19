use std::sync::Arc;

#[cfg(not(target_arch = "wasm32"))]
use anyhow::Context as _;
use re_log_channel::{LogReceiver, LogSource};
use re_log_types::RecordingId;
use re_redap_client::ConnectionRegistryHandle;

use crate::FileContents;
use crate::stream_rrd_from_http::stream_from_http_to_channel;

pub type AuthErrorHandler =
    Arc<dyn Fn(re_uri::DatasetSegmentUri, &re_redap_client::ClientCredentialsError) + Send + Sync>;

/// Somewhere we can get Rerun logging data from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LogDataSource {
    /// A remote file, served over http.
    ///
    /// Could be an `.rrd` recording, `.rbl` blueprint, `.mcap`, `.png`, `.glb`, etc.
    HttpUrl {
        /// This is a canonicalized URL path without any parameters or fragments.
        url: url::Url,

        /// If `follow` is `true`, the viewer will open the stream in `Following` mode rather than `Playing` mode.
        follow: bool,
    },

    /// A path to a local file.
    #[cfg(not(target_arch = "wasm32"))]
    FilePath(re_log_types::FileSource, std::path::PathBuf),

    /// The contents of a file.
    ///
    /// This is what you get when loading a file on Web, or when using drag-n-drop.
    FileContents(re_log_types::FileSource, FileContents),

    // RRD data streaming in from standard input.
    #[cfg(not(target_arch = "wasm32"))]
    Stdin,

    /// A `rerun://` URI pointing to a recording.
    RedapDatasetSegment {
        uri: re_uri::DatasetSegmentUri,

        /// Switch to this recording once it has been loaded?
        select_when_loaded: bool,
    },

    /// A `rerun+http://` URI pointing to a proxy.
    RedapProxy(re_uri::ProxyUri),
}

impl LogDataSource {
    /// Tried to classify a URI into a [`LogDataSource`].
    ///
    /// Tries to figure out if it looks like a local path,
    /// a web-socket address, a grpc url, a http url, etc.
    ///
    /// Note that not all URLs are log data sources!
    /// For instance a pure server or entry url is not a source of log data.
    pub fn from_uri(_file_source: re_log_types::FileSource, url: &str) -> Option<Self> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            use itertools::Itertools as _;

            fn looks_like_windows_abs_path(path: &str) -> bool {
                let path = path.as_bytes();
                // "C:/" etc
                path.get(1).copied() == Some(b':') && path.get(2).copied() == Some(b'/')
            }

            fn looks_like_a_file_path(uri: &str) -> bool {
                // Files must have a supported extension.
                let Some(file_extension) = uri.split('.').next_back() else {
                    return false;
                };
                if !re_data_loader::is_supported_file_extension(file_extension) {
                    return false;
                }

                #[expect(clippy::if_same_then_else)]
                if uri.starts_with('/') {
                    true // Unix absolute path
                } else if uri.starts_with("./") || uri.starts_with("../") {
                    true // Unix relative path
                } else if looks_like_windows_abs_path(uri) {
                    true
                } else if uri.starts_with("http:") || uri.starts_with("https:") {
                    false
                } else {
                    // We use a simple heuristic here: if there are multiple dots, it is likely an url,
                    // like "example.com/foo.zip".
                    // If there is only one dot, we treat it as an extension and look it up in a list of common
                    // file extensions:

                    let parts = uri.split('.').collect_vec();
                    if parts.len() == 2 {
                        true
                    } else {
                        false // Too many dots; assume an url
                    }
                }
            }

            // Reading from standard input in non-TTY environments (e.g. GitHub Actions, but I'm sure we can
            // come up with more convoluted than that…) can lead to many unexpected,
            // platform-specific problems that aren't even necessarily consistent across runs.
            //
            // In order to avoid having to swallow errors based on unreliable heuristics (or inversely:
            // throwing errors when we shouldn't), we just make reading from standard input explicit.
            if url == "-" {
                return Some(Self::Stdin);
            }

            let path = std::path::Path::new(url).to_path_buf();

            if url.starts_with("file://") || path.exists() {
                return Some(Self::FilePath(_file_source, path));
            }

            if looks_like_a_file_path(url) {
                return Some(Self::FilePath(_file_source, path));
            }
        }

        if let Ok(uri) = url.parse::<re_uri::DatasetSegmentUri>() {
            Some(Self::RedapDatasetSegment {
                uri,
                select_when_loaded: true,
            })
        } else if let Ok(uri) = url.parse::<re_uri::ProxyUri>() {
            Some(Self::RedapProxy(uri))
        } else {
            let url = url::Url::parse(url)
                .or_else(|_| url::Url::parse(&format!("http://{url}")))
                .ok()?;
            let path = url.path();
            let extension = std::path::Path::new(path)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");

            re_data_loader::is_supported_file_extension(extension)
                .then_some(Self::HttpUrl { url, follow: false })
        }
    }

    /// Stream the data from the given data source.
    ///
    /// Will do minimal checks (e.g. that the file exists), for synchronous errors,
    /// but the loading is done in a background task.
    ///
    /// `on_redap_err` should handle authentication errors by showing a login prompt.
    pub fn stream(
        self,
        on_auth_err: AuthErrorHandler,
        connection_registry: &ConnectionRegistryHandle,
    ) -> anyhow::Result<LogReceiver> {
        re_tracing::profile_function!();

        match self {
            Self::HttpUrl { url, follow } => {
                let path = url.path();
                let is_rrd = path.ends_with(".rrd") || path.ends_with(".rbl");
                if is_rrd {
                    Ok(stream_from_http_to_channel(url.to_string(), follow))
                } else {
                    Ok(crate::fetch_file_from_http::fetch_and_load(&url))
                }
            }

            #[cfg(not(target_arch = "wasm32"))]
            Self::FilePath(file_source, path) => {
                let (tx, rx) = re_log_channel::log_channel(LogSource::File(path.clone()));

                // This recording will be communicated to all `DataLoader`s, which may or may not
                // decide to use it depending on whether they want to share a common recording
                // or not.
                let shared_recording_id = RecordingId::random();
                let settings = re_data_loader::DataLoaderSettings {
                    opened_store_id: file_source.recommended_store_id().cloned(),
                    force_store_info: file_source.force_store_info(),
                    ..re_data_loader::DataLoaderSettings::recommended(shared_recording_id)
                };
                re_data_loader::load_from_path(&settings, file_source, &path, &tx)
                    .with_context(|| format!("{path:?}"))?;

                Ok(rx)
            }

            // When loading a file on Web, or when using drag-n-drop.
            Self::FileContents(file_source, file_contents) => {
                let name = file_contents.name.clone();
                let (tx, rx) = re_log_channel::log_channel(LogSource::File(name.clone().into()));

                // This `StoreId` will be communicated to all `DataLoader`s, which may or may not
                // decide to use it depending on whether they want to share a common recording
                // or not.
                let shared_recording_id = RecordingId::random();
                let settings = re_data_loader::DataLoaderSettings {
                    opened_store_id: file_source.recommended_store_id().cloned(),
                    force_store_info: file_source.force_store_info(),
                    ..re_data_loader::DataLoaderSettings::recommended(shared_recording_id)
                };
                re_data_loader::load_from_file_contents(
                    &settings,
                    file_source,
                    &std::path::PathBuf::from(file_contents.name),
                    std::borrow::Cow::Borrowed(&file_contents.bytes),
                    &tx,
                )?;

                Ok(rx)
            }

            #[cfg(not(target_arch = "wasm32"))]
            Self::Stdin => {
                let (tx, rx) = re_log_channel::log_channel(LogSource::Stdin);

                crate::load_stdin::load_stdin(tx).with_context(|| "stdin".to_owned())?;

                Ok(rx)
            }

            Self::RedapDatasetSegment {
                uri,
                select_when_loaded,
            } => {
                let (tx, rx) =
                    re_log_channel::log_channel(re_log_channel::LogSource::RedapGrpcStream {
                        uri: uri.clone(),
                        select_when_loaded,
                    });

                let connection_registry = connection_registry.clone();
                let uri_clone = uri.clone();
                let stream_segment = async move {
                    let client = connection_registry.client(uri_clone.origin.clone()).await?;
                    re_redap_client::stream_blueprint_and_segment_from_server(client, tx, uri_clone)
                        .await
                };

                spawn_future(async move {
                    if let Err(err) = stream_segment.await {
                        if let Some(err) = err.as_client_credentials_error() {
                            on_auth_err(uri, err);
                        } else {
                            re_log::warn!("Error while streaming: {}", re_error::format_ref(&err));
                        }
                    }
                });
                Ok(rx)
            }

            Self::RedapProxy(uri) => Ok(re_grpc_client::stream(uri)),
        }
    }

    /// Returns analytics data for this data source.
    pub fn analytics(&self) -> LogDataSourceAnalytics {
        match self {
            Self::HttpUrl { url, .. } => {
                let file_extension = std::path::Path::new(url.path())
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|s| s.to_lowercase());
                LogDataSourceAnalytics {
                    source_type: "http_url",
                    file_extension,
                    file_source: None,
                }
            }

            #[cfg(not(target_arch = "wasm32"))]
            Self::FilePath(file_src, path) => {
                let file_extension = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|s| s.to_lowercase());
                LogDataSourceAnalytics {
                    source_type: "file_path",
                    file_extension,
                    file_source: Some(Self::file_source_to_analytics_str(file_src)),
                }
            }

            Self::FileContents(file_src, file_contents) => {
                let file_extension = std::path::Path::new(&file_contents.name)
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|s| s.to_lowercase());
                LogDataSourceAnalytics {
                    source_type: "file_contents",
                    file_extension,
                    file_source: Some(Self::file_source_to_analytics_str(file_src)),
                }
            }

            #[cfg(not(target_arch = "wasm32"))]
            Self::Stdin => LogDataSourceAnalytics {
                source_type: "stdin",
                file_extension: None,
                file_source: None,
            },

            Self::RedapDatasetSegment { .. } => LogDataSourceAnalytics {
                source_type: "redap_dataset_segment",
                file_extension: None,
                file_source: None,
            },

            Self::RedapProxy(_) => LogDataSourceAnalytics {
                source_type: "redap_proxy",
                file_extension: None,
                file_source: None,
            },
        }
    }

    fn file_source_to_analytics_str(file_source: &re_log_types::FileSource) -> &'static str {
        use re_log_types::FileSource;
        match file_source {
            FileSource::Cli => "cli",
            FileSource::Uri => "uri",
            FileSource::DragAndDrop { .. } => "drag_and_drop",
            FileSource::FileDialog { .. } => "file_dialog",
            FileSource::Sdk => "sdk",
        }
    }
}

/// Analytics data extracted from a [`LogDataSource`].
#[derive(Clone, Debug)]
pub struct LogDataSourceAnalytics {
    /// The type of data source (e.g., "file", "http", ``redap_grpc``, "stdin").
    pub source_type: &'static str,

    /// The file extension if applicable (e.g., "rrd", "png", "glb").
    pub file_extension: Option<String>,

    /// How the file was opened (e.g., "cli", ``file_dialog``, ``drag_and_drop``).
    /// Only applicable for file-based sources.
    pub file_source: Option<&'static str>,
}

// TODO(ab, andreas): This should be replaced by the use of `AsyncRuntimeHandle`. However, this
// requires:
// - `AsyncRuntimeHandle` to be moved lower in the crate hierarchy to be available here (unsure
//   where).
// - Make sure that all callers of `DataSource::stream` have access to an `AsyncRuntimeHandle`
//   (maybe it should be in `AppContext`?).
#[cfg(target_arch = "wasm32")]
fn spawn_future<F>(future: F)
where
    F: std::future::Future<Output = ()> + 'static,
{
    wasm_bindgen_futures::spawn_local(future);
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn_future<F>(future: F)
where
    F: std::future::Future<Output = ()> + 'static + Send,
{
    tokio::spawn(future);
}

#[cfg(test)]
mod tests {
    use re_log_types::FileSource;

    use super::*;

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_data_source_from_uri() {
        let mut failed = false;

        let file = [
            "file://foo",
            "foo.rrd",
            "foo.png",
            "/foo/bar/baz.rbl",
            "D:/file.jpg",
        ];
        let http = [
            "http://example.com/foo.rrd",
            "https://example.com/foo.rrd",
            "http://example.com/foo.rrd?useless_param=1",
            "example.zip/foo.rrd",
            "www.foo.zip/foo.rrd",
            "www.foo.zip/blueprint.rbl",
            "https://example.com/recording.mcap",
            "https://example.com/scene.glb",
            "https://example.com/photo.png",
            "https://example.com/video.mp4",
        ];
        let grpc = [
            // segment_id (new)
            "rerun://127.0.0.1:1234/dataset/1830B33B45B963E7774455beb91701ae/data?segment_id=sid",
            "rerun://127.0.0.1:1234/dataset/1830B33B45B963E7774455beb91701ae/data?segment_id=sid&time_range=timeline@1230ms..1m12s",
            "rerun+http://example.com/dataset/1830B33B45B963E7774455beb91701ae/data?segment_id=sid",
            // partition_id (legacy, for backward compatibility)
            "rerun://127.0.0.1:1234/dataset/1830B33B45B963E7774455beb91701ae/data?partition_id=pid",
        ];

        let proxy = [
            "rerun+http://127.0.0.1:9876/proxy",
            "rerun+https://127.0.0.1:9876/proxy",
            "rerun+http://example.com/proxy",
        ];

        let file_source = FileSource::DragAndDrop {
            recommended_store_id: None,
            force_store_info: false,
        };

        for uri in file {
            let data_source = LogDataSource::from_uri(file_source.clone(), uri);
            if !matches!(data_source, Some(LogDataSource::FilePath { .. })) {
                eprintln!(
                    "Expected {uri:?} to be categorized as FilePath. Instead it got parsed as {data_source:?}"
                );
                failed = true;
            }
        }

        for uri in http {
            let data_source = LogDataSource::from_uri(file_source.clone(), uri);
            if !matches!(data_source, Some(LogDataSource::HttpUrl { .. })) {
                eprintln!(
                    "Expected {uri:?} to be categorized as HttpUrl. Instead it got parsed as {data_source:?}"
                );
                failed = true;
            }
        }

        for uri in grpc {
            let data_source = LogDataSource::from_uri(file_source.clone(), uri);
            if !matches!(data_source, Some(LogDataSource::RedapDatasetSegment { .. })) {
                eprintln!(
                    "Expected {uri:?} to be categorized as redap dataset segment. Instead it got parsed as {data_source:?}"
                );
                failed = true;
            }
        }

        for uri in proxy {
            let data_source = LogDataSource::from_uri(file_source.clone(), uri);
            if !matches!(data_source, Some(LogDataSource::RedapProxy { .. })) {
                eprintln!(
                    "Expected {uri:?} to be categorized as MessageProxy. Instead it got parsed as {data_source:?}"
                );
                failed = true;
            }
        }

        assert!(!failed, "one or more test cases failed");
    }
}
