use std::sync::Arc;

#[cfg(not(target_arch = "wasm32"))]
use anyhow::Context as _;
use re_log_channel::{LogReceiver, LogSource, RecordingOpenBehavior};
use re_log_types::RecordingId;
use re_redap_client::ConnectionRegistryHandle;

use crate::FileContents;
use crate::stream_rrd_from_http::stream_from_http_to_channel;

pub type AuthErrorHandler =
    Arc<dyn Fn(re_uri::DatasetSegmentUri, &re_redap_client::ClientCredentialsError) + Send + Sync>;

/// Somewhere we can get Rerun logging data from.
// TODO(emilk): there is a lot of overlap between this and `ViewerOpenUrl`
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
    FilePath {
        /// How we got to know about the file
        file_source: re_log_types::FileSource,

        /// Where the file is
        path: std::path::PathBuf,

        /// If `true`, keep reading `.rrd` files past EOF, tailing new data as it arrives.
        follow: bool,
    },

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

        open_behavior: RecordingOpenBehavior,
    },

    /// A `rerun+http://` URI pointing to a proxy.
    RedapProxy(re_uri::ProxyUri),
}

/// Options for [`LogDataSource::from_uri`].
#[derive(Clone, Debug, Default)]
pub struct FromUriOptions {
    /// If `true`, keep reading `.rrd` files past EOF, tailing new data as it arrives.
    pub follow: bool,

    /// If `true`, accept extensionless HTTP URLs for magic-bytes-based format detection.
    ///
    /// This should be `true` at external entry points (CLI, explicit user URL input),
    /// but `false` when parsing URLs from viewer-internal links, where extensionless
    /// URLs (e.g. `https://rerun.io/docs/getting-started/data-in`) should fall through to be opened in
    /// the browser.
    pub accept_extensionless_http: bool,
}

impl LogDataSource {
    /// Tries to classify a URI into a [`LogDataSource`].
    ///
    /// Tries to figure out if it looks like a local path,
    /// a web-socket address, a grpc url, a http url, etc.
    ///
    /// Note that not all URLs are log data sources!
    /// For instance a pure server or entry url is not a source of log data.
    pub fn from_uri(
        _file_source: re_log_types::FileSource,
        url: &str,
        options: &FromUriOptions,
    ) -> Option<Self> {
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
                return Some(Self::FilePath {
                    file_source: _file_source,
                    path,
                    follow: options.follow,
                });
            }

