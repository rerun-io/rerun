use re_log_types::LogMsg;
use re_smart_channel::Receiver;

/// `on_msg` can be used to wake up the UI thread on Wasm.
pub fn connect_to_ws_url(
    url: &str,
    on_msg: Option<Box<dyn Fn() + Send + Sync>>,
) -> anyhow::Result<Receiver<LogMsg>> {
    let (tx, rx) = re_smart_channel::smart_channel(
        re_smart_channel::SmartMessageSource::WsClient {
            ws_server_url: url.to_owned(),
        },
        re_smart_channel::SmartChannelSource::WsClient {
            ws_server_url: url.to_owned(),
        },
    );

    re_log::info!("Connecting to WebSocket server at {url:?}…");

    let callback = {
        let url = url.to_owned();
        move |binary: Vec<u8>| match re_ws_comms::decode_log_msg(&binary) {
            Ok(log_msg) => {
                if tx.send(log_msg).is_ok() {
                    if let Some(on_msg) = &on_msg {
                        on_msg();
                    }
                    std::ops::ControlFlow::Continue(())
                } else {
                    re_log::info_once!("Closing connection to {url}");
                    std::ops::ControlFlow::Break(())
                }
            }
            Err(err) => {
                re_log::error!("Failed to parse message: {err}");
                std::ops::ControlFlow::Break(())
            }
        }
    };

    re_ws_comms::viewer_to_server(url.to_owned(), callback)?;
    Ok(rx)
}
