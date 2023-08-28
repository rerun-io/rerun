use anyhow::Context as _;

use re_log_types::LogMsg;
use re_smart_channel::Receiver;

/// The contents of as file
#[derive(Clone)]
pub struct FileContents {
    pub file_name: String,
    pub bytes: std::sync::Arc<[u8]>,
}

/// Somewhere we can get Rerun data from.
#[derive(Clone)]
pub enum DataSource {
    /// A remote RRD file, served over http.
    RrdHttpUrl(String),

    /// A path to a local file.
    #[cfg(not(target_arch = "wasm32"))]
    FilePath(std::path::PathBuf),

    /// The contents of a file.
    ///
    /// This is what you get when loading a file on Web.
    FileContents(FileContents),

    /// A remote Rerun server.
    WebSocketAddr(String),
}

impl DataSource {
    /// Tried to classify a URI into a `DataSource`.
    ///
    /// Tried to figure out if it looks like a local path,
    /// a web-socket address, or a http url.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_uri(mut uri: String) -> DataSource {
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
                true // Only one part. Weird. Let's assume it is a file path.
            } else if parts.len() == 2 {
                let extension = parts[1];
                matches!(
                    extension,
                    // Our own:
                    "rrd"

                    // Misc:
                    | "txt"
                    | "zip"

                    // Meshes:
                    | "glb"
                    | "gltf"
                    | "obj"
                    | "ply"
                    | "stl"

                    // Images:
                    | "avif"
                    | "bmp"
                    | "dds"
                    | "exr"
                    | "farbfeld"
                    | "ff"
                    | "gif"
                    | "hdr"
                    | "ico"
                    | "jpeg"
                    | "jpg"
                    | "pam"
                    | "pbm"
                    | "pgm"
                    | "png"
                    | "ppm"
                    | "tga"
                    | "tif"
                    | "tiff"
                    | "webp"
                )
            } else {
                false // Too many dots; assume an url
            }
        }

        let path = std::path::Path::new(&uri).to_path_buf();

        if uri.starts_with("file://") || path.exists() {
            DataSource::FilePath(path)
        } else if uri.starts_with("http://")
            || uri.starts_with("https://")
            || (uri.starts_with("www.") && uri.ends_with(".rrd"))
        {
            DataSource::RrdHttpUrl(uri)
        } else if uri.starts_with("ws://") || uri.starts_with("wss://") {
            DataSource::WebSocketAddr(uri)

        // Now we are into heuristics territory:
        } else if looks_like_a_file_path(&uri) {
            DataSource::FilePath(path)
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
    /// Will do minimal checks (e.g. that the file exists), for syncronous errors,
    /// but the loading is done in a task.
    pub fn stream(self) -> anyhow::Result<Receiver<LogMsg>> {
        match self {
            DataSource::RrdHttpUrl(url) => {
                Ok(re_log_encoding::stream_rrd_from_http::stream_rrd_from_http_to_channel(url))
            }

            #[cfg(not(target_arch = "wasm32"))]
            DataSource::FilePath(path) => {
                let (tx, rx) = re_smart_channel::smart_channel(
                    re_smart_channel::SmartMessageSource::File(path.clone()),
                    re_smart_channel::SmartChannelSource::File { path: path.clone() },
                );
                let store_id = re_log_types::StoreId::random(re_log_types::StoreKind::Recording);
                load_file_to_channel_at(store_id, &path, tx)
                    .with_context(|| format!("{path:?}"))?;
                Ok(rx)
            }

            DataSource::FileContents(file_contents) => {
                let name = &file_contents.file_name;
                let (tx, rx) = re_smart_channel::smart_channel(
                    re_smart_channel::SmartMessageSource::File(name.clone().into()),
                    re_smart_channel::SmartChannelSource::File {
                        path: name.clone().into(),
                    },
                );
                let store_id = re_log_types::StoreId::random(re_log_types::StoreKind::Recording);
                load_file_contents_to_channel_at(store_id, &file_contents, tx)
                    .with_context(|| format!("{name:?}"))?;
                Ok(rx)
            }

            DataSource::WebSocketAddr(rerun_server_ws_url) => {
                connect_to_ws_url(&rerun_server_ws_url)
            }
        }
    }
}

