use re_log_encoding::protobuf_conversions::log_msg_from_proto;
use re_log_types::LogMsg;
use re_protos::sdk_comms::v1alpha1::ReadMessagesRequest;
use re_protos::sdk_comms::v1alpha1::ReadMessagesResponse;
use re_protos::sdk_comms::v1alpha1::message_proxy_service_client::MessageProxyServiceClient;
use tokio_stream::StreamExt as _;

use crate::MAX_DECODING_MESSAGE_SIZE;
use crate::StreamError;
use crate::TonicStatusError;

/// Read log messages from a proxy server.
///
/// This is used by the viewer to _receive_ log messages.
pub fn stream(
    uri: re_uri::ProxyUri,
    on_msg: Option<Box<dyn Fn() + Send + Sync>>,
) -> re_smart_channel::Receiver<LogMsg> {
    re_log::debug!("Loading {uri} via gRPC…");

    let (tx, rx) = re_smart_channel::smart_channel(
        re_smart_channel::SmartMessageSource::MessageProxy(uri.clone()),
        re_smart_channel::SmartChannelSource::MessageProxy(uri.clone()),
    );

    crate::spawn_future(async move {
        if let Err(err) = stream_async(uri, &tx, on_msg).await {
            tx.quit(Some(Box::new(err))).ok();
        }
    });

    rx
}

async fn stream_async(
    uri: re_uri::ProxyUri,
    tx: &re_smart_channel::Sender<LogMsg>,
    on_msg: Option<Box<dyn Fn() + Send + Sync>>,
) -> Result<(), StreamError> {
    let mut client = {
        let url = uri.endpoint_url();

        #[cfg(target_arch = "wasm32")]
        let tonic_client = {
            tonic_web_wasm_client::Client::new_with_options(
                url,
                tonic_web_wasm_client::options::FetchOptions::new(),
            )
        };

        #[cfg(not(target_arch = "wasm32"))]
        let tonic_client = { tonic::transport::Endpoint::new(url)?.connect().await? };

        MessageProxyServiceClient::new(tonic_client)
            .max_decoding_message_size(MAX_DECODING_MESSAGE_SIZE)
    };

    re_log::debug!("Streaming messages from gRPC endpoint {uri}");

    let mut stream = client
        .read_messages(ReadMessagesRequest {})
        .await
        .map_err(TonicStatusError)?
        .into_inner();

    loop {
        match stream.try_next().await {
            Ok(Some(ReadMessagesResponse {
                log_msg: Some(log_msg_proto),
            })) => {
                let mut log_msg = log_msg_from_proto(log_msg_proto)?;

                // Insert the timestamp metadata into the Arrow message for accurate e2e latency measurements:
                log_msg.insert_arrow_record_batch_metadata(
                    re_sorbet::timestamp_metadata::KEY_TIMESTAMP_VIEWER_IPC_DECODED.to_owned(),
                    re_sorbet::timestamp_metadata::now_timestamp(),
                );

                if tx.send(log_msg).is_err() {
                    re_log::debug!("gRPC stream smart channel closed");
                    break;
                }
                if let Some(on_msg) = &on_msg {
                    on_msg();
                }
            }

            Ok(Some(ReadMessagesResponse { log_msg: None })) => {
                re_log::debug!("empty ReadMessagesResponse");
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
