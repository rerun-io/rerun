use std::sync::LazyLock;

use re_data_source::LogDataSource;
use re_log_channel::LogSource;
use re_uri::Scheme;
use re_uri::external::url::{self, Url};
use vec1::{Vec1, vec1};

use crate::{
    CommandSender, DisplayMode, Item, ItemCollection, StoreHub, SystemCommand,
    SystemCommandSender as _, ViewerContext,
};

/// A URL that points to a selection (typically an entity) within the currently active recording.
pub const INTRA_RECORDING_URL_SCHEME: &str = "recording://";

pub const SETTINGS_URL: &str = "about:settings";

pub const CHUNK_STORE_BROWSER_URL: &str = "about:chunk_store";

/// An eventListener for rrd posted from containing html
pub const WEB_EVENT_LISTENER_SCHEME: &str = "web_event:";

/// Origin used to show the examples ui in the redap browser.
///
/// Not actually a valid origin.
pub static EXAMPLES_ORIGIN: LazyLock<re_uri::Origin> = LazyLock::new(|| re_uri::Origin {
    scheme: Scheme::RerunHttps,
    host: url::Host::Domain(String::from("_examples.rerun.io")),
    port: 443,
});

/// Types of URLs that can be opened directly in the viewer.
///
/// This is the highest level way of handling arbitrary URLs inside the viewer.
/// The only higher level way of opening URLs is `ui.ctx().open_url(...)` which will
/// open the URL in a browser if it's not a content URL that we can open inside the viewer.
#[derive(Clone, PartialEq)]
pub enum ViewerOpenUrl {
    /// A URL that points to a selection (typically an entity) within the currently active recording.
    // TODO(andreas): Not all item types are supported right now. Many of them aren't intra recording, so we probably want a new schema for this
    // that we can re-use in any fragment.
    IntraRecordingSelection(Item),

    /// A remote file, served over http.
    ///
    /// Could be an `.rrd` recording, `.rbl` blueprint, `.mcap`, `.png`, `.glb`, etc.
    /// See also [`LogDataSource::HttpUrl`].
    HttpUrl(Url),

    /// A path to a local file.
    ///
    /// See also [`LogDataSource::FilePath`].
    #[cfg(not(target_arch = "wasm32"))]
    FilePath(std::path::PathBuf),

    /// A `rerun://` URI pointing to a recording.
    ///
    /// See also [`LogDataSource::RedapDatasetSegment`].
    RedapDatasetSegment(re_uri::DatasetSegmentUri),

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
        base_url: Url,

        /// The url parameter(s) that can be opened individually.
        ///
        /// Several can be present by providing multiple `url` parameters,
        /// but it's guaranteed to at least one if we hit this enum variant.
        url_parameters: vec1::Vec1<Self>,
    },

    /// The url to the settings screen.
    Settings,

    /// A url to the chunk store browser.
    ChunkStoreBrowser,
}

impl std::fmt::Debug for ViewerOpenUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IntraRecordingSelection(item) => write!(f, "IntraRecordingSelection{item:?}"),
            Self::HttpUrl(url) => write!(f, "HttpUrl({url})"),
            #[cfg(not(target_arch = "wasm32"))]
            Self::FilePath(path) => write!(f, "FilePath({path:?})"),
            Self::RedapDatasetSegment(uri) => write!(f, "RedapDatasetSegment({uri})"),
            Self::RedapProxy(uri) => write!(f, "RedapProxy({uri})"),
            Self::RedapCatalog(uri) => write!(f, "RedapCatalog({uri})"),
            Self::RedapEntry(uri) => write!(f, "RedapEntry({uri})"),
            Self::WebEventListener => write!(f, "WebEventListener"),
            Self::WebViewerUrl {
                base_url,
                url_parameters,
            } => f
                .debug_struct("WebViewerUrl")
                .field("base_url", base_url)
                .field("url_parameters", url_parameters)
                .finish(),
            Self::Settings => write!(f, "Settings"),
            Self::ChunkStoreBrowser => write!(f, "ChunkStoreBrowser"),
        }
    }
}

