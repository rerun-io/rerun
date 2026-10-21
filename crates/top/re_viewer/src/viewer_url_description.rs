use re_viewer_context::viewer_url::ViewerUrl;

/// A description of what happens when opening a [`ViewerUrl`].
pub struct ViewerUrlDescription {
    /// The general category of this URL.
    pub category: &'static str,

    /// The specific target of this URL if known.
    ///
    /// This is always shorter than the original URL.
    pub target_short: Option<String>,
}

impl std::fmt::Display for ViewerUrlDescription {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(target) = &self.target_short {
            write!(f, "{}: {target}", self.category)
        } else {
            write!(f, "{}", self.category)
        }
    }
}

impl ViewerUrlDescription {
    pub fn from_url(open_url: &ViewerUrl) -> Self {
        match open_url {
            ViewerUrl::IntraRecordingSelection(item) => Self {
                category: "Selection",
                target_short: item.entity_path().map(|p| p.to_string()),
            },

            ViewerUrl::HttpUrl(url) => {
                let path = url.path();
                let rrd_file_name = path.split('/').next_back().map(|s| s.to_owned());

                Self {
                    category: "HTTP url",
                    target_short: rrd_file_name,
                }
            }

            #[cfg(not(target_arch = "wasm32"))]
            ViewerUrl::FilePath(path) => Self {
                category: "File",
                target_short: path.file_name().map(|s| s.display().to_string()),
            },

            ViewerUrl::RedapDataset(uri) => {
                if let Some(segment_id) = &uri.segment_id {
                    Self {
                        category: "Segment",
                        target_short: Some(segment_id.as_str().to_owned()),
                    }
                } else {
                    Self {
                        category: "Dataset",
                        target_short: Some(uri.dataset_id.to_string()),
                    }
                }
            }

            ViewerUrl::RedapProxy(_) => Self {
                category: "GRPC proxy",
                target_short: None,
            },

            ViewerUrl::RedapCatalog(uri) => Self {
                category: "Catalog",
                target_short: Some(uri.origin.host.to_string()),
            },

            ViewerUrl::RedapEntry(uri) => Self {
                category: "Redap entry",
                target_short: Some(uri.entry_id.to_string()),
            },

            ViewerUrl::RedapFolder(uri) => Self {
                category: "Folder",
                target_short: Some(uri.path.clone()),
            },

            ViewerUrl::WebEventListener => Self {
                category: "Web event listener",
                target_short: None,
            },

            ViewerUrl::WebViewer { url_parameters, .. } => {
                if url_parameters.len() == 1 {
                    Self::from_url(url_parameters.first())
                } else {
                    Self {
                        category: "Several URLs",
                        target_short: Some(format!("{} URLs", url_parameters.len())),
                    }
                }
            }

            ViewerUrl::Settings => Self {
                category: "Settings",
                target_short: None,
            },

            ViewerUrl::ChunkStoreBrowser { .. } => Self {
                category: "Chunk store browser",
                target_short: None,
            },

            ViewerUrl::LocalRecording(store_id) => Self {
                category: "Recording",
                target_short: Some(store_id.recording_id().to_string()),
            },

            ViewerUrl::LocalTable(table_id) => Self {
                category: "Table",
                target_short: Some(table_id.as_str().to_owned()),
            },
        }
    }
}
