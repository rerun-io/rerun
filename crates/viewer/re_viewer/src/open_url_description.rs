use re_ui::CommandPaletteUrl;
use re_viewer_context::open_url::ViewerOpenUrl;

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

impl ViewerOpenUrlDescription {
    pub fn from_url(open_url: &ViewerOpenUrl) -> Self {
        match open_url {
            ViewerOpenUrl::IntraRecordingSelection(item) => Self {
                category: "Selection",
                target_short: item.entity_path().map(|p| p.to_string()),
            },

            ViewerOpenUrl::HttpUrl(url) => {
                let path = url.path();
                let rrd_file_name = path.split('/').next_back().map(|s| s.to_owned());

                Self {
                    category: "From http link",
                    target_short: rrd_file_name,
                }
            }

            #[cfg(not(target_arch = "wasm32"))]
            ViewerOpenUrl::FilePath(path) => Self {
                category: "File",
                target_short: path.file_name().map(|s| s.display().to_string()),
            },

            ViewerOpenUrl::RedapDatasetSegment(uri) => Self {
                category: "Segment",
                target_short: Some(uri.segment_id.clone()),
            },

            ViewerOpenUrl::RedapProxy(_) => Self {
                category: "GRPC proxy",
                target_short: None,
            },

            ViewerOpenUrl::RedapCatalog(uri) => Self {
                category: "Catalog",
                target_short: Some(uri.origin.host.to_string()),
            },

            ViewerOpenUrl::RedapEntry(uri) => Self {
                category: "Redap Entry",
                target_short: Some(uri.entry_id.to_string()),
            },

            ViewerOpenUrl::WebEventListener => Self {
                category: "Web event listener",
                target_short: None,
            },

            ViewerOpenUrl::WebViewerUrl { url_parameters, .. } => {
                if url_parameters.len() == 1 {
                    Self::from_url(url_parameters.first())
                } else {
                    Self {
                        category: "Several URLs",
                        target_short: Some(format!("{} URLs", url_parameters.len())),
                    }
                }
            }

            ViewerOpenUrl::Settings => Self {
                category: "Settings",
                target_short: None,
            },

            ViewerOpenUrl::ChunkStoreBrowser => Self {
                category: "Chunk store browser",
                target_short: None,
            },
        }
    }
}

pub fn command_palette_parse_url(url: &str) -> Option<CommandPaletteUrl> {
    let Ok(open_url) = url.parse::<ViewerOpenUrl>() else {
        return None;
    };

    Some(CommandPaletteUrl {
        url: url.to_owned(),
        command_text: format!("Open {}", ViewerOpenUrlDescription::from_url(&open_url)),
    })
}
