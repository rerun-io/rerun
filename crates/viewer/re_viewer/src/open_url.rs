use url::Url;
use vec1::{Vec1, vec1};

use re_data_source::LogDataSource;
use re_redap_browser::EXAMPLES_ORIGIN;
use re_smart_channel::SmartChannelSource;
use re_ui::CommandPaletteUrl;
use re_viewer_context::{
    CommandSender, DisplayMode, Item, StoreHub, SystemCommand, SystemCommandSender as _,
};

/// A URL that points to a selection (typically an entity) within the currently active recording.
pub const INTRA_RECORDING_URL_SCHEME: &str = "recording://";

/// An eventListener for rrd posted from containing html
pub const WEB_EVENT_LISTENER_SCHEME: &str = "web_event:";

/// Types of URLs that can be opened directly in the viewer.
///
/// This is the highest level way of handling arbitrary URLs inside the viewer.
/// The only higher level way of opening URLs is `ui.ctx().open_url(...)` which will
/// open the URL in a browser if it's not a content URL that we can open inside the viewer.
#[derive(Debug, Clone, PartialEq)]
pub enum ViewerOpenUrl {
    /// A URL that points to a selection (typically an entity) within the currently active recording.
    // TODO(andreas): Not all item types are supported right now. Many of them aren't intra recording, so we probably want a new schema for this
    // that we can re-use in any fragment.
    IntraRecordingSelection(Item),

    /// A remote RRD file, served over http.
    ///
    /// Could be either an `.rrd` recording or a `.rbl` blueprint.
    /// See also [`LogDataSource::RrdHttpUrl`].
    RrdHttpUrl(url::Url),

    /// A path to a local file.
    ///
    /// See also [`LogDataSource::FilePath`].
    #[cfg(not(target_arch = "wasm32"))]
    FilePath(std::path::PathBuf),

    /// A `rerun://` URI pointing to a recording.
    ///
    /// See also [`LogDataSource::RedapDatasetPartition`].
    RedapDatasetPartition(re_uri::DatasetPartitionUri),

    /// A `rerun+http://` URI pointing to a proxy.
    ///
    /// See also [`LogDataSource::RedapProxy`].
    RedapProxy(re_uri::ProxyUri),

    /// A URL that points to a redap server.
    RedapCatalog(re_uri::CatalogUri),

    /// A URL that points to a redap entry.
    RedapEntry(re_uri::EntryUri),

    /// A URL that points to a web event listener.
    ///
    /// This is used only for legacy notebooks.
    WebEventListener,

    /// A web viewer URL with one or more url parameters which all individually can be opened.
    WebViewerUrl {
        /// The base URL of the web viewer (this no longer includes any queries and fragments).
        base_url: url::Url,

        /// The url parameter(s) that can be opened individually.
        ///
        /// Several can be present by providing multiple `url` parameters,
        /// but it's guaranteed to at least one if we hit this enum variant.
        url_parameters: vec1::Vec1<ViewerOpenUrl>,
    },
}

impl std::str::FromStr for ViewerOpenUrl {
    type Err = anyhow::Error;

    /// Tries to parse a content URL or file inside the viewer.
    ///
    /// This is for handling opening arbitrary URLs inside the viewer
    /// (as opposed to opening them in a new tab) for both native and web.
    /// Supported are:
    /// * any URL or file path that can be interpreted as a [`LogDataSource`]
    /// * intra-recording links (typically links to an entity)
    /// * web event listeners
    fn from_str(url: &str) -> Result<Self, Self::Err> {
        // Catalog URI.
        if let Ok(uri) = url.parse::<re_uri::CatalogUri>() {
            Ok(Self::RedapCatalog(uri))
        }
        // Entry URI.
        else if let Ok(uri) = url.parse::<re_uri::EntryUri>() {
            Ok(Self::RedapEntry(uri))
        }
        // Intra-recording selection.
        else if let Some(selection) = url.strip_prefix(INTRA_RECORDING_URL_SCHEME) {
            match selection.parse::<Item>() {
                Ok(item) => Ok(Self::IntraRecordingSelection(item)),
                Err(err) => {
                    anyhow::bail!("Failed to parse selection path {selection:?}: {err}")
                }
            }
        }
        // Web event listener (legacy notebooks).
        else if url.starts_with(WEB_EVENT_LISTENER_SCHEME) {
            Ok(Self::WebEventListener)
        }
        // Log data source.
        else if let Some(data_source) =
            LogDataSource::from_uri(re_log_types::FileSource::Uri, url)
        {
            match data_source {
                LogDataSource::RrdHttpUrl { url, follow: _ } => Ok(Self::RrdHttpUrl(url)),

                #[cfg(not(target_arch = "wasm32"))]
                LogDataSource::FilePath(_file_source, path_buf) => Ok(Self::FilePath(path_buf)),

                LogDataSource::FileContents(..) => {
                    unreachable!("FileContents can not be shared as a URL");
                }

                #[cfg(not(target_arch = "wasm32"))]
                LogDataSource::Stdin => Err(anyhow::anyhow!("`-` is not a valid URL.")),

                LogDataSource::RedapDatasetPartition {
                    uri,
                    select_when_loaded: _,
                } => Ok(Self::RedapDatasetPartition(uri)),

                LogDataSource::RedapProxy(proxy_uri) => Ok(Self::RedapProxy(proxy_uri)),
            }
        }
        // Web viewer URL with `url` parameters.
        else if let Ok(url) = parse_webviewer_url(url) {
            Ok(url)
        }
        // Failed to parse.
        else {
            anyhow::bail!("Failed to parse URL: {url}")
        }
    }
}

