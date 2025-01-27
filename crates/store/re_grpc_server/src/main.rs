use re_grpc_server::{serve, DEFAULT_MEMORY_LIMIT, DEFAULT_SERVER_PORT};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), tonic::transport::Error> {
    re_log::setup_logging();

    serve(DEFAULT_SERVER_PORT, DEFAULT_MEMORY_LIMIT).await
}
