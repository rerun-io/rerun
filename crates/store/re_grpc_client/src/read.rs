use re_log_encoding::ToApplication as _;
use re_protos::sdk_comms::v1alpha1::message_proxy_service_client::MessageProxyServiceClient;
use re_protos::sdk_comms::v1alpha1::{ReadMessagesRequest, ReadMessagesResponse};
use tokio_stream::StreamExt as _;

use crate::{MAX_DECODING_MESSAGE_SIZE, StreamError, TonicStatusError};

/// Yield to the browser event loop by awaiting a `setTimeout(millis)` promise.
///
/// On WASM, there is no preemptive scheduler. A tight async loop that never
/// returns `Poll::Pending` will starve the browser event loop, preventing GC,
/// rendering, and other tasks from executing. This function creates a minimal
/// yield point by scheduling a `setTimeout` callback.
#[cfg(target_arch = "wasm32")]
async fn yield_to_browser(millis: i32) {
    let promise = js_sys::Promise::new(&mut |resolve, _reject| {
        web_sys::window()
            .expect("no global `window` exists")
            .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, millis)
            .expect("Failed to call set_timeout");
    });
    wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .expect("Failed to await setTimeout promise");
}

/// Read log messages from a proxy server.
///
/// This is used by the viewer to _receive_ log messages.
pub fn stream(uri: re_uri::ProxyUri) -> re_log_channel::LogReceiver {
    re_log::debug!(?uri, "Loading via gRPC…");

    let (tx, rx) =
        re_log_channel::log_channel(re_log_channel::LogSource::MessageProxy(uri.clone()));

    crate::spawn_future(async move {
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

        #[cfg(target_arch = "wasm32")]
        let tonic_client = {
            tonic_web_wasm_client::Client::new_with_options(
                url,
                tonic_web_wasm_client::options::FetchOptions::new(),
            )
        };

        #[cfg(not(target_arch = "wasm32"))]
        let tonic_client = {
            tonic::transport::Endpoint::new(url)?
                .http2_adaptive_window(true) // Optimize for throughput
                .connect()
                .await?
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

    // On WASM, we must yield to the browser event loop periodically.
    // Without this, the tight async loop starves rendering, GC, and the
    // channel consumer, causing unbounded WASM linear memory growth.
    // When memory.grow hits the 2 GiB wasm32 ceiling, Chromium raises SIGILL. (#12723)
    #[cfg(target_arch = "wasm32")]
    let mut msgs_since_yield: u32 = 0;

    loop {
        // On WASM, check if the channel consumer is falling behind.
        // If so, pause decoding and yield until the consumer drains,
        // rather than pushing more data into unbounded memory.
        #[cfg(target_arch = "wasm32")]
        if !tx.is_empty() && tx.len() > 128 {
            yield_to_browser(1).await;
            continue;
        }

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

                if tx.send(log_msg.into()).is_err() {
                    re_log::debug!("gRPC stream smart channel closed");
                    break;
                }

                // Yield to browser event loop periodically on WASM.
                // Under sustained load, try_next().await resolves instantly
                // (Poll::Ready) every iteration, starving the single-threaded
                // event loop. A setTimeout(0) yield gives the browser one tick
                // for GC, rendering, and channel consumer to drain. (#12723)
                #[cfg(target_arch = "wasm32")]
                {
                    msgs_since_yield += 1;
                    if msgs_since_yield >= 32 {
                        msgs_since_yield = 0;
                        yield_to_browser(0).await;
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
