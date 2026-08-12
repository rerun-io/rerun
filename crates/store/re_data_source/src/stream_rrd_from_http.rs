use std::ops::ControlFlow;
use std::sync::Arc;

use re_log::ResultExt as _;
use re_log_encoding::stream_from_http::{HttpMessage, stream_from_http};

/// Stream an rrd file from a HTTP server.
///
/// `on_msg` can be used to wake up the UI thread on Wasm.
pub fn stream_from_http_to_channel(url: String) -> re_log_channel::LogReceiver {
    let (tx, rx) =
        re_log_channel::log_channel(re_log_channel::LogSource::HttpStream { url: url.clone() });
    stream_from_http(
        url.clone(),
        Arc::new(move |msg| match msg {
            HttpMessage::LogMsg(msg) => {
                if tx.send(msg.into()).is_ok() {
                    ControlFlow::Continue(())
                } else {
                    re_log::info_once!("Closing connection to {url}");
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
        }),
    );
    rx
}
