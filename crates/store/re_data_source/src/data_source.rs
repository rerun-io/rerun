use crate::FileContents;
use re_log_types::LogMsg;
use re_smart_channel::{Receiver, SmartChannelSource, SmartMessageSource};

#[cfg(not(target_arch = "wasm32"))]
use anyhow::Context as _;

/// Somewhere we can get Rerun data from.
#[derive(Debug, Clone)]
pub enum DataSource {
    /// A remote RRD file, served over http.
    ///
    /// If `follow` is `true`, the viewer will open the stream in `Following` mode rather than `Playing` mode.
    ///
    /// Could be either an `.rrd` recording or a `.rbl` blueprint.
    RrdHttpUrl { url: String, follow: bool },

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

    /// A file or a metadata catalog on a Rerun Data Platform server,
    /// over `rerun://` gRPC interface.
    #[cfg(feature = "grpc")]
    RerunGrpcUrl { url: String },

    /// A stream of messages over gRPC, relayed from the SDK.
    MessageProxy { url: String },
}

// TODO(#9058): Temporary hack, see issue for how to fix this.
pub enum StreamSource {
    LogMessages(Receiver<LogMsg>),
    CatalogData {
        origin: re_grpc_client::redap::Origin,
    },
}

impl DataSource {
    /// Tried to classify a URI into a [`DataSource`].
    ///
    /// Tries to figure out if it looks like a local path,
    /// a web-socket address, or a http url.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_uri(file_source: re_log_types::FileSource, mut uri: String) -> Self {
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
                re_data_loader::is_supported_file_extension(parts[1])
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
            return Self::Stdin;
        }

        let path = std::path::Path::new(&uri).to_path_buf();

        #[cfg(feature = "grpc")]
        if uri.starts_with("rerun://")
            || uri.starts_with("rerun+http://")
            || uri.starts_with("rerun+https://")
        {
            return Self::RerunGrpcUrl { url: uri };
        }

        if uri.starts_with("file://") || path.exists() {
            Self::FilePath(file_source, path)
        } else if (uri.starts_with("http://") || uri.starts_with("https://"))
            && (uri.ends_with(".rrd") || uri.ends_with(".rbl"))
        {
            Self::RrdHttpUrl {
                url: uri,
                follow: false,
            }
        } else if uri.starts_with("http://") || uri.starts_with("https://") {
            Self::MessageProxy { url: uri }
        } else if looks_like_a_file_path(&uri) {
            Self::FilePath(file_source, path)
        } else if uri.ends_with(".rrd") || uri.ends_with(".rbl") {
            Self::RrdHttpUrl {
                url: uri,
                follow: false,
            }
        } else {
            // If this is sometyhing like `foo.com` we can't know what it is until we connect to it.
            // We could/should connect and see what it is, but for now we just take a wild guess instead:
            re_log::debug!("Assuming gRPC endpoint");
            if !uri.contains("://") {
                // TODO(jan): this should be `https` if it's not localhost, anything hosted over public network
                //            should be going through https, anyway.
                uri = format!("http://{uri}");
            }
            Self::MessageProxy { url: uri }
        }
    }

    pub fn file_name(&self) -> Option<String> {
        match self {
            Self::RrdHttpUrl { url, .. } => url.split('/').last().map(|r| r.to_owned()),
            #[cfg(not(target_arch = "wasm32"))]
            Self::FilePath(_, path) => path.file_name().map(|s| s.to_string_lossy().to_string()),
            Self::FileContents(_, file_contents) => Some(file_contents.name.clone()),
            #[cfg(not(target_arch = "wasm32"))]
            Self::Stdin => None,
            #[cfg(feature = "grpc")]
            Self::RerunGrpcUrl { .. } => None, // TODO(jleibs): This needs to come from the server.
            Self::MessageProxy { .. } => None,
        }
    }

    pub fn is_blueprint(&self) -> Option<bool> {
        self.file_name().map(|name| name.ends_with(".rbl"))
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
    ) -> anyhow::Result<StreamSource> {
        re_tracing::profile_function!();

        match self {
            Self::RrdHttpUrl { url, follow } => Ok(StreamSource::LogMessages(
                re_log_encoding::stream_rrd_from_http::stream_rrd_from_http_to_channel(
                    url, follow, on_msg,
                ),
            )),

            #[cfg(not(target_arch = "wasm32"))]
            Self::FilePath(file_source, path) => {
                let (tx, rx) = re_smart_channel::smart_channel(
                    SmartMessageSource::File(path.clone()),
                    SmartChannelSource::File(path.clone()),
                );

                // This `StoreId` will be communicated to all `DataLoader`s, which may or may not
                // decide to use it depending on whether they want to share a common recording
                // or not.
                let shared_store_id =
                    re_log_types::StoreId::random(re_log_types::StoreKind::Recording);
                let settings = re_data_loader::DataLoaderSettings {
                    opened_application_id: file_source.recommended_application_id().cloned(),
                    opened_store_id: file_source.recommended_recording_id().cloned(),
                    force_store_info: file_source.force_store_info(),
                    ..re_data_loader::DataLoaderSettings::recommended(shared_store_id)
                };
                re_data_loader::load_from_path(&settings, file_source, &path, &tx)
                    .with_context(|| format!("{path:?}"))?;

                if let Some(on_msg) = on_msg {
                    on_msg();
                }

                Ok(StreamSource::LogMessages(rx))
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
                let shared_store_id =
                    re_log_types::StoreId::random(re_log_types::StoreKind::Recording);
                let settings = re_data_loader::DataLoaderSettings {
                    opened_application_id: file_source.recommended_application_id().cloned(),
                    opened_store_id: file_source.recommended_recording_id().cloned(),
                    force_store_info: file_source.force_store_info(),
                    ..re_data_loader::DataLoaderSettings::recommended(shared_store_id)
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

                Ok(StreamSource::LogMessages(rx))
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

                Ok(StreamSource::LogMessages(rx))
            }

            #[cfg(feature = "grpc")]
            Self::RerunGrpcUrl { url } => {
                use re_grpc_client::redap::RedapAddress;

                re_log::debug!("Loading {url}…");

                let address = url.as_str().try_into()?;

                match address {
                    RedapAddress::Recording {
                        origin,
                        recording_id,
                    } => {
                        let (tx, rx) = re_smart_channel::smart_channel(
                            re_smart_channel::SmartMessageSource::RerunGrpcStream {
                                url: url.clone(),
                            },
                            re_smart_channel::SmartChannelSource::RerunGrpcStream {
                                url: url.clone(),
                            },
                        );
                        spawn_future(async move {
                            if let Err(err) = re_grpc_client::redap::stream_recording_async(
                                tx,
                                origin,
                                recording_id,
                                on_msg,
                            )
                            .await
                            {
                                re_log::warn!(
                                    "Error while streaming {url}: {}",
                                    re_error::format_ref(&err)
                                );
                            }
                        });
                        Ok(StreamSource::LogMessages(rx))
                    }
                    RedapAddress::Catalog { origin } => Ok(StreamSource::CatalogData { origin }),
                }
            }

            Self::MessageProxy { url } => re_grpc_client::message_proxy::stream(&url, on_msg)
                .map_err(|err| err.into())
                .map(StreamSource::LogMessages),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn test_data_source_from_uri() {
    use re_log_types::FileSource;

    let mut failed = false;

    let file = [
        "file://foo",
        "foo.rrd",
        "foo.png",
        "/foo/bar/baz",
        "D:/file",
    ];
    let http = [
        "example.zip/foo.rrd",
        "www.foo.zip/foo.rrd",
        "www.foo.zip/blueprint.rbl",
    ];
    let grpc = [
        "http://foo.zip",
        "https://foo.zip",
        "http://127.0.0.1:9876",
        "https://redap.rerun.io",
    ];

    let file_source = FileSource::DragAndDrop {
        recommended_application_id: None,
        recommended_recording_id: None,
        force_store_info: false,
    };

    for uri in file {
        if !matches!(
            DataSource::from_uri(file_source.clone(), uri.to_owned()),
            DataSource::FilePath { .. }
        ) {
            eprintln!("Expected {uri:?} to be categorized as FilePath");
            failed = true;
        }
    }

    for uri in http {
        if !matches!(
            DataSource::from_uri(file_source.clone(), uri.to_owned()),
            DataSource::RrdHttpUrl { .. }
        ) {
            eprintln!("Expected {uri:?} to be categorized as RrdHttpUrl");
            failed = true;
        }
    }

    for uri in grpc {
        if !matches!(
            DataSource::from_uri(file_source.clone(), uri.to_owned()),
            DataSource::MessageProxy { .. }
        ) {
            eprintln!("Expected {uri:?} to be categorized as MessageProxy");
            failed = true;
        }
    }

    assert!(!failed, "one or more test cases failed");
}

//TODO: can we clean that up?
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
