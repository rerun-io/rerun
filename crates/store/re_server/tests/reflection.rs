#![cfg(not(target_arch = "wasm32"))]

use tonic_reflection::pb::v1::ServerReflectionRequest;
use tonic_reflection::pb::v1::server_reflection_client::ServerReflectionClient;
use tonic_reflection::pb::v1::server_reflection_request::MessageRequest;
use tonic_reflection::pb::v1::server_reflection_response::MessageResponse;

#[tokio::test(flavor = "multi_thread")]
async fn lists_services_through_reflection() {
    let handle = re_server::Args {
        host: "127.0.0.1".into(),
        port: 0,
        ..Default::default()
    }
    .create_server_handle()
    .await
    .expect("failed to start server");

    let addr = handle.connect_addr();
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{addr}"))
        .expect("invalid endpoint")
        .connect()
        .await
        .expect("failed to connect");
    let mut client = ServerReflectionClient::new(channel);

    let request = ServerReflectionRequest {
        host: String::new(),
        message_request: Some(MessageRequest::ListServices(String::new())),
    };
    let mut responses = client
        .server_reflection_info(tokio_stream::once(request))
        .await
        .expect("reflection request failed")
        .into_inner();
    let response = responses
        .message()
        .await
        .expect("reflection stream failed")
        .expect("reflection stream ended early");

    let Some(MessageResponse::ListServicesResponse(list)) = response.message_response else {
        panic!("unexpected reflection response: {response:?}");
    };
    let mut services: Vec<String> = list.service.into_iter().map(|s| s.name).collect();
    services.sort();

    handle.shutdown_and_wait().await;

    assert_eq!(
        services,
        [
            "grpc.reflection.v1.ServerReflection",
            "rerun.cloud.v1alpha1.RerunCloudService",
        ]
    );
}