fn parse_webviewer_url(url: &str) -> anyhow::Result<ViewerOpenUrl> {
    use std::str::FromStr as _;

    let url = Url::parse(url)?;

    // It's rare, but there might be *several* `url` parameters.
    let url_params = vec1::Vec1::try_from_vec(
        url.query_pairs()
            .filter_map(|(key, value)| (key == "url").then(|| ViewerOpenUrl::from_str(&value)))
            .collect::<anyhow::Result<Vec<_>>>()?,
    )?;

    Ok(ViewerOpenUrl::WebViewerUrl {
        base_url: base_url(&url),
        url_parameters: url_params,
    })
}

/// URL stripped of query and fragment.
pub fn base_url(url: &url::Url) -> url::Url {
    let mut base_url = url.clone();
    base_url.set_query(None);
    base_url.set_fragment(None);
    base_url
}

/// A description of what happens when opening a [`ViewerOpenUrl`].
pub struct ViewerOpenUrlDescription {
    /// The general category of this URL.
    pub category: &'static str,

    /// The specific target of this URL if known.
    ///
    /// This is always shorter than the original URL.
    pub target_short: Option<String>,
}

impl std::fmt::Display for ViewerOpenUrlDescription {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(target) = &self.target_short {
            write!(f, "{}: {target}", self.category)
        } else {
            write!(f, "{}", self.category)
        }
    }
}

impl ViewerOpenUrl {
    /// Tries to create a viewer import URL for the current display mode (typically for sharing purposes).
    ///
    /// Conceptually, this is the inverse of [`Self::open`]. However, some import URLs like
    /// intra-recording links aren't stand-alone enough to be returned by this function.
    ///
    /// To produce a sharable url, from this result, call [`Self::sharable_url`].
    ///
    /// Returns Err(reason) if the current state can't be shared with a url.
    // TODO(#10866): Should have anchors for selection etc. when supported. Need to figure out how this works together with the "share editor".
    // Does this method merely provide the starting point?
    #[allow(unused)] // TODO(rerun/dataplatform#1336): Only used on the web. About to change!
    pub fn from_display_mode(
        store_hub: &StoreHub,
        display_mode: DisplayMode,
    ) -> anyhow::Result<Self> {
        match display_mode {
            DisplayMode::Settings => {
                // Not much point in updating address for the settings screen.
                Err(anyhow::anyhow!("Can't share links to the settings screen."))
            }

            DisplayMode::LocalRecordings => {
                // Local recordings includes those downloaded from rrd urls
                // (as of writing this includes the sample recordings!)
                // If it's one of those we want to update the address bar accordingly.

                let active_recording = store_hub
                    .active_recording()
                    .ok_or(anyhow::anyhow!("No active recording"))?;
                let data_source = active_recording
                    .data_source
                    .as_ref()
                    .ok_or(anyhow::anyhow!("No data source"))?;

                // Note that some of these data sources aren't actually sharable URLs.
                // But since we have to handles this for `open_url` and `sharable_url` anyways,
                // we just preserve as much as possible here.
                match data_source {
                    SmartChannelSource::RrdHttpStream { url, follow: _ } => {
                        Ok(Self::RrdHttpUrl(url.parse::<url::Url>()?))
                    }

                    SmartChannelSource::File(path_buf) => {
                        #[cfg(not(target_arch = "wasm32"))]
                        {
                            Ok(Self::FilePath(path_buf.clone()))
                        }
                        #[cfg(target_arch = "wasm32")]
                        {
                            Err(anyhow::anyhow!(
                                "Can't share links to local files on the web."
                            ))
                        }
                    }

                    SmartChannelSource::RrdWebEventListener => Ok(Self::WebEventListener),

                    SmartChannelSource::JsChannel { .. } => Err(anyhow::anyhow!(
                        "Can't share links to recordings streamed from the web."
                    )),

                    SmartChannelSource::Sdk => Err(anyhow::anyhow!(
                        "Can't share links to recordings streamed from the SDKs."
                    )),

                    SmartChannelSource::Stdin => Err(anyhow::anyhow!(
                        "Can't share links to recordings streamed from stdin."
                    )),

                    SmartChannelSource::RedapGrpcStream {
                        uri,
                        select_when_loaded: _,
                    } => Ok(Self::RedapDatasetPartition(uri.clone())),

                    SmartChannelSource::MessageProxy(proxy_uri) => {
                        Ok(Self::RedapProxy(proxy_uri.clone()))
                    }
                }
            }

            DisplayMode::LocalTable(_table_id) => {
                // We can't share links to local tables, so can't update the url.
                Err(anyhow::anyhow!("Can't share links to local tables."))
            }

            DisplayMode::RedapEntry(entry) => Ok(Self::RedapEntry(entry.clone())),

            DisplayMode::RedapServer(origin) => {
                // `as_url` on the origin gives us an http link.
                // We want a rerun link here instead.
                Ok(Self::RedapCatalog(re_uri::CatalogUri::new(origin.clone())))
            }

            DisplayMode::ChunkStoreBrowser => {
                // As of writing the store browser is more of a debugging feature.
                Err(anyhow::anyhow!(
                    "Can't share links to the chunk store browser."
                ))
            }
        }
    }

