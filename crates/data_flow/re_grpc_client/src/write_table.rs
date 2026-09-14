use re_protos::sdk_comms::v1alpha1::message_proxy_service_client::MessageProxyServiceClient;
use re_uri::Origin;

pub type ViewerClient = MessageProxyServiceClient<tonic::transport::Channel>;

pub async fn viewer_client(origin: Origin) -> Result<ViewerClient, tonic::transport::Error> {
    let channel = channel(origin).await?;
    Ok(MessageProxyServiceClient::new(channel)
        .max_decoding_message_size(crate::MAX_DECODING_MESSAGE_SIZE))
}

pub async fn channel(origin: Origin) -> Result<tonic::transport::Channel, tonic::transport::Error> {
    use tonic::transport::Endpoint;

    let http_url = origin.as_url();

    let mut endpoint = Endpoint::new(http_url)?.tls_config(
        tonic::transport::ClientTlsConfig::new()
            .with_enabled_roots()
            .assume_http2(true),
    )?;

    endpoint = endpoint.http2_adaptive_window(true); // Optimize for throughput

    if false {
        // NOTE: Tried it, had no noticeable effects in any of my benchmarks.
        endpoint = endpoint.initial_stream_window_size(Some(4 * 1024 * 1024));
        endpoint = endpoint.initial_connection_window_size(Some(16 * 1024 * 1024));
    }

    endpoint.connect().await
}
