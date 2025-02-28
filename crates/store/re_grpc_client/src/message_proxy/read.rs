use re_log_encoding::protobuf_conversions::log_msg_from_proto;
use re_log_types::LogMsg;
use re_protos::sdk_comms::v0::message_proxy_client::MessageProxyClient;
use re_protos::sdk_comms::v0::ReadMessagesRequest;
use tokio_stream::StreamExt;

use crate::StreamError;
use crate::TonicStatusError;
use crate::MAX_DECODING_MESSAGE_SIZE;

pub fn stream(
    endpoint: re_uri::ProxyEndpoint,
    on_msg: Option<Box<dyn Fn() + Send + Sync>>,
) -> re_smart_channel::Receiver<LogMsg> {
    re_log::debug!("Loading {endpoint} via gRPC…");

    let url = format!("{endpoint}");
    let (tx, rx) = re_smart_channel::smart_channel(
        re_smart_channel::SmartMessageSource::MessageProxy { url: url.clone() },
        re_smart_channel::SmartChannelSource::MessageProxy { url },
    );

    crate::spawn_future(async move {
        if let Err(err) = stream_async(endpoint, &tx, on_msg).await {
            tx.quit(Some(Box::new(err))).ok();
        }
    });

    rx
}

async fn stream_async(
    endpoint: re_uri::ProxyEndpoint,
    tx: &re_smart_channel::Sender<LogMsg>,
    on_msg: Option<Box<dyn Fn() + Send + Sync>>,
) -> Result<(), StreamError> {
    let mut client = {
        let url = endpoint.origin.as_url();

        #[cfg(target_arch = "wasm32")]
        let tonic_client = {
            tonic_web_wasm_client::Client::new_with_options(
                url,
                tonic_web_wasm_client::options::FetchOptions::new(),
            )
        };

        #[cfg(not(target_arch = "wasm32"))]
        let tonic_client = { tonic::transport::Endpoint::new(url)?.connect().await? };

        // TODO(#8411): figure out the right size for this
        MessageProxyClient::new(tonic_client).max_decoding_message_size(MAX_DECODING_MESSAGE_SIZE)
    };

    re_log::debug!("Streaming messages from gRPC endpoint {endpoint}");

    let mut stream = client
        .read_messages(ReadMessagesRequest {})
        .await
        .map_err(TonicStatusError)?
        .into_inner();

    loop {
        match stream.try_next().await {
            Ok(Some(msg)) => {
                let msg = log_msg_from_proto(msg)?;
                if tx.send(msg).is_err() {
                    re_log::debug!("gRPC stream smart channel closed");
                    break;
                }
                if let Some(on_msg) = &on_msg {
                    on_msg();
                }
            }

            // Stream closed
            Ok(None) => {
                re_log::debug!("gRPC stream disconnected");
                break;
            }

            Err(_) => {
                re_log::debug!("gRPC stream timed out");
                break;
            }
        }
    }

    Ok(())
}
