use std::sync::Arc;

use re_log::ResultExt as _;
use re_log_channel::{LogReceiver, LogSource};
use re_log_types::{FileSource, RecordingId};

/// Fetch a file from an HTTP URL and load it using all available data loaders.
///
/// Unlike RRD streaming which decodes incrementally, this downloads the entire file
/// first, then passes the bytes through [`re_data_loader::load_from_file_contents`].
///
/// This works for all file types supported by the data loaders (MCAP, images, 3D models, etc.).
pub fn fetch_and_load(url: &url::Url) -> LogReceiver {
    let url_string = url.to_string();

    let filename = url
        .path_segments()
        .and_then(|mut segments| segments.next_back())
        .unwrap_or("downloaded_file")
        .to_owned();

    let (tx, rx) = re_log_channel::log_channel(LogSource::HttpStream {
        url: url_string.clone(),
        follow: false,
    });

    re_log::debug!("Fetching file from {url_string:?}…");

    ehttp::fetch(
        ehttp::Request::get(&url_string).with_timeout(None),
        move |result| {
            match result {
                Ok(response) => {
                    if !response.ok {
                        re_log::error!(
                            url = url_string,
                            "Failed to fetch: {} {}",
                            response.status,
                            response.status_text
                        );
                        tx.quit(Some(Box::new(std::io::Error::other(format!(
                            "Failed to fetch file: HTTP {} {}",
                            response.status, response.status_text
                        )))))
                        .warn_on_err_once("Failed to send quit marker");
                        return;
                    }

                    re_log::debug!(
                        "Fetched {url_string} ({}), loading…",
                        re_format::format_bytes(response.bytes.len() as f64)
                    );

                    let bytes: Arc<[u8]> = response.bytes.into();

                    let shared_recording_id = RecordingId::random();
                    let settings = re_data_loader::DataLoaderSettings {
                        force_store_info: true,
                        ..re_data_loader::DataLoaderSettings::recommended(shared_recording_id)
                    };

                    if let Err(err) = re_data_loader::load_from_file_contents(
                        &settings,
                        FileSource::Uri,
                        &std::path::PathBuf::from(&filename),
                        std::borrow::Cow::Borrowed(&bytes),
                        &tx,
                    ) {
                        re_log::error!(path = filename, "Failed to load: {err}");
                        tx.quit(Some(Box::new(err)))
                            .warn_on_err_once("Failed to send quit marker");
                    }

                    // `load_from_file_contents` internally calls `send()` which calls `tx.quit(None)`
                    // when all data has been forwarded, so we don't need to call it here on success.
                }
                Err(err) => {
                    re_log::error!(url = url_string, "Failed to fetch: {err}");
                    tx.quit(Some(Box::new(std::io::Error::other(format!(
                        "Failed to fetch file: {err}",
                    )))))
                    .warn_on_err_once("Failed to send quit marker");
                }
            }
        },
    );

    rx
}