impl From<re_uri::RedapUri> for ViewerOpenUrl {
    fn from(value: re_uri::RedapUri) -> Self {
        match value {
            re_uri::RedapUri::Catalog(uri) => Self::RedapCatalog(uri),
            re_uri::RedapUri::Entry(uri) => Self::RedapEntry(uri),
            re_uri::RedapUri::DatasetData(uri) => Self::RedapDatasetSegment(uri),
            re_uri::RedapUri::Proxy(uri) => Self::RedapProxy(uri),
        }
    }
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
        let follow = false;

        if url == SETTINGS_URL {
            Ok(Self::Settings)
        } else if url == CHUNK_STORE_BROWSER_URL {
            Ok(Self::ChunkStoreBrowser)
        } else if let Ok(uri) = url.parse::<re_uri::CatalogUri>() {
            Ok(Self::RedapCatalog(uri))
        } else if let Ok(uri) = url.parse::<re_uri::EntryUri>() {
            Ok(Self::RedapEntry(uri))
        } else if let Some(selection) = url.strip_prefix(INTRA_RECORDING_URL_SCHEME) {
            match selection.parse::<Item>() {
                Ok(item) => Ok(Self::IntraRecordingSelection(item)),
                Err(err) => {
                    anyhow::bail!("Failed to parse selection path {selection:?}: {err}")
                }
            }
        } else if url.starts_with(WEB_EVENT_LISTENER_SCHEME) {
            // Web event listener (legacy notebooks).
            Ok(Self::WebEventListener)
        } else if let Some(data_source) =
            LogDataSource::from_uri(re_log_types::FileSource::Uri, url, follow)
        {
            match data_source {
                LogDataSource::HttpUrl { url, .. } => Ok(Self::HttpUrl(url)),

                #[cfg(not(target_arch = "wasm32"))]
                LogDataSource::FilePath { path, .. } => Ok(Self::FilePath(path)),

                LogDataSource::FileContents(..) => {
                    unreachable!("FileContents can not be shared as a URL");
                }

                #[cfg(not(target_arch = "wasm32"))]
                LogDataSource::Stdin => Err(anyhow::anyhow!("`-` is not a valid URL.")),

                LogDataSource::RedapDatasetSegment {
                    uri,
                    select_when_loaded: _,
                } => Ok(Self::RedapDatasetSegment(uri)),

                LogDataSource::RedapProxy(proxy_uri) => Ok(Self::RedapProxy(proxy_uri)),
            }
        } else if let Ok(url) = parse_webviewer_url(url) {
            // Web viewer URL with `url` parameters.
            Ok(url)
        } else {
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
pub fn base_url(url: &Url) -> Url {
    let mut base_url = url.clone();
    base_url.set_query(None);
    base_url.set_fragment(None);
    base_url
}

#[derive(Debug, Clone, Copy, Default)]
pub struct OpenUrlOptions {
    /// Follow live HTTP or file paths.
    //
    // TODO(emilk): consider making this part of `ViewerOpenUrl::RrdHttpUrl/FilePath` instead
    pub follow: bool,

    pub select_redap_source_when_loaded: bool,

    /// Shows the loading screen.
    pub show_loader: bool,
}

impl ViewerOpenUrl {
    pub fn from_context(ctx: &ViewerContext<'_>) -> anyhow::Result<Self> {
        Self::from_context_expanded(
            ctx.store_hub(),
            ctx.display_mode(),
            Some(ctx.time_ctrl),
            ctx.selection(),
        )
    }

    pub fn from_context_expanded(
        store_hub: &StoreHub,
        display_mode: &DisplayMode,
        time_ctrl: Option<&crate::TimeControl>,
        selection: &ItemCollection,
    ) -> anyhow::Result<Self> {
        let mut this = Self::from_display_mode(store_hub, display_mode)?;

        if let Some(fragment) = this.fragment_mut() {
            fragment.selection = selection.first_item().and_then(|item| item.to_data_path());
            fragment.when = time_ctrl.and_then(|time_ctrl| {
                let time = time_ctrl.time_int()?;
                Some((
                    *time_ctrl.timeline_name(),
                    re_log_types::TimeCell {
                        typ: time_ctrl.time_type()?,
                        value: time.into(),
                    },
                ))
            });
            fragment.time_selection = time_ctrl.and_then(|time_ctrl| {
                let time_selection = time_ctrl.time_selection()?;

                Some(re_uri::TimeSelection {
                    timeline: *time_ctrl.timeline()?,
                    range: time_selection.to_int(),
                })
            });
        }

        Ok(this)
    }

    /// Create a link for a channel source.
    ///
    /// Refer to [`Self::from_display_mode`] for more information.
    pub fn from_data_source(data_source: &LogSource) -> anyhow::Result<Self> {
        // Note that some of these data sources aren't actually sharable URLs.
        // But since we have to handles this for `open_url` and `sharable_url` anyways,
        // we just preserve as much as possible here.
        match data_source {
            LogSource::HttpStream { url, .. } => Ok(Self::HttpUrl(url.parse::<Url>()?)),

            LogSource::File { path, .. } => {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    Ok(Self::FilePath(path.clone()))
                }
                #[cfg(target_arch = "wasm32")]
                {
                    _ = path;
                    Err(anyhow::anyhow!(
                        "Can't share links to local files on the web."
                    ))
                }
            }

            LogSource::RrdWebEvent => Ok(Self::WebEventListener),

            LogSource::JsChannel { .. } => Err(anyhow::anyhow!(
                "Can't share links to recordings streamed from the web."
            )),

            LogSource::Sdk => Err(anyhow::anyhow!(
                "Can't share links to recordings streamed from the SDKs."
            )),

            LogSource::Stdin => Err(anyhow::anyhow!(
                "Can't share links to recordings streamed from stdin."
            )),

            LogSource::RedapGrpcStream {
                uri,
                select_when_loaded: _,
            } => Ok(Self::RedapDatasetSegment(uri.clone())),

            LogSource::MessageProxy(proxy_uri) => Ok(Self::RedapProxy(proxy_uri.clone())),
        }
    }

    /// Tries to create a viewer import URL for a [`DisplayMode`] (typically for sharing purposes).
    ///
    /// Conceptually, this is the inverse of [`Self::open`]. However, some import URLs like
    /// intra-recording links aren't stand-alone enough to be returned by this function.
    ///
    /// To produce a sharable url, from this result, call [`Self::sharable_url`].
    ///
    /// Returns Err(reason) if the current state can't be shared with a url.
    pub fn from_display_mode(
        store_hub: &StoreHub,
        display_mode: &DisplayMode,
    ) -> anyhow::Result<Self> {
        match display_mode {
            DisplayMode::Settings(_) => Ok(Self::Settings),

            DisplayMode::Loading(source) => Self::from_data_source(source),

            DisplayMode::LocalRecordings(store_id) => {
                // Local recordings includes those downloaded from rrd urls
                // (as of writing this includes the sample recordings!)
                // If it's one of those we want to update the address bar accordingly.

                let recording = store_hub
                    .store_bundle()
                    .get(store_id)
                    .ok_or_else(|| anyhow::anyhow!("No data for active recording"))?;
                let data_source = recording
                    .data_source
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("No data source"))?;

                Self::from_data_source(data_source)
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

            DisplayMode::ChunkStoreBrowser(_) => Ok(Self::ChunkStoreBrowser),
        }
    }

    /// Returns a URL for sharing purposes.
    ///
    /// Whenever possible you should provide a web viewer base URL so that the URL can be opened
    /// in the browser (this does *not* exclude native, web viewer URLs can still be opened there as well!)
    ///
    /// This is roughly the inverse of `Self::from_str`.
    pub fn sharable_url(&self, web_viewer_base_url: Option<&Url>) -> anyhow::Result<String> {
        let urls = match self {
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

            Self::HttpUrl(url) => vec1![url.to_string()],

            #[cfg(not(target_arch = "wasm32"))]
            Self::FilePath(path) => vec1![(*path.to_string_lossy()).to_owned()],

            Self::RedapDatasetSegment(dataset_segment_uri) => {
                vec1![dataset_segment_uri.to_string()]
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

            Self::Settings => {
                vec1![SETTINGS_URL.to_owned()]
            }
            Self::ChunkStoreBrowser => {
                vec1![CHUNK_STORE_BROWSER_URL.to_owned()]
            }
        };

        combine_with_base_url(web_viewer_base_url, urls)
    }

    /// Try to create a system command for copying this URL.
    ///
    /// This command ([`SystemCommand::CopyViewerUrl`]) makes sure
    /// that if this is in a web-viewer the web-viewer base url is
    /// also correctly copied.
    pub fn copy_url_command(&self) -> anyhow::Result<SystemCommand> {
        self.sharable_url(None).map(SystemCommand::CopyViewerUrl)
    }

    /// Get the data source related to this link, if any.
    pub fn get_data_source(&self) -> Option<LogSource> {
        match &self {
            Self::RedapCatalog(_)
            | Self::RedapEntry(_)
            | Self::IntraRecordingSelection(_)
            | Self::Settings
            | Self::ChunkStoreBrowser => None,

            Self::HttpUrl(url) => Some(LogSource::HttpStream {
                url: url.to_string(),
                follow: false,
            }),
            #[cfg(not(target_arch = "wasm32"))]
            Self::FilePath(path) => Some(LogSource::File {
                path: path.clone(),
                follow: false,
            }),
            Self::RedapDatasetSegment(uri) => Some(LogSource::RedapGrpcStream {
                uri: uri.clone(),
                select_when_loaded: false,
            }),
            Self::RedapProxy(uri) => Some(LogSource::MessageProxy(uri.clone())),
            Self::WebEventListener => Some(LogSource::RrdWebEvent),
            Self::WebViewerUrl { url_parameters, .. } => (url_parameters.len() == 1)
                .then(|| url_parameters.first().get_data_source())
                .flatten(),
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
        options: &OpenUrlOptions,
        command_sender: &CommandSender,
    ) {
        re_log::debug!("Opening URL: {self:?}");

        if options.show_loader
            && let Some(data_source) = self.get_data_source()
        {
            // It doesn't matter if this is overridden by some command below, as that most likely
            // means we want to skip the loading screen anyway.
            command_sender.send_system(SystemCommand::ChangeDisplayMode(DisplayMode::Loading(
                Box::new(data_source),
            )));
        }

        match self {
            Self::IntraRecordingSelection(item) => {
                command_sender.send_system(SystemCommand::set_selection(item));
            }
            Self::HttpUrl(url) => {
                command_sender.send_system(SystemCommand::LoadDataSource(LogDataSource::HttpUrl {
                    url,
                    follow: options.follow,
                }));
            }
            #[cfg(not(target_arch = "wasm32"))]
            Self::FilePath(path) => {
                command_sender.send_system(SystemCommand::LoadDataSource(
                    LogDataSource::FilePath {
                        file_source: re_log_types::FileSource::Uri,
                        path,
                        follow: options.follow,
                    },
                ));
            }
            Self::RedapDatasetSegment(uri) => {
                command_sender.send_system(SystemCommand::LoadDataSource(
                    LogDataSource::RedapDatasetSegment {
                        uri,
                        // `select_when_loaded` is not encoded in the url itself right now.
                        select_when_loaded: options.select_redap_source_when_loaded,
                    },
                ));
            }
            Self::RedapProxy(proxy_uri) => {
                command_sender.send_system(SystemCommand::LoadDataSource(
                    LogDataSource::RedapProxy(proxy_uri.clone()),
                ));
                command_sender.send_system(SystemCommand::set_selection(Item::RedapServer(
                    proxy_uri.origin,
                )));
            }
            Self::RedapCatalog(uri) => {
                command_sender.send_system(SystemCommand::AddRedapServer(uri.origin.clone()));
                command_sender
                    .send_system(SystemCommand::set_selection(Item::RedapServer(uri.origin)));
            }
            Self::RedapEntry(uri) => {
                command_sender.send_system(SystemCommand::AddRedapServer(uri.origin.clone()));
                command_sender.send_system(SystemCommand::set_selection(Item::RedapEntry(uri)));
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
                        &OpenUrlOptions {
                            show_loader: false,
                            ..*options
                        },
                        command_sender,
                    );
                }
            }
            Self::Settings => {
                command_sender.send_system(SystemCommand::OpenSettings);
            }
            Self::ChunkStoreBrowser => {
                command_sender.send_system(SystemCommand::OpenChunkStoreBrowser);
            }
        }
    }

    pub fn without_fragment(self) -> Self {
        match self {
            Self::Settings
            | Self::ChunkStoreBrowser
            | Self::IntraRecordingSelection(..)
            | Self::HttpUrl(..)
            | Self::RedapProxy(..)
            | Self::RedapCatalog(..)
            | Self::RedapEntry(..)
            | Self::WebEventListener => self,

            #[cfg(not(target_arch = "wasm32"))]
            Self::FilePath(..) => self,

            Self::RedapDatasetSegment(uri) => Self::RedapDatasetSegment(uri.without_fragment()),
            Self::WebViewerUrl {
                base_url,
                mut url_parameters,
            } => {
                for url in &mut url_parameters {
                    *url = url.clone().without_fragment();
                }

                Self::WebViewerUrl {
                    base_url,
                    url_parameters,
                }
            }
        }
    }

    /// Fragments of the URL if supported.
    pub fn fragment_mut(&mut self) -> Option<&mut re_uri::Fragment> {
        match self {
            Self::IntraRecordingSelection(..) => None,
            Self::HttpUrl(..) => None,
            #[cfg(not(target_arch = "wasm32"))]
            Self::FilePath(..) => None,
            Self::RedapDatasetSegment(uri) => Some(&mut uri.fragment),
            Self::RedapProxy(..) => None,
            Self::RedapCatalog(..) => None,
            Self::RedapEntry(..) => None,
            Self::WebEventListener => None,
            Self::WebViewerUrl {
                base_url: _,
                url_parameters,
            } => {
                if url_parameters.len() == 1 {
                    url_parameters.first_mut().fragment_mut()
                } else {
                    None
                }
            }
            Self::Settings => None,
            Self::ChunkStoreBrowser => None,
        }
    }
}

/// Combines a base url, for example a web viewer url
/// with a list of content urls to open.
pub fn combine_with_base_url(
    base_url: Option<&Url>,
    urls: impl IntoIterator<Item = String>,
) -> anyhow::Result<String> {
    let mut urls = urls.into_iter();
    // Combine the URL(s) with the web viewer base URL if provided.
    if let Some(base_url) = base_url {
        let mut share_url = base_url.clone();

        // Use the form_urlencoded::Serializer to build the query string with multiple "url" parameters.
        // It's important to not just append the strings, since we have to take care of correctly escaping.
        let mut serializer = url::form_urlencoded::Serializer::new(String::new());
        for url in urls {
            serializer.append_pair("url", &url);
        }
        share_url.set_query(Some(&serializer.finish()));

        Ok(share_url.to_string())
    } else if let Some(url) = urls.next()
        && urls.next().is_none()
    {
        Ok(url)
    } else {
        Err(anyhow::anyhow!(
            "Can't share more than one URL without a web viewer base URL"
        ))
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn handle_web_event_listener(_egui_ctx: &egui::Context, _command_sender: &CommandSender) {
    re_log::warn!("{WEB_EVENT_LISTENER_SCHEME:?} urls are only available on the web viewer.");
}

#[cfg(target_arch = "wasm32")]
fn handle_web_event_listener(egui_ctx: &egui::Context, command_sender: &CommandSender) {
    use std::ops::ControlFlow;
    use std::sync::Arc;

    use re_log::ResultExt as _;
    use re_log_encoding::rrd::stream_from_http::HttpMessage;

    // Process an rrd when it's posted via `window.postMessage`
    let (tx, rx) = re_log_channel::log_channel(re_log_channel::LogSource::RrdWebEvent);
    let egui_ctx = egui_ctx.clone();
    re_log_encoding::rrd::stream_from_http::stream_rrd_from_event_listener(Arc::new({
        move |msg| {
            egui_ctx.request_repaint_after(std::time::Duration::from_millis(10));

            match msg {
                HttpMessage::LogMsg(msg) => {
                    if tx.send(msg.into()).is_ok() {
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
                    tx.quit(Some(Box::new(err)))
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
    use re_log_channel::LogSource;
    use re_log_types::{EntryId, StoreId, StoreKind, TableId};
    use re_uri::external::url::{self, Url};
    use re_uri::{CatalogUri, DatasetSegmentUri, Fragment};

    use super::ViewerOpenUrl;
    use crate::{DisplayMode, Item, StoreHub};

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

        // DatasetSegmentUri
        let url = format!("rerun://127.0.0.1:1234/dataset/{entry_id}?segment_id=pid");
        assert_eq!(
            ViewerOpenUrl::from_str(&url).unwrap(),
            ViewerOpenUrl::RedapDatasetSegment(url.parse().unwrap())
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
                ViewerOpenUrl::HttpUrl(Url::parse("https://example.com/data.rrd").unwrap())
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
                url_parameters: vec1::vec1![ViewerOpenUrl::HttpUrl(
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
                    ViewerOpenUrl::HttpUrl(Url::parse("https://example.com/data.rrd").unwrap())
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
            let result = url.parse::<ViewerOpenUrl>();
            assert!(result.is_err(), "Expected error for {url}: {result:?}");
        }
    }

    #[test]
    fn test_viewer_open_url_from_display_mode() {
        let store_hub = StoreHub::test_hub();

        // RedapServer
        assert_eq!(
            ViewerOpenUrl::from_display_mode(
                &store_hub,
                &DisplayMode::RedapServer("rerun://localhost:51234".parse().unwrap()),
            )
            .unwrap(),
            ViewerOpenUrl::RedapCatalog("rerun://localhost:51234".parse().unwrap())
        );

        // LocalTable
        assert!(
            ViewerOpenUrl::from_display_mode(
                &store_hub,
                &DisplayMode::LocalTable(TableId::new("test_table".to_owned())),
            )
            .is_err()
        );

        // RedapEntry
        let origin = "rerun://localhost:51234".parse().unwrap();
        let entry_uri = re_uri::EntryUri::new(origin, EntryId::new());
        assert_eq!(
            ViewerOpenUrl::from_display_mode(
                &store_hub,
                &DisplayMode::RedapEntry(entry_uri.clone()),
            )
            .unwrap(),
            ViewerOpenUrl::RedapEntry(entry_uri.clone())
        );

        let dummy_mode = DisplayMode::RedapEntry(entry_uri);

        assert_eq!(
            ViewerOpenUrl::from_display_mode(
                &store_hub,
                &DisplayMode::Settings(Box::new(dummy_mode.clone()))
            )
            .unwrap(),
            ViewerOpenUrl::Settings
        );

        assert_eq!(
            ViewerOpenUrl::from_display_mode(
                &store_hub,
                &DisplayMode::ChunkStoreBrowser(Box::new(dummy_mode))
            )
            .unwrap(),
            ViewerOpenUrl::ChunkStoreBrowser
        );

        // Local recordings is handled in `test_viewer_open_url_from_local_recordings_display_mode`
    }

    #[test]
    fn test_viewer_open_url_from_local_recordings_display_mode() {
        let mut store_hub = StoreHub::test_hub();

        fn add_store(store_hub: &mut StoreHub, data_source: Option<LogSource>) -> StoreId {
            let store_id = StoreId::random(StoreKind::Recording, "test");
            let mut entity_db = EntityDb::new(store_id.clone());
            entity_db.data_source = data_source;
            store_hub.insert_entity_db(entity_db);
            store_hub.set_active_recording(store_id.clone());
            store_id
        }

        // originating from a file.
        let id = add_store(
            &mut store_hub,
            Some(LogSource::File {
                path: std::path::PathBuf::from("/path/to/test.rrd"),
                follow: false,
            }),
        );
        assert_eq!(
            ViewerOpenUrl::from_display_mode(&store_hub, &DisplayMode::LocalRecordings(id))
                .unwrap(),
            ViewerOpenUrl::FilePath(std::path::PathBuf::from("/path/to/test.rrd"))
        );

        // originating from HTTP stream.
        let id = add_store(
            &mut store_hub,
            Some(LogSource::HttpStream {
                url: "https://example.com/recording.rrd".to_owned(),
                follow: false,
            }),
        );
        assert_eq!(
            ViewerOpenUrl::from_display_mode(&store_hub, &DisplayMode::LocalRecordings(id))
                .unwrap(),
            ViewerOpenUrl::HttpUrl("https://example.com/recording.rrd".parse().unwrap())
        );

        // originating from SDK (not possible).
        let id = add_store(&mut store_hub, Some(LogSource::Sdk));
        assert!(
            ViewerOpenUrl::from_display_mode(&store_hub, &DisplayMode::LocalRecordings(id))
                .is_err(),
        );

        // originating from stdin (not possible).
        let id = add_store(&mut store_hub, Some(LogSource::Stdin));
        assert!(
            ViewerOpenUrl::from_display_mode(&store_hub, &DisplayMode::LocalRecordings(id))
                .is_err(),
        );

        // originating from web event listener.
        let id = add_store(&mut store_hub, Some(LogSource::RrdWebEvent));
        assert_eq!(
            ViewerOpenUrl::from_display_mode(&store_hub, &DisplayMode::LocalRecordings(id))
                .unwrap(),
            ViewerOpenUrl::WebEventListener
        );

        // originating from JS channel (not possible).
        let id = add_store(
            &mut store_hub,
            Some(LogSource::JsChannel {
                channel_name: "test_channel".to_owned(),
            }),
        );
        assert!(
            ViewerOpenUrl::from_display_mode(&store_hub, &DisplayMode::LocalRecordings(id))
                .is_err(),
        );

        // originating from Redap gRPC stream.
        let entry_id = EntryId::new();
        let uri = format!("rerun://127.0.0.1:1234/dataset/{entry_id}?segment_id=pid");
        let id = add_store(
            &mut store_hub,
            Some(LogSource::RedapGrpcStream {
                uri: uri.parse().unwrap(),
                select_when_loaded: false,
            }),
        );

        let mut uri: re_uri::DatasetSegmentUri = uri.parse().unwrap();

        assert_eq!(
            ViewerOpenUrl::from_display_mode(&store_hub, &DisplayMode::LocalRecordings(id.clone()))
                .unwrap(),
            ViewerOpenUrl::RedapDatasetSegment(uri.clone())
        );

        let fragment = Fragment {
            selection: Some(re_log_types::DataPath {
                entity_path: EntityPath::from_single_string("test/entity"),
                instance: None,
                component: None,
            }),
            when: Some((
                re_chunk::TimelineName::new("test"),
                re_log_types::TimeCell {
                    typ: re_log_types::TimeType::DurationNs,
                    value: re_log_types::NonMinI64::ONE,
                },
            )),
            time_selection: None,
        };

        uri.fragment = fragment.clone();

        let mut url =
            ViewerOpenUrl::from_display_mode(&store_hub, &DisplayMode::LocalRecordings(id))
                .unwrap();

        *url.fragment_mut().unwrap() = fragment;

        assert_eq!(url, ViewerOpenUrl::RedapDatasetSegment(uri),);

        // originating from message proxy.
        let uri = "rerun://localhost:51234/proxy";
        let id = add_store(
            &mut store_hub,
            Some(LogSource::MessageProxy(uri.parse().unwrap())),
        );
        assert_eq!(
            ViewerOpenUrl::from_display_mode(&store_hub, &DisplayMode::LocalRecordings(id))
                .unwrap(),
            ViewerOpenUrl::RedapProxy(uri.parse().unwrap())
        );

        // with no data source (not possible).
        let id = add_store(&mut store_hub, None);
        assert!(
            ViewerOpenUrl::from_display_mode(&store_hub, &DisplayMode::LocalRecordings(id))
                .is_err(),
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
            ViewerOpenUrl::HttpUrl(Url::parse("https://example.com/data.rrd").unwrap())
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
        let uri = format!("rerun://127.0.0.1:1234/dataset/{entry_id}?segment_id=pid");
        assert_eq!(
            ViewerOpenUrl::RedapDatasetSegment(uri.parse().unwrap())
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
                url_parameters: vec1::vec1![ViewerOpenUrl::HttpUrl(
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
                    ViewerOpenUrl::HttpUrl(Url::parse("https://example.com/bar.rrd").unwrap()),
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
            ViewerOpenUrl::HttpUrl("https://example.com/data.rrd".parse().unwrap())
                .sharable_url(base_url_param)
                .unwrap(),
            "https://foo.com/test?url=https%3A%2F%2Fexample.com%2Fdata.rrd"
        );

        assert_eq!(
            ViewerOpenUrl::RedapDatasetSegment(
                "rerun://127.0.0.1:1234/dataset/1830B33B45B963E7774455beb91701ae?segment_id=pid"
                    .parse()
                    .unwrap()
            )
            .sharable_url(base_url_param)
            .unwrap(),
            format!(
                "https://foo.com/test?url=rerun%3A%2F%2F127.0.0.1%3A1234%2Fdataset%2F1830B33B45B963E7774455beb91701ae%3Fsegment_id%3Dpid"
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
                url_parameters: vec1::vec1![ViewerOpenUrl::HttpUrl(
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
                    ViewerOpenUrl::HttpUrl(Url::parse("https://example.com/bar.rrd").unwrap()),
                    ViewerOpenUrl::RedapProxy("rerun://localhost:51234/proxy".parse().unwrap())
                ],
            }
            .sharable_url(base_url_param)
            .unwrap(),
            "https://foo.com/test?url=https%3A%2F%2Fexample.com%2Fbar.rrd&url=rerun%3A%2F%2Flocalhost%3A51234%2Fproxy",
        );
    }

    #[test]
    fn test_fragment_uri() {
        let uri_out = [
            (
                "rerun+http://localhost:51234/",
                ViewerOpenUrl::RedapCatalog(CatalogUri {
                    origin: "rerun+http://localhost:51234".parse().unwrap(),
                }),
            ),
            (
                "rerun+http://localhost:51234/dataset/187A3200CAE4DD795748a7ad187e21a3?segment_id=6977dcfd524a45b3b786c9a5a0bde4e1",
                ViewerOpenUrl::RedapDatasetSegment(DatasetSegmentUri {
                    origin: "rerun+http://localhost:51234".parse().unwrap(),
                    dataset_id: "187A3200CAE4DD795748a7ad187e21a3".parse().unwrap(),
                    segment_id: "6977dcfd524a45b3b786c9a5a0bde4e1".parse().unwrap(),
                    fragment: Default::default(),
                }),
            ),
            (
                "rerun+http://localhost:51234/dataset/187A3200CAE4DD795748a7ad187e21a3?segment_id=6977dcfd524a45b3b786c9a5a0bde4e1#time_selection=stable_time@+1.096s..+2.097s",
                ViewerOpenUrl::RedapDatasetSegment(DatasetSegmentUri {
                    origin: "rerun+http://localhost:51234".parse().unwrap(),
                    dataset_id: "187A3200CAE4DD795748a7ad187e21a3".parse().unwrap(),
                    segment_id: "6977dcfd524a45b3b786c9a5a0bde4e1".parse().unwrap(),
                    fragment: re_uri::Fragment {
                        time_selection: Some("stable_time@+1.096s..+2.097s".parse().unwrap()),
                        ..Default::default()
                    },
                }),
            ),
            (
                "rerun+http://localhost:51234/dataset/187A3200CAE4DD795748a7ad187e21a3?segment_id=6977dcfd524a45b3b786c9a5a0bde4e1#time_selection=stable_time@+1.096s..+2.097s&when=stable_time@+3.990s",
                ViewerOpenUrl::RedapDatasetSegment(DatasetSegmentUri {
                    origin: "rerun+http://localhost:51234".parse().unwrap(),
                    dataset_id: "187A3200CAE4DD795748a7ad187e21a3".parse().unwrap(),
                    segment_id: "6977dcfd524a45b3b786c9a5a0bde4e1".parse().unwrap(),
                    fragment: re_uri::Fragment {
                        when: Some((
                            "stable_time".into(),
                            re_log_types::TimeCell::from_str("+3.990s").unwrap(),
                        )),
                        time_selection: Some("stable_time@+1.096s..+2.097s".parse().unwrap()),
                        ..Default::default()
                    },
                }),
            ),
        ];

        for (uri, expected) in uri_out {
            eprintln!("uri: {uri}");
            match ViewerOpenUrl::from_str(uri) {
                Ok(got) => {
                    assert_eq!(got, expected);
                }
                Err(err) => {
                    DatasetSegmentUri::from_str(uri).unwrap();
                    panic!("{err}");
                }
            }
        }
    }
}