    /// Returns a URL for sharing purposes.
    ///
    /// Whenever possible you should provide a web viewer base URL so that the URL can be opened
    /// in the browser (this does *not* exclude native, web viewer URLs can still be opened there as well!)
    ///
    /// This is roughly the inverse of `Self::from_str`.
    #[allow(unused)] // TODO(rerun/dataplatform#1336): Only used on the web. About to change!
    pub fn sharable_url(&self, web_viewer_base_url: Option<&url::Url>) -> anyhow::Result<String> {
        let urls: Vec1<String> = match self {
            Self::IntraRecordingSelection(item) => {
                let data_path = item.to_data_path().ok_or_else(|| {
                    // See also `Item::from_str`
                    anyhow::anyhow!("Can only share links to entities & components")
                })?;
                let data_path_str = data_path.to_string();
                vec1![format!(
                    "{INTRA_RECORDING_URL_SCHEME}{}",
                    data_path_str.trim_start_matches('/')
                )]
            }

            Self::RrdHttpUrl(url) => {
                vec1![url.to_string()]
            }

            #[cfg(not(target_arch = "wasm32"))]
            Self::FilePath(path_buf) => {
                vec1![(*path_buf.to_string_lossy()).to_owned()]
            }

            Self::RedapDatasetPartition(dataset_partition_uri) => {
                vec1![dataset_partition_uri.to_string()]
            }

            Self::RedapProxy(proxy_uri) => {
                vec1![proxy_uri.to_string()]
            }

            Self::RedapCatalog(catalog_uri) => {
                // The welcome page is a fake catalog right now.
                // If we dont'have a base url we'll just roll with it. It looks ugly but it's sharable.
                if catalog_uri.origin == *EXAMPLES_ORIGIN
                    && let Some(base_url) = web_viewer_base_url
                {
                    return Ok(base_url.to_string());
                }

                vec1![catalog_uri.to_string()]
            }

            Self::RedapEntry(entry) => vec1![entry.to_string()],

            Self::WebEventListener => vec1![WEB_EVENT_LISTENER_SCHEME.to_owned()],

            Self::WebViewerUrl {
                base_url: _,
                url_parameters,
            } => {
                // Already a sharable URL to a web viewer.
                // Typically we don't end up here, but if we do and have a mismatching web viewer base URL
                // things might get weird. We could warn about it, but if we intentionally overwrote it
                // that's not helping either!
                //
                // Either way we definitely want to use the web viewer base URL that got passed in, since
                // this one defines the user's intention
                Vec1::try_from_vec(
                    url_parameters
                        .iter()
                        .map(|url| url.sharable_url(None))
                        .collect::<anyhow::Result<Vec<_>>>()?,
                )
                .expect("converted from a vec1")
            }
        };

        // Combine the URL(s) with the web viewer base URL if provided.
        if let Some(web_viewer_base_url) = web_viewer_base_url {
            let mut share_url = web_viewer_base_url.clone();

            // Use the form_urlencoded::Serializer to build the query string with multiple "url" parameters.
            // It's important to not just append the strings, since we have to take care of correctly escaping.
            let mut serializer = url::form_urlencoded::Serializer::new(String::new());
            for url in &urls {
                serializer.append_pair("url", url);
            }
            share_url.set_query(Some(&serializer.finish()));

            Ok(share_url.to_string())
        } else if urls.len() == 1 {
            Ok(urls.split_off_first().0)
        } else {
            Err(anyhow::anyhow!(
                "Can't share more than one URL without a web viewer base URL"
            ))
        }
    }

