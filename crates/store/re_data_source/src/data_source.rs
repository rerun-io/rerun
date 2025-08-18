use re_grpc_client::ConnectionRegistryHandle;
use re_log_types::{LogMsg, RecordingId};
use re_smart_channel::{Receiver, SmartChannelSource, SmartMessageSource};

use crate::FileContents;

#[cfg(not(target_arch = "wasm32"))]
use anyhow::Context as _;

/// Somewhere we can get Rerun logging data from.
#[derive(Clone, Debug)]
pub enum LogDataSource {
    /// A remote RRD file, served over http.
    ///
    /// Could be either an `.rrd` recording or a `.rbl` blueprint.
    RrdHttpUrl {
        /// This is a canonicalized URL path without any parameters or fragments.
        url: String,

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
    RedapDatasetPartition {
        uri: re_uri::DatasetPartitionUri,

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

        if let Ok(uri) = url.parse::<re_uri::DatasetPartitionUri>() {
            Some(Self::RedapDatasetPartition {
                uri,
                select_when_loaded: true,
            })
        } else if let Ok(uri) = url.parse::<re_uri::ProxyUri>() {
            Some(Self::RedapProxy(uri))
        } else {
            let mut parsed_url = url::Url::parse(url)
                .or_else(|_| url::Url::parse(&format!("http://{url}")))
                .ok()?;

            // Ignore any parameters, we don't support them for http urls.
            parsed_url.set_query(None);
            let url = parsed_url.to_string();
            (url.ends_with(".rrd") || url.ends_with(".rbl"))
                .then_some(Self::RrdHttpUrl { url, follow: false })
        }
    }

    /// Stream the data from the given data source.
    ///
    /// Will do minimal checks (e.g. that the file exists), for synchronous errors,
    /// but the loading is done in a background task.
    ///
    /// `on_cmd` is used to respond to UI commands.
    ///
    /// `on_msg` can be used to wake up the UI thread on Wasm.
    pub fn stream(
        self,
        connection_registry: &ConnectionRegistryHandle,
        on_ui_cmd: Option<Box<dyn Fn(re_grpc_client::UiCommand) + Send + Sync>>,
        on_msg: Option<Box<dyn Fn() + Send + Sync>>,
    ) -> anyhow::Result<Receiver<LogMsg>> {
        re_tracing::profile_function!();

        match self {
            Self::RrdHttpUrl { url, follow } => Ok(
                re_log_encoding::stream_rrd_from_http::stream_rrd_from_http_to_channel(
                    url, follow, on_msg,
                ),
            ),

            #[cfg(not(target_arch = "wasm32"))]
            Self::FilePath(file_source, path) => {
                let (tx, rx) = re_smart_channel::smart_channel(
                    SmartMessageSource::File(path.clone()),
                    SmartChannelSource::File(path.clone()),
                );

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

                if let Some(on_msg) = on_msg {
                    on_msg();
                }

                Ok(rx)
            }

            // When loading a file on Web, or when using drag-n-drop.
            Self::FileContents(file_source, file_contents) => {
                let name = file_contents.name.clone();
                let (tx, rx) = re_smart_channel::smart_channel(
                    SmartMessageSource::File(name.clone().into()),
                    SmartChannelSource::File(name.clone().into()),
                );

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

                if let Some(on_msg) = on_msg {
                    on_msg();
                }

                Ok(rx)
            }

            #[cfg(not(target_arch = "wasm32"))]
            Self::Stdin => {
                let (tx, rx) = re_smart_channel::smart_channel(
                    SmartMessageSource::Stdin,
                    SmartChannelSource::Stdin,
                );

                crate::load_stdin::load_stdin(tx).with_context(|| "stdin".to_owned())?;

                if let Some(on_msg) = on_msg {
                    on_msg();
                }

                Ok(rx)
            }

            Self::RedapDatasetPartition {
                uri,
                select_when_loaded,
            } => {
                let (tx, rx) = re_smart_channel::smart_channel(
                    re_smart_channel::SmartMessageSource::RedapGrpcStream {
                        uri: uri.clone(),
                        select_when_loaded,
                    },
                    re_smart_channel::SmartChannelSource::RedapGrpcStream {
                        uri: uri.clone(),
                        select_when_loaded,
                    },
                );

                let connection_registry = connection_registry.clone();
                let uri_clone = uri.clone();
                let stream_partition = async move {
                    let client = connection_registry.client(uri_clone.origin.clone()).await?;
                    re_grpc_client::stream_blueprint_and_partition_from_server(
                        client, tx, uri_clone, on_ui_cmd, on_msg,
                    )
                    .await
                };

                spawn_future(async move {
                    if let Err(err) = stream_partition.await {
                        re_log::warn!(
                            "Error while streaming {uri}: {}",
                            re_error::format_ref(&err)
                        );
                    }
                });
                Ok(rx)
            }

            Self::RedapProxy(uri) => Ok(re_grpc_client::message_proxy::stream(uri, on_msg)),
        }
    }
}

// TODO(ab, andreas): This should be replaced by the use of `AsyncRuntimeHandle`. However, this
// requires:
// - `AsyncRuntimeHandle` to be moved lower in the crate hierarchy to be available here (unsure
//   where).
// - Make sure that all callers of `DataSource::stream` have access to an `AsyncRuntimeHandle`
//   (maybe it should be in `GlobalContext`?).
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
    use super::*;
    use re_log_types::FileSource;

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
        ];
        let grpc = [
            "rerun://127.0.0.1:1234/dataset/1830B33B45B963E7774455beb91701ae/data?partition_id=pid",
            "rerun://127.0.0.1:1234/dataset/1830B33B45B963E7774455beb91701ae/data?partition_id=pid&time_range=timeline@1230ms..1m12s",
            "rerun+http://example.com/dataset/1830B33B45B963E7774455beb91701ae/data?partition_id=pid",
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
            if !matches!(data_source, Some(LogDataSource::RrdHttpUrl { .. })) {
                eprintln!(
                    "Expected {uri:?} to be categorized as RrdHttpUrl. Instead it got parsed as {data_source:?}"
                );
                failed = true;
            }
        }

        for uri in grpc {
            let data_source = LogDataSource::from_uri(file_source.clone(), uri);
            if !matches!(
                data_source,
                Some(LogDataSource::RedapDatasetPartition { .. })
            ) {
                eprintln!(
                    "Expected {uri:?} to be categorized as readp dataset. Instead it got parsed as {data_source:?}"
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
