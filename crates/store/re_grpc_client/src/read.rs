use re_async::AsyncRuntimeHandle;
use re_log_encoding::ToApplication as _;
use re_protos::sdk_comms::v1alpha1::message_proxy_service_client::MessageProxyServiceClient;
use re_protos::sdk_comms::v1alpha1::{ReadMessagesRequest, ReadMessagesResponse};
use tokio_stream::StreamExt as _;

use crate::{MAX_DECODING_MESSAGE_SIZE, StreamError, TonicStatusError};

/// Read log messages from a proxy server.
///
/// This is used by the viewer to _receive_ log messages.
pub fn stream(
    async_runtime: &AsyncRuntimeHandle,
    uri: re_uri::ProxyUri,
) -> re_log_channel::LogReceiver {
    re_log::debug!(?uri, "Loading via gRPC…");

    let (tx, rx) =
        re_log_channel::log_channel(re_log_channel::LogSource::MessageProxy(uri.clone()));

    async_runtime.spawn_future(async move {
        if let Err(err) = stream_async(uri, &tx).await {
            tx.quit(Some(Box::new(err))).ok();
        }
    });

    rx
}

async fn stream_async(
    uri: re_uri::ProxyUri,
    tx: &re_log_channel::LogSender,
) -> Result<(), StreamError> {
    let mut client = {
        let url = uri.origin.as_url();

        let tonic_client = cfg_select! {
            target_arch = "wasm32" => {
                tonic_web_wasm_client::Client::new_with_options(
                    url,
                    tonic_web_wasm_client::options::FetchOptions::new(),
                )
            }
            _ => {
                tonic::transport::Endpoint::new(url)?
                    .http2_adaptive_window(true) // Optimize for throughput
                    .connect()
                    .await?
            }
        };

        MessageProxyServiceClient::new(tonic_client)
            .max_decoding_message_size(MAX_DECODING_MESSAGE_SIZE)
    };

    re_log::debug!(?uri, "Streaming messages from gRPC endpoint");

    let mut stream = client
        .read_messages(ReadMessagesRequest {})
        .await
        .map_err(TonicStatusError::from)?
        .into_inner();

    let mut app_id_cache = re_log_encoding::CachingApplicationIdInjector::default();
    #[cfg(target_arch = "wasm32")]
    let mut last_yield = web_time::Instant::now();

    loop {
        match stream.try_next().await {
            Ok(Some(ReadMessagesResponse {
                log_msg: Some(log_msg_proto),
            })) => {
                let mut log_msg = log_msg_proto.to_application((&mut app_id_cache, None))?;

                if let Some(metadata_key) = re_sorbet::TimestampLocation::IPCDecode.metadata_key() {
                    // Insert the timestamp metadata into the Arrow message for accurate e2e latency measurements:
                    log_msg.insert_arrow_record_batch_metadata(
                        metadata_key.to_owned(),
                        re_sorbet::timestamp_metadata::now_timestamp(),
                    );
                }

                cfg_select! {
                    target_arch = "wasm32" => {
                        let mut msg = log_msg.into();
                        loop {
                            match tx.try_send(msg) {
                                Ok(()) => break,
                                Err(re_log_channel::TrySendError::Full(unsent_msg)) => {
                                    msg = *unsent_msg;
                                    re_async::yield_now().await;
                                    last_yield = web_time::Instant::now();
                                }
                                Err(re_log_channel::TrySendError::Disconnected(_)) => {
                                    re_log::debug!("gRPC stream smart channel closed");
                                    return Ok(());
                                }
                            }
                        }

                        if last_yield.elapsed() >= web_time::Duration::from_millis(10) {
                            re_async::yield_now().await;
                            last_yield = web_time::Instant::now();
                        }
                    }
                    _ => {
                        if tx.send(log_msg.into()).is_err() {
                            re_log::debug!("gRPC stream smart channel closed");
                            break;
                        }
                    }
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

            Err(err) => {
                return Err(err.into());
            }
        }
    }

    Ok(())
}
