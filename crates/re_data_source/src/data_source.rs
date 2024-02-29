use re_log_types::LogMsg;
use re_smart_channel::{Receiver, SmartChannelSource, SmartMessageSource};

use crate::FileContents;

#[cfg(not(target_arch = "wasm32"))]
use anyhow::Context as _;

/// Somewhere we can get Rerun data from.
#[derive(Debug, Clone)]
pub enum DataSource {
    /// A remote RRD file, served over http.
    RrdHttpUrl(String),

    /// A path to a local file.
    #[cfg(not(target_arch = "wasm32"))]
    FilePath(re_log_types::FileSource, std::path::PathBuf),

    /// The contents of a file.
    ///
    /// This is what you get when loading a file on Web, or when using drag-n-drop.
    FileContents(re_log_types::FileSource, FileContents),

    /// A remote Rerun server.
    WebSocketAddr(String),

    // RRD data streaming in from standard input.
    #[cfg(not(target_arch = "wasm32"))]
    Stdin,
}

impl DataSource {
    /// Tried to classify a URI into a [`DataSource`].
    ///
    /// Tries to figure out if it looks like a local path,
    /// a web-socket address, or a http url.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_uri(file_source: re_log_types::FileSource, mut uri: String) -> DataSource {
        use itertools::Itertools as _;

        fn looks_like_windows_abs_path(path: &str) -> bool {
            let path = path.as_bytes();
            // "C:/" etc
            path.get(1).copied() == Some(b':') && path.get(2).copied() == Some(b'/')
        }

        fn looks_like_a_file_path(uri: &str) -> bool {
            // How do we distinguish a file path from a web url? "example.zip" could be either.

            if uri.starts_with('/') {
                return true; // Unix absolute path
            }
            if looks_like_windows_abs_path(uri) {
                return true;
            }

            // We use a simple heuristic here: if there are multiple dots, it is likely an url,
            // like "example.com/foo.zip".
            // If there is only one dot, we treat it as an extension and look it up in a list of common
            // file extensions:

            let parts = uri.split('.').collect_vec();
            if parts.len() <= 1 {
                true // No dots. Weird. Let's assume it is a file path.
            } else if parts.len() == 2 {
                // Extension or `.com` etc?
                crate::is_supported_file_extension(parts[1])
            } else {
                false // Too many dots; assume an url
            }
        }

        // Reading from standard input in non-TTY environments (e.g. GitHub Actions, but I'm sure we can
        // come up with more convoluted than that…) can lead to many unexpected,
        // platform-specific problems that aren't even necessarily consistent across runs.
        //
        // In order to avoid having to swallow errors based on unreliable heuristics (or inversely:
        // throwing errors when we shouldn't), we just make reading from standard input explicit.
        if uri == "-" {
            return DataSource::Stdin;
        }

        let path = std::path::Path::new(&uri).to_path_buf();

        if uri.starts_with("file://") || path.exists() {
            DataSource::FilePath(file_source, path)
        } else if uri.starts_with("http://")
            || uri.starts_with("https://")
            || (uri.starts_with("www.") && uri.ends_with(".rrd"))
        {
            DataSource::RrdHttpUrl(uri)
        } else if uri.starts_with("ws://") || uri.starts_with("wss://") {
            DataSource::WebSocketAddr(uri)

        // Now we are into heuristics territory:
        } else if looks_like_a_file_path(&uri) {
            DataSource::FilePath(file_source, path)
        } else if uri.ends_with(".rrd") {
            DataSource::RrdHttpUrl(uri)
        } else {
            // If this is sometyhing like `foo.com` we can't know what it is until we connect to it.
            // We could/should connect and see what it is, but for now we just take a wild guess instead:
            re_log::debug!("Assuming WebSocket endpoint");
            if !uri.contains("://") {
                uri = format!("{}://{uri}", re_ws_comms::PROTOCOL);
            }
            DataSource::WebSocketAddr(uri)
        }
    }

    /// Stream the data from the given data source.
    ///
    /// Will do minimal checks (e.g. that the file exists), for synchronous errors,
    /// but the loading is done in a background task.
    ///
    /// `on_msg` can be used to wake up the UI thread on Wasm.
    pub fn stream(
        self,
        on_msg: Option<Box<dyn Fn() + Send + Sync>>,
    ) -> anyhow::Result<Receiver<LogMsg>> {
        re_tracing::profile_function!();
        match self {
            DataSource::RrdHttpUrl(url) => Ok(
                re_log_encoding::stream_rrd_from_http::stream_rrd_from_http_to_channel(url, on_msg),
            ),

            #[cfg(not(target_arch = "wasm32"))]
            DataSource::FilePath(file_source, path) => {
                let (tx, rx) = re_smart_channel::smart_channel(
                    SmartMessageSource::File(path.clone()),
                    SmartChannelSource::File(path.clone()),
                );

                // This `StoreId` will be communicated to all `DataLoader`s, which may or may not
                // decide to use it depending on whether they want to share a common recording
                // or not.
                let shared_store_id =
                    re_log_types::StoreId::random(re_log_types::StoreKind::Recording);
                let settings = crate::DataLoaderSettings::recommended(shared_store_id);
                crate::load_from_path(&settings, file_source, &path, &tx)
                    .with_context(|| format!("{path:?}"))?;

                if let Some(on_msg) = on_msg {
                    on_msg();
                }

                Ok(rx)
            }

            // When loading a file on Web, or when using drag-n-drop.
            DataSource::FileContents(file_source, file_contents) => {
                let name = file_contents.name.clone();
                let (tx, rx) = re_smart_channel::smart_channel(
                    SmartMessageSource::File(name.clone().into()),
                    SmartChannelSource::File(name.clone().into()),
                );

                // This `StoreId` will be communicated to all `DataLoader`s, which may or may not
                // decide to use it depending on whether they want to share a common recording
                // or not.
                let shared_store_id =
                    re_log_types::StoreId::random(re_log_types::StoreKind::Recording);
                let settings = crate::DataLoaderSettings::recommended(shared_store_id);
                crate::load_from_file_contents(
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

            DataSource::WebSocketAddr(rerun_server_ws_url) => {
                crate::web_sockets::connect_to_ws_url(&rerun_server_ws_url, on_msg)
            }

            #[cfg(not(target_arch = "wasm32"))]
            DataSource::Stdin => {
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
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn test_data_source_from_uri() {
    use re_log_types::FileSource;

    let file = [
        "file://foo",
        "foo.rrd",
        "foo.png",
        "/foo/bar/baz",
        "D:/file",
    ];
    let http = [
        "http://foo.zip",
        "https://foo.zip",
        "example.zip/foo.rrd",
        "www.foo.zip/foo.rrd",
    ];
    let ws = ["ws://foo.zip", "wss://foo.zip", "127.0.0.1"];

    let file_source = FileSource::DragAndDrop;

    for uri in file {
        assert!(
            matches!(
                DataSource::from_uri(file_source, uri.to_owned()),
                DataSource::FilePath { .. }
            ),
            "Expected {uri:?} to be categorized as FilePath"
        );
    }

    for uri in http {
        assert!(
            matches!(
                DataSource::from_uri(file_source, uri.to_owned()),
                DataSource::RrdHttpUrl(_)
            ),
            "Expected {uri:?} to be categorized as RrdHttpUrl"
        );
    }

    for uri in ws {
        assert!(
            matches!(
                DataSource::from_uri(file_source, uri.to_owned()),
                DataSource::WebSocketAddr(_)
            ),
            "Expected {uri:?} to be categorized as WebSocketAddr"
        );
    }
}