    /// Opens a content URL or file inside the viewer.
    ///
    /// This is for handling opening arbitrary URLs inside the viewer
    /// (as opposed to opening them in a new tab) for both native and web.
    /// Supported are:
    /// * any URL or file path that can be interpreted as a [`LogDataSource`]
    /// * intra-recording links (typically links to an entity)
    /// * web event listeners
    ///
    /// This is the highest level way of opening arbitrary URLs inside the viewer.
    /// The only higher level way of opening URLs is `ui.ctx().open_url(...)` which will
    /// open the URL in a browser if it's not a content URL that we can open inside the viewer.
    pub fn open(
        self,
        egui_ctx: &egui::Context,
        follow_if_http: bool,
        select_redap_source_when_loaded: bool,
        command_sender: &CommandSender,
    ) {
        re_log::debug!("Opening URL: {:?}", &self);

        match self {
            Self::IntraRecordingSelection(item) => {
                command_sender.send_system(SystemCommand::SetSelection(item));
            }
            Self::RrdHttpUrl(url) => {
                command_sender.send_system(SystemCommand::LoadDataSource(
                    LogDataSource::RrdHttpUrl {
                        url,
                        // `follow` is not encoded in the url itself right now.
                        follow: follow_if_http,
                    },
                ));
            }
            #[cfg(not(target_arch = "wasm32"))]
            Self::FilePath(path_buf) => {
                command_sender.send_system(SystemCommand::LoadDataSource(LogDataSource::FilePath(
                    re_log_types::FileSource::Uri,
                    path_buf,
                )));
            }
            Self::RedapDatasetPartition(uri) => {
                command_sender.send_system(SystemCommand::LoadDataSource(
                    LogDataSource::RedapDatasetPartition {
                        uri,
                        // `select_when_loaded` is not encoded in the url itself right now.
                        select_when_loaded: select_redap_source_when_loaded,
                    },
                ));
            }
            Self::RedapProxy(proxy_uri) => {
                command_sender.send_system(SystemCommand::LoadDataSource(
                    LogDataSource::RedapProxy(proxy_uri),
                ));
            }
            Self::RedapCatalog(uri) => {
                command_sender.send_system(SystemCommand::AddRedapServer(uri.origin.clone()));
                command_sender
                    .send_system(SystemCommand::SetSelection(Item::RedapServer(uri.origin)));
            }
            Self::RedapEntry(uri) => {
                command_sender.send_system(SystemCommand::AddRedapServer(uri.origin.clone()));
                command_sender.send_system(SystemCommand::SetSelection(Item::RedapEntry(uri)));
            }
            Self::WebEventListener => {
                handle_web_event_listener(egui_ctx, command_sender);
            }
            Self::WebViewerUrl {
                base_url: _base_url,
                url_parameters,
            } => {
                #[cfg(target_arch = "wasm32")]
                {
                    // We _are_ a web viewer.
                    // If the base URL doesn't match our own then that's reason for concern (==warn),
                    // because this URL was probably meant to be opened in a different Rerun version.
                    if let Some(window) = web_sys::window()
                        && let Ok(location) = window.location().href()
                        && let Ok(location) = Url::parse(&location)
                    {
                        let current_webpage_base_url = base_url(&location);

                        if _base_url != current_webpage_base_url {
                            re_log::warn!(
                                "The base URL of the web viewer ({:?}) does not match the URL being opened ({:?}). This URL may be intended for a different Rerun version.",
                                current_webpage_base_url.as_str(),
                                _base_url.as_str(),
                            );
                        }
                    }
                }

                for url in url_parameters {
                    url.open(
                        egui_ctx,
                        follow_if_http,
                        select_redap_source_when_loaded,
                        command_sender,
                    );
                }
            }
        }
    }

    pub fn command_palette_parse_url(url: &str) -> Option<CommandPaletteUrl> {
        let Ok(open_url) = url.parse::<Self>() else {
            return None;
        };

        Some(CommandPaletteUrl {
            url: url.to_owned(),
            command_text: format!("Open {}", open_url.open_description()),
        })
    }

