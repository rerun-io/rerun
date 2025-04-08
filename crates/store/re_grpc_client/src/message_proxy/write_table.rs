use re_protos::sdk_comms::v1alpha1::message_proxy_service_client::MessageProxyServiceClient;

#[cfg(not(target_arch = "wasm32"))]
pub type TableClient = MessageProxyServiceClient<tonic::transport::Channel>;

#[cfg(not(target_arch = "wasm32"))]
pub async fn table_client(
    origin: re_uri::Origin,
) -> Result<MessageProxyServiceClient<tonic::transport::Channel>, crate::redap::ConnectionError> {
    let channel = crate::redap::channel(origin).await?;
    Ok(MessageProxyServiceClient::new(channel)
        .max_decoding_message_size(crate::MAX_DECODING_MESSAGE_SIZE))
}