/// Stream .rrd files, but loads other fiels syncronously (blocking).
#[cfg(not(target_arch = "wasm32"))]
#[allow(clippy::needless_pass_by_value)] // false positive on some feature flags
fn load_file_to_channel_at(
    store_id: re_log_types::StoreId,
    path: &std::path::Path,
    tx: re_smart_channel::Sender<LogMsg>,
) -> Result<(), anyhow::Error> {
    re_tracing::profile_function!(path.to_string_lossy());
    re_log::info!("Loading {path:?}…");

    let extension = path
        .extension()
        .unwrap_or_default()
        .to_ascii_lowercase()
        .to_string_lossy()
        .to_string();

    if extension == "rrd" {
        stream_rrd_file(path.to_owned(), tx)
    } else {
        #[cfg(feature = "sdk")]
        {
            use re_log_types::SetStoreInfo;
            // First, set a store info since this is the first thing the application expects.
            tx.send(LogMsg::SetStoreInfo(SetStoreInfo {
                row_id: re_log_types::RowId::random(),
                info: re_log_types::StoreInfo {
                    application_id: re_log_types::ApplicationId(path.display().to_string()),
                    store_id: store_id.clone(),
                    is_official_example: false,
                    started: re_log_types::Time::now(),
                    store_source: re_log_types::StoreSource::FileFromCli {
                        rustc_version: env!("RE_BUILD_RUSTC_VERSION").into(),
                        llvm_version: env!("RE_BUILD_LLVM_VERSION").into(),
                    },
                    store_kind: re_log_types::StoreKind::Recording,
                },
            }))
            .ok(); // .ok(): we may be running in a background thread, so who knows if the receiver is still open

            // Send actual file.
            tx.send(re_sdk::MsgSender::from_file_path(path)?.into_log_msg(store_id)?)
                .ok();

            tx.quit(None).ok();
            Ok(())
        }

        #[cfg(not(feature = "sdk"))]
        {
            _ = store_id;
            anyhow::bail!("Unsupported file extension: '{extension}' for path {path:?}. Try enabling the 'sdk' feature of 'rerun'.");
        }
    }
}

#[allow(clippy::needless_pass_by_value)] // false positive on some feature flags
fn load_file_contents_to_channel_at(
    _store_id: re_log_types::StoreId,
    file_contents: &FileContents,
    tx: re_smart_channel::Sender<LogMsg>,
) -> Result<(), anyhow::Error> {
    let file_name = &file_contents.file_name;
    re_tracing::profile_function!(file_name);
    re_log::info!("Loading {file_name:?}…");

    if file_name.ends_with(".rrd") {
        // TODO: background thread on native
        let bytes: &[u8] = &file_contents.bytes;
        let decoder = re_log_encoding::decoder::Decoder::new(bytes)?;
        for msg in decoder {
            tx.send(msg?)?;
        }
        re_log::debug!("Finished loading {file_name:?}.");
        Ok(())
    } else {
        // TODO: support images and meshes
        anyhow::bail!("Unsupported file extension for {file_name:?}.");
    }
}

// Non-blocking
#[cfg(not(target_arch = "wasm32"))]
fn stream_rrd_file(
    path: std::path::PathBuf,
    tx: re_smart_channel::Sender<LogMsg>,
) -> anyhow::Result<()> {
    let file = std::fs::File::open(&path).context("Failed to open file")?;
    let decoder = re_log_encoding::decoder::Decoder::new(file)?;

    rayon::spawn(move || {
        re_tracing::profile_scope!("load_rrd_file_to_channel");
        for msg in decoder {
            match msg {
                Ok(msg) => {
                    tx.send(msg).ok(); // .ok(): we're running in a background thread, so who knows if the receiver is still open
                }
                Err(err) => {
                    re_log::warn_once!("Failed to decode message in {path:?}: {err}");
                }
            }
        }
        tx.quit(None).ok(); // .ok(): we're running in a background thread, so who knows if the receiver is still open
    });

    Ok(())
}

fn connect_to_ws_url(url: &str) -> anyhow::Result<Receiver<LogMsg>> {
    let (tx, rx) = re_smart_channel::smart_channel(
        re_smart_channel::SmartMessageSource::WsClient {
            ws_server_url: url.to_owned(),
        },
        re_smart_channel::SmartChannelSource::WsClient {
            ws_server_url: url.to_owned(),
        },
    );

    re_log::info!("Connecting to WS server at {:?}…", url);

    let callback = move |binary: Vec<u8>| match re_ws_comms::decode_log_msg(&binary) {
        Ok(log_msg) => {
            if tx.send(log_msg).is_ok() {
                std::ops::ControlFlow::Continue(())
            } else {
                re_log::info!("Failed to send log message to viewer - closing");
                std::ops::ControlFlow::Break(())
            }
        }
        Err(err) => {
            re_log::error!("Failed to parse message: {err}");
            std::ops::ControlFlow::Break(())
        }
    };

    let connection = re_ws_comms::Connection::viewer_to_server(url.to_owned(), callback)?;
    std::mem::drop(connection); // Never close the connection. TODO: is this wise?
    Ok(rx)
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn test_data_source_from_uri() {
    let file = [
        "file://foo",
        "foo.rrd",
        "foo.zip",
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

    for uri in file {
        assert!(
            matches!(
                DataSource::from_uri(uri.to_owned()),
                DataSource::FilePath(_)
            ),
            "Expected {uri:?} to be categorized as FilePath"
        );
    }

    for uri in http {
        assert!(
            matches!(
                DataSource::from_uri(uri.to_owned()),
                DataSource::RrdHttpUrl(_)
            ),
            "Expected {uri:?} to be categorized as RrdHttpUrl"
        );
    }

    for uri in ws {
        assert!(
            matches!(
                DataSource::from_uri(uri.to_owned()),
                DataSource::WebSocketAddr(_)
            ),
            "Expected {uri:?} to be categorized as WebSocketAddr"
        );
    }
}