    /// Describes what happens when calling [`Self::open`] with this URL.
    pub fn open_description(&self) -> ViewerOpenUrlDescription {
        match self {
            Self::IntraRecordingSelection(item) => ViewerOpenUrlDescription {
                category: "Selection",
                target_short: item.entity_path().map(|p| p.to_string()),
            },

            Self::RrdHttpUrl(url) => {
                let path = url.path();
                let rrd_file_name = path.split('/').next_back().map(|s| s.to_owned());

                ViewerOpenUrlDescription {
                    category: "RRD from link",
                    target_short: rrd_file_name,
                }
            }

            #[cfg(not(target_arch = "wasm32"))]
            Self::FilePath(path) => ViewerOpenUrlDescription {
                category: "File",
                target_short: path.file_name().map(|s| s.display().to_string()),
            },

            Self::RedapDatasetPartition(uri) => ViewerOpenUrlDescription {
                category: "Partition",
                target_short: Some(uri.partition_id.clone()),
            },

            Self::RedapProxy(_) => ViewerOpenUrlDescription {
                category: "GRPC proxy",
                target_short: None,
            },

            Self::RedapCatalog(uri) => ViewerOpenUrlDescription {
                category: "Catalog",
                target_short: Some(uri.origin.host.to_string()),
            },

            Self::RedapEntry(uri) => ViewerOpenUrlDescription {
                category: "Redap Entry",
                target_short: Some(uri.entry_id.to_string()),
            },

            Self::WebEventListener => ViewerOpenUrlDescription {
                category: "Web event listener",
                target_short: None,
            },

            Self::WebViewerUrl { url_parameters, .. } => {
                if url_parameters.len() == 1 {
                    url_parameters.first().open_description()
                } else {
                    ViewerOpenUrlDescription {
                        category: "Several URLs",
                        target_short: Some(format!("{} URLs", url_parameters.len())),
                    }
                }
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn handle_web_event_listener(_egui_ctx: &egui::Context, _command_sender: &CommandSender) {
    re_log::warn!("{WEB_EVENT_LISTENER_SCHEME:?} urls are only available on the web viewer.");
}

#[cfg(target_arch = "wasm32")]
fn handle_web_event_listener(egui_ctx: &egui::Context, command_sender: &CommandSender) {
    use re_log::ResultExt as _;
    use re_log_encoding::stream_rrd_from_http::HttpMessage;
    use std::{ops::ControlFlow, sync::Arc};

    // Process an rrd when it's posted via `window.postMessage`
    let (tx, rx) = re_smart_channel::smart_channel(
        re_smart_channel::SmartMessageSource::RrdWebEventCallback,
        re_smart_channel::SmartChannelSource::RrdWebEventListener,
    );
    let egui_ctx = egui_ctx.clone();
    re_log_encoding::stream_rrd_from_http::stream_rrd_from_event_listener(Arc::new({
        move |msg| {
            egui_ctx.request_repaint_after(std::time::Duration::from_millis(10));

            match msg {
                HttpMessage::LogMsg(msg) => {
                    if tx.send(msg).is_ok() {
                        ControlFlow::Continue(())
                    } else {
                        re_log::info_once!(
                            "Failed to send log message to viewer - closing connection"
                        );
                        ControlFlow::Break(())
                    }
                }
                HttpMessage::Success => {
                    tx.quit(None).warn_on_err_once("Failed to send quit marker");
                    ControlFlow::Break(())
                }
                HttpMessage::Failure(err) => {
                    tx.quit(Some(err))
                        .warn_on_err_once("Failed to send quit marker");
                    ControlFlow::Break(())
                }
            }
        }
    }));

    command_sender.send_system(SystemCommand::AddReceiver(rx));
}

#[cfg(test)]
mod tests {
    use std::str::FromStr as _;

    use re_entity_db::{EntityDb, EntityPath, InstancePath};
    use re_log_types::{EntryId, StoreId, StoreKind, TableId};
    use re_smart_channel::SmartChannelSource;
    use re_viewer_context::{DisplayMode, Item, StoreHub};
    use url::Url;

    use super::ViewerOpenUrl;

    #[test]
    fn test_viewer_open_url_from_str() {
        // RedapCatalog
        let url = "rerun://localhost:51234/catalog";
        assert_eq!(
            ViewerOpenUrl::from_str(url).unwrap(),
            ViewerOpenUrl::RedapCatalog(re_uri::CatalogUri::from_str(url).unwrap())
        );

        // RedapEntry
        let entry_id = EntryId::new();
        let url = format!("rerun://localhost:51234/entry/{entry_id}");
        assert_eq!(
            ViewerOpenUrl::from_str(&url).unwrap(),
            ViewerOpenUrl::RedapEntry(re_uri::EntryUri::from_str(&url).unwrap())
        );

        // DatasetPartitionUri
        let url = format!("rerun://127.0.0.1:1234/dataset/{entry_id}?partition_id=pid");
        assert_eq!(
            ViewerOpenUrl::from_str(&url).unwrap(),
            ViewerOpenUrl::RedapDatasetPartition(url.parse().unwrap())
        );

        // IntraRecordingSelection
        let entity_path = EntityPath::from("camera");
        let url = format!("recording://{entity_path}");
        assert_eq!(
            ViewerOpenUrl::from_str(&url).unwrap(),
            ViewerOpenUrl::IntraRecordingSelection(Item::InstancePath(InstancePath::entity_all(
                entity_path
            )))
        );

        // WebEventListener
        let url = "web_event:test_listener";
        assert_eq!(
            ViewerOpenUrl::from_str(url).unwrap(),
            ViewerOpenUrl::WebEventListener
        );

        // LogDataSource
        {
            // HTTP URL
            let url = "https://example.com/data.rrd";
            assert_eq!(
                ViewerOpenUrl::from_str(url).unwrap(),
                ViewerOpenUrl::RrdHttpUrl(Url::parse("https://example.com/data.rrd").unwrap())
            );

            // Test file path (native only)
            #[cfg(not(target_arch = "wasm32"))]
            {
                let url = "/path/to/file.rrd";
                assert_eq!(
                    ViewerOpenUrl::from_str(url).unwrap(),
                    ViewerOpenUrl::FilePath(std::path::PathBuf::from("/path/to/file.rrd"))
                );
            }

            // Other variants should be sufficiently covered by `LogDataSource::from_uri` tests.
        }
        // Test WebViewerUrl
        {
            // Simple - single URL parameter.
            let url = "https://foo.com/test?url=https://example.com/data.rrd";
            let expected = ViewerOpenUrl::WebViewerUrl {
                base_url: Url::parse("https://foo.com/test").unwrap(),
                url_parameters: vec1::vec1![ViewerOpenUrl::RrdHttpUrl(
                    Url::parse("https://example.com/data.rrd").unwrap()
                )],
            };
            assert_eq!(ViewerOpenUrl::from_str(url).unwrap(), expected);

            // Complex - multiple URL parameters of different typesl
            let url = "https://foo.com/?url=rerun://localhost:51234/catalog&url=recording://camera&url=https://example.com/data.rrd";
            let expected = ViewerOpenUrl::WebViewerUrl {
                base_url: Url::parse("https://foo.com/").unwrap(),
                url_parameters: vec1::vec1![
                    ViewerOpenUrl::RedapCatalog(
                        re_uri::CatalogUri::from_str("rerun://localhost:51234/catalog").unwrap()
                    ),
                    ViewerOpenUrl::IntraRecordingSelection(Item::InstancePath(
                        InstancePath::entity_all(EntityPath::from("camera"))
                    )),
                    ViewerOpenUrl::RrdHttpUrl(Url::parse("https://example.com/data.rrd").unwrap())
                ],
            };
            assert_eq!(ViewerOpenUrl::from_str(url).unwrap(), expected);
        }

        // Invalid URLs.
        let invalid_urls = vec![
            "invalid://url",
            "recording://camera%20with%20spaces",
            "https://foo.com/?url=invalid_url",
            "https://foo.com/test?url=invalid_url",
            "",
            "   ",
            "aaaaaaaaaaa",
        ];

        for url in invalid_urls {
            assert!(
                url.parse::<ViewerOpenUrl>().is_err(),
                "Expected error for {url}"
            );
        }
    }

    #[test]
    fn test_viewer_open_url_from_display_mode() {
        let store_hub = StoreHub::test_hub();

        // Settings
        assert!(ViewerOpenUrl::from_display_mode(&store_hub, DisplayMode::Settings).is_err());

        // RedapServer
        assert_eq!(
            ViewerOpenUrl::from_display_mode(
                &store_hub,
                DisplayMode::RedapServer("rerun://localhost:51234".parse().unwrap(),)
            )
            .unwrap(),
            ViewerOpenUrl::RedapCatalog("rerun://localhost:51234".parse().unwrap())
        );

        // LocalTable
        assert!(
            ViewerOpenUrl::from_display_mode(
                &store_hub,
                DisplayMode::LocalTable(TableId::new("test_table".to_owned()))
            )
            .is_err()
        );

        // RedapEntry
        let origin = "rerun://localhost:51234".parse().unwrap();
        let entry_uri = re_uri::EntryUri::new(origin, EntryId::new());
        assert_eq!(
            ViewerOpenUrl::from_display_mode(
                &store_hub,
                DisplayMode::RedapEntry(entry_uri.clone())
            )
            .unwrap(),
            ViewerOpenUrl::RedapEntry(entry_uri)
        );

        // ChunkStoreBrowser
        assert!(
            ViewerOpenUrl::from_display_mode(&store_hub, DisplayMode::ChunkStoreBrowser).is_err(),
            "ChunkStoreBrowser should not be convertible to ViewerOpenUrl"
        );

        // Local recordings is handled in `test_viewer_open_url_from_local_recordings_display_mode`
    }

    #[test]
    fn test_viewer_open_url_from_local_recordings_display_mode() {
        let mut store_hub = StoreHub::test_hub();

        fn add_store(store_hub: &mut StoreHub, data_source: Option<SmartChannelSource>) {
            let store_id = StoreId::random(StoreKind::Recording, "test");
            let mut entity_db = EntityDb::new(store_id.clone());
            entity_db.data_source = data_source;
            store_hub.insert_entity_db(entity_db);
            store_hub.set_active_recording(store_id);
        }

        // originating from a file.
        add_store(
            &mut store_hub,
            Some(SmartChannelSource::File(std::path::PathBuf::from(
                "/path/to/test.rrd",
            ))),
        );
        assert_eq!(
            ViewerOpenUrl::from_display_mode(&store_hub, DisplayMode::LocalRecordings).unwrap(),
            ViewerOpenUrl::FilePath(std::path::PathBuf::from("/path/to/test.rrd"))
        );

        // originating from HTTP stream.
        add_store(
            &mut store_hub,
            Some(SmartChannelSource::RrdHttpStream {
                url: "https://example.com/recording.rrd".to_owned(),
                follow: false,
            }),
        );
        assert_eq!(
            ViewerOpenUrl::from_display_mode(&store_hub, DisplayMode::LocalRecordings).unwrap(),
            ViewerOpenUrl::RrdHttpUrl("https://example.com/recording.rrd".parse().unwrap())
        );

        // originating from SDK (not possible).
        add_store(&mut store_hub, Some(SmartChannelSource::Sdk));
        assert!(
            ViewerOpenUrl::from_display_mode(&store_hub, DisplayMode::LocalRecordings).is_err(),
        );

        // originating from stdin (not possible).
        add_store(&mut store_hub, Some(SmartChannelSource::Stdin));
        assert!(
            ViewerOpenUrl::from_display_mode(&store_hub, DisplayMode::LocalRecordings).is_err(),
        );

        // originating from web event listener.
        add_store(
            &mut store_hub,
            Some(SmartChannelSource::RrdWebEventListener),
        );
        assert_eq!(
            ViewerOpenUrl::from_display_mode(&store_hub, DisplayMode::LocalRecordings).unwrap(),
            ViewerOpenUrl::WebEventListener
        );

        // originating from JS channel (not possible).
        add_store(
            &mut store_hub,
            Some(SmartChannelSource::JsChannel {
                channel_name: "test_channel".to_owned(),
            }),
        );
        assert!(
            ViewerOpenUrl::from_display_mode(&store_hub, DisplayMode::LocalRecordings).is_err(),
        );

        // originating from Redap gRPC stream.
        let entry_id = EntryId::new();
        let uri = format!("rerun://127.0.0.1:1234/dataset/{entry_id}?partition_id=pid");
        add_store(
            &mut store_hub,
            Some(SmartChannelSource::RedapGrpcStream {
                uri: uri.parse().unwrap(),
                select_when_loaded: false,
            }),
        );
        assert_eq!(
            ViewerOpenUrl::from_display_mode(&store_hub, DisplayMode::LocalRecordings).unwrap(),
            ViewerOpenUrl::RedapDatasetPartition(uri.parse().unwrap())
        );

        // originating from message proxy.
        let uri = "rerun://localhost:51234/proxy";
        add_store(
            &mut store_hub,
            Some(SmartChannelSource::MessageProxy(uri.parse().unwrap())),
        );
        assert_eq!(
            ViewerOpenUrl::from_display_mode(&store_hub, DisplayMode::LocalRecordings).unwrap(),
            ViewerOpenUrl::RedapProxy(uri.parse().unwrap())
        );

        // with no data source (not possible).
        add_store(&mut store_hub, None);
        assert!(
            ViewerOpenUrl::from_display_mode(&store_hub, DisplayMode::LocalRecordings).is_err(),
        );
    }

    #[test]
    fn test_viewer_open_url_sharable_url_without_base_url() {
        assert_eq!(
            ViewerOpenUrl::IntraRecordingSelection("my/path".parse().unwrap())
                .sharable_url(None)
                .unwrap(),
            "recording://my/path"
        );

        assert_eq!(
            ViewerOpenUrl::RrdHttpUrl(Url::parse("https://example.com/data.rrd").unwrap())
                .sharable_url(None)
                .unwrap(),
            "https://example.com/data.rrd"
        );

        assert_eq!(
            ViewerOpenUrl::FilePath("/path/to/file.rrd".into())
                .sharable_url(None)
                .unwrap(),
            "/path/to/file.rrd"
        );

        let entry_id = EntryId::new();
        let uri = format!("rerun://127.0.0.1:1234/dataset/{entry_id}?partition_id=pid");
        assert_eq!(
            ViewerOpenUrl::RedapDatasetPartition(uri.parse().unwrap())
                .sharable_url(None)
                .unwrap(),
            uri
        );

        assert_eq!(
            ViewerOpenUrl::RedapProxy("rerun://localhost:51234/proxy".parse().unwrap())
                .sharable_url(None)
                .unwrap(),
            "rerun://localhost:51234/proxy"
        );

        assert_eq!(
            ViewerOpenUrl::RedapCatalog("rerun://localhost:51234/catalog".parse().unwrap())
                .sharable_url(None)
                .unwrap(),
            "rerun://localhost:51234/catalog"
        );

        let url = format!("rerun://localhost:51234/entry/{entry_id}");
        assert_eq!(
            ViewerOpenUrl::RedapEntry(url.parse().unwrap())
                .sharable_url(None)
                .unwrap(),
            url
        );

        assert_eq!(
            ViewerOpenUrl::WebEventListener.sharable_url(None).unwrap(),
            "web_event:"
        );

        assert_eq!(
            ViewerOpenUrl::WebViewerUrl {
                base_url: Url::parse("https://foo.com/test").unwrap(),
                url_parameters: vec1::vec1![ViewerOpenUrl::RrdHttpUrl(
                    Url::parse("https://example.com/data.rrd").unwrap()
                )],
            }
            .sharable_url(None)
            .unwrap(),
            "https://example.com/data.rrd",
        );
        assert!(
            ViewerOpenUrl::WebViewerUrl {
                base_url: Url::parse("https://foo.com/test").unwrap(),
                url_parameters: vec1::vec1![
                    ViewerOpenUrl::RrdHttpUrl(Url::parse("https://example.com/bar.rrd").unwrap()),
                    ViewerOpenUrl::RedapProxy("rerun://localhost:51234/proxy".parse().unwrap())
                ],
            }
            .sharable_url(None)
            .is_err() // We don't know how to share several URLs at once without a web viewer URL.
        );
    }

    #[test]
    fn test_viewer_open_url_sharable_url_with_base_url() {
        let base_url = Url::parse("https://foo.com/test").unwrap();
        let base_url_param = Some(&base_url);

        assert_eq!(
            ViewerOpenUrl::IntraRecordingSelection("my/path".parse().unwrap())
                .sharable_url(base_url_param)
                .unwrap(),
            "https://foo.com/test?url=recording%3A%2F%2Fmy%2Fpath"
        );

        assert_eq!(
            ViewerOpenUrl::RrdHttpUrl("https://example.com/data.rrd".parse().unwrap())
                .sharable_url(base_url_param)
                .unwrap(),
            "https://foo.com/test?url=https%3A%2F%2Fexample.com%2Fdata.rrd"
        );

        assert_eq!(
            ViewerOpenUrl::RedapDatasetPartition(
                "rerun://127.0.0.1:1234/dataset/1830B33B45B963E7774455beb91701ae?partition_id=pid"
                    .parse()
                    .unwrap()
            )
            .sharable_url(base_url_param)
            .unwrap(),
            format!(
                "https://foo.com/test?url=rerun%3A%2F%2F127.0.0.1%3A1234%2Fdataset%2F1830B33B45B963E7774455beb91701ae%3Fpartition_id%3Dpid"
            )
        );

        assert_eq!(
            ViewerOpenUrl::RedapProxy("rerun://localhost:51234/proxy".parse().unwrap())
                .sharable_url(base_url_param)
                .unwrap(),
            "https://foo.com/test?url=rerun%3A%2F%2Flocalhost%3A51234%2Fproxy"
        );

        assert_eq!(
            ViewerOpenUrl::RedapCatalog("rerun://localhost:51234/catalog".parse().unwrap())
                .sharable_url(base_url_param)
                .unwrap(),
            "https://foo.com/test?url=rerun%3A%2F%2Flocalhost%3A51234%2Fcatalog"
        );

        let entry_id = EntryId::new();
        let url = format!("rerun://localhost:51234/entry/{entry_id}");
        let encoded_url = url::form_urlencoded::byte_serialize(url.as_bytes()).collect::<String>();
        assert_eq!(
            ViewerOpenUrl::RedapEntry(url.parse().unwrap())
                .sharable_url(base_url_param)
                .unwrap(),
            format!("https://foo.com/test?url={encoded_url}")
        );

        assert_eq!(
            ViewerOpenUrl::WebEventListener
                .sharable_url(base_url_param)
                .unwrap(),
            "https://foo.com/test?url=web_event%3A"
        );

        assert_eq!(
            ViewerOpenUrl::WebViewerUrl {
                base_url: Url::parse("http://foo.com/doesn't-matter").unwrap(),
                url_parameters: vec1::vec1![ViewerOpenUrl::RrdHttpUrl(
                    Url::parse("https://example.com/data.rrd").unwrap()
                )],
            }
            .sharable_url(base_url_param)
            .unwrap(),
            "https://foo.com/test?url=https%3A%2F%2Fexample.com%2Fdata.rrd",
        );
        assert_eq!(
            ViewerOpenUrl::WebViewerUrl {
                base_url: Url::parse("http://foo.com/doesn't-matter").unwrap(),
                url_parameters: vec1::vec1![
                    ViewerOpenUrl::RrdHttpUrl(Url::parse("https://example.com/bar.rrd").unwrap()),
                    ViewerOpenUrl::RedapProxy("rerun://localhost:51234/proxy".parse().unwrap())
                ],
            }
            .sharable_url(base_url_param)
            .unwrap(),
            "https://foo.com/test?url=https%3A%2F%2Fexample.com%2Fbar.rrd&url=rerun%3A%2F%2Flocalhost%3A51234%2Fproxy",
        );
    }
}