            if looks_like_a_file_path(url) {
                return Some(Self::FilePath {
                    file_source: _file_source,
                    path,
                    follow: options.follow,
                });
            }
        }

        if let Ok(uri) = url.parse::<re_uri::DatasetSegmentUri>() {
            Some(Self::RedapDatasetSegment {
                uri,
                open_behavior: RecordingOpenBehavior::OpenAndSelect,
            })
        } else if let Ok(uri) = url.parse::<re_uri::ProxyUri>() {
            Some(Self::RedapProxy(uri))
        } else {
            // Only do magic bytes loading if the url has a protocol
            // without this, `anything` or `xyz` would be a proper url we'd try to load from
            let mut was_proper_http_url = true;
            let url = url::Url::parse(url)
                .or_else(|_| {
                    was_proper_http_url = false;
                    url::Url::parse(&format!("http://{url}"))
                })
                .ok()?;

            // We can only load http/s urls, so don't try to load any other schemes
            if url.scheme() != "http" && url.scheme() != "https" {
                return None;
            }

            let path = url.path();
            let extension = std::path::Path::new(path)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");

            // If the url contains a `?url=…` param, it'll be parsed as a `ViewerOpenUrl` later
            // so don't try loading it as a `HttpUrl` if it doesn't have a file extension we know.
            let contains_viewer_query_url_param = url.query_pairs().any(|(key, _)| key == "url");

            if re_data_loader::is_supported_file_extension(extension) {
                Some(Self::HttpUrl { url, follow: false })
            } else if options.accept_extensionless_http
                && extension.is_empty()
                && was_proper_http_url
                && !contains_viewer_query_url_param
            {
                // No extension — accept the URL and try to detect format after download
                Some(Self::HttpUrl {
                    url,
                    follow: options.follow,
                })
            } else if contains_viewer_query_url_param {
                // This is a web viewer URL with a `?url=` parameter.
                // Extract the URL parameter and try to parse it as a redap URI.
                let (_, value) = url.query_pairs().find(|(key, _)| key == "url")?;
                if let Ok(uri) = value.parse::<re_uri::DatasetSegmentUri>() {
                    Some(Self::RedapDatasetSegment {
                        uri,
                        open_behavior: RecordingOpenBehavior::OpenAndSelect,
                    })
                } else if let Ok(uri) = value.parse::<re_uri::ProxyUri>() {
                    Some(Self::RedapProxy(uri))
                } else {
                    None
                }
            } else {
                None // Has an extension but it's not one we support
            }
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
        self.stream_with_options(
            on_auth_err,
            connection_registry,
            re_redap_client::StreamingOptions::default(),
        )
    }

    /// Like [`Self::stream`], but with additional options controlling streaming behavior.
    pub fn stream_with_options(
        self,
        on_auth_err: AuthErrorHandler,
        connection_registry: &ConnectionRegistryHandle,
        streaming_options: re_redap_client::StreamingOptions,
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
            Self::FilePath {
                file_source,
                path,
                follow,
            } => {
                let (tx, rx) = re_log_channel::log_channel(LogSource::File {
                    path: path.clone(),
                    follow,
                });

                // This recording will be communicated to all `DataLoader`s, which may or may not
                // decide to use it depending on whether they want to share a common recording
                // or not.
                let shared_recording_id = RecordingId::random();
                let settings = re_data_loader::DataLoaderSettings {
                    opened_store_id: file_source.recommended_store_id().cloned(),
                    force_store_info: file_source.force_store_info(),
                    follow,
                    ..re_data_loader::DataLoaderSettings::recommended(shared_recording_id)
                };
                re_data_loader::load_from_path(&settings, file_source, &path, &tx)
                    .with_context(|| format!("{path:?}"))?;

                Ok(rx)
            }

            // When loading a file on Web, or when using drag-n-drop.
            Self::FileContents(file_source, file_contents) => {
                let name = file_contents.name.clone();
                let (tx, rx) = re_log_channel::log_channel(LogSource::File {
                    path: name.clone().into(),
                    follow: false,
                });

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

            Self::RedapDatasetSegment { uri, open_behavior } => {
                let (tx, rx) =
                    re_log_channel::log_channel(re_log_channel::LogSource::RedapGrpcStream {
                        uri: uri.clone(),
                        open_behavior,
                    });

                let connection_registry = connection_registry.clone();
                let uri_clone = uri.clone();
                let tx_err = tx.clone();
                let stream_segment = async move {
                    let client = connection_registry.client(uri_clone.origin.clone()).await?;
                    re_redap_client::stream_blueprint_and_segment_from_server(
                        client,
                        tx,
                        uri_clone,
                        streaming_options,
                    )
                    .await
                };

                spawn_future(async move {
                    if let Err(err) = stream_segment.await {
                        if let Some(err) = err.as_client_credentials_error() {
                            on_auth_err(uri, err);
                        } else {
                            tx_err.quit(Some(Box::new(err))).ok();
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
            Self::FilePath {
                file_source, path, ..
            } => {
                let file_extension = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|s| s.to_lowercase());
                LogDataSourceAnalytics {
                    source_type: "file_path",
                    file_extension,
                    file_source: Some(Self::file_source_to_analytics_str(file_source)),
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

    /// Concert the data source to a URI string, if possible.
    pub fn as_uri(&self) -> Option<String> {
        match self {
            Self::HttpUrl { url, .. } => Some(url.to_string()),
            #[cfg(not(target_arch = "wasm32"))]
            Self::FilePath { path, .. } => Some(format!("file://{}", path.display())),
            Self::FileContents { .. } => None,
            #[cfg(not(target_arch = "wasm32"))]
            Self::Stdin => Some("-".to_owned()),
            Self::RedapDatasetSegment { uri, .. } => Some(uri.to_string()),
            Self::RedapProxy(uri) => Some(uri.to_string()),
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
            // Since the path has an explicit extension, this will be parsed as a DataSource and
            // not a `ViewerOpenUrl` (see invalid section below)
            "https://example.com/some-file.rrd?url=recording.rrd",
        ];
        // Extensionless URLs — only accepted when accept_extensionless_http is true
        let extensionless_http = [
            "https://example.com/download",
            "https://example.com/api/file?id=123",
            "https://storage.example.com/abc123?token=xyz",
            "https://example.com/files?my.id",
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

        let invalid = [
            // This will be ignored as a DataSource so it can later be parsed as a
            // `ViewerOpenUrl` (due to the ?url=)
            "https://example.com/some-file?url=recording.rrd",
            // Extensionless urls need a proper http protocol present, otherwise even `aaaa` would
            // be parsed as an http url.
            "example.com/some-file",
            "aaaa",
        ];

        let file_source = FileSource::DragAndDrop {
            recommended_store_id: None,
            force_store_info: false,
        };
        let default_options = FromUriOptions::default();
        let extensionless_options = FromUriOptions {
            accept_extensionless_http: true,
            ..Default::default()
        };

        for uri in file {
            let data_source = LogDataSource::from_uri(file_source.clone(), uri, &default_options);
            if !matches!(data_source, Some(LogDataSource::FilePath { .. })) {
                eprintln!(
                    "Expected {uri:?} to be categorized as FilePath. Instead it got parsed as {data_source:?}"
                );
                failed = true;
            }
        }

        for uri in http {
            let data_source = LogDataSource::from_uri(file_source.clone(), uri, &default_options);
            if !matches!(data_source, Some(LogDataSource::HttpUrl { .. })) {
                eprintln!(
                    "Expected {uri:?} to be categorized as HttpUrl. Instead it got parsed as {data_source:?}"
                );
                failed = true;
            }
        }

        // Extensionless URLs are accepted when accept_extensionless_http is true
        for uri in extensionless_http {
            let data_source =
                LogDataSource::from_uri(file_source.clone(), uri, &extensionless_options);
            if !matches!(data_source, Some(LogDataSource::HttpUrl { .. })) {
                eprintln!(
                    "Expected {uri:?} to be categorized as HttpUrl (with accept_extensionless_http=true). Instead it got parsed as {data_source:?}"
                );
                failed = true;
            }

            // …but rejected when accept_extensionless_http is false
            let data_source = LogDataSource::from_uri(file_source.clone(), uri, &default_options);
            if data_source.is_some() {
                eprintln!(
                    "Expected {uri:?} to be None (with accept_extensionless_http=false). Instead it got parsed as {data_source:?}"
                );
                failed = true;
            }
        }

        for uri in grpc {
            let data_source = LogDataSource::from_uri(file_source.clone(), uri, &default_options);
            if !matches!(data_source, Some(LogDataSource::RedapDatasetSegment { .. })) {
                eprintln!(
                    "Expected {uri:?} to be categorized as redap dataset segment. Instead it got parsed as {data_source:?}"
                );
                failed = true;
            }
        }

        for uri in proxy {
            let data_source = LogDataSource::from_uri(file_source.clone(), uri, &default_options);
            if !matches!(data_source, Some(LogDataSource::RedapProxy { .. })) {
                eprintln!(
                    "Expected {uri:?} to be categorized as MessageProxy. Instead it got parsed as {data_source:?}"
                );
                failed = true;
            }
        }

        for uri in invalid {
            let data_source =
                LogDataSource::from_uri(file_source.clone(), uri, &extensionless_options);
            if data_source.is_some() {
                eprintln!("Expected {uri:?} to be None. Instead it got parsed as {data_source:?}");
                failed = true;
            }
        }

        assert!(!failed, "one or more test cases failed");
    }

    #[test]
    fn test_data_source_from_viewer_url() {
        // This is the sort of url:s we get when sharing copying links from the web viewer:

        let url = "https://customer.cloud.rerun.io/?url=rerun%3A%2F%2Fapi.customer.cloud.rerun.io%3A443%2Fdataset%2F18A23D2FAC59F8572563b312ef21f53b%3Fsegment_id%3Dthe_segment_name";

        let data_source = LogDataSource::from_uri(FileSource::Cli, url, &FromUriOptions::default());
        assert_eq!(
            data_source,
            Some(LogDataSource::RedapDatasetSegment {
                uri: re_uri::DatasetSegmentUri {
                    origin: "api.customer.cloud.rerun.io:443".parse().unwrap(),
                    dataset_id: "18A23D2FAC59F8572563b312ef21f53b".parse().unwrap(),
                    segment_id: "the_segment_name".to_owned(),
                    fragment: Default::default(),
                },
                open_behavior: RecordingOpenBehavior::OpenAndSelect,
            })
        );
    }
}
