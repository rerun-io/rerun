//! Communications with an RRDP GRPC server.

use re_log_types::LogMsg;

/// Stream an rrd file from an RRDP server.
///
/// `on_msg` can be used to wake up the UI thread on Wasm.
pub fn stream_recording(
    url: String,
    _on_msg: Option<Box<dyn Fn() + Send + Sync>>,
) -> re_smart_channel::Receiver<LogMsg> {
    let (tx, rx) = re_smart_channel::smart_channel(
        re_smart_channel::SmartMessageSource::RrdpStream { url: url.clone() },
        re_smart_channel::SmartChannelSource::RrdpStream { url: url.clone() },
    );

    // TODO(jleibs): Implement the actual streaming logic here

    rx
}
