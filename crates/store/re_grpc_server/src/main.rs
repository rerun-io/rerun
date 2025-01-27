use re_grpc_server::{serve, DEFAULT_GRPC_ADDR, DEFAULT_MEMORY_LIMIT};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), tonic::transport::Error> {
    re_log::setup_logging();

    serve(DEFAULT_GRPC_ADDR, DEFAULT_MEMORY_LIMIT).await
}
