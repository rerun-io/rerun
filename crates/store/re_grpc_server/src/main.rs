use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::net::SocketAddrV4;

use re_grpc_server::{serve, shutdown, DEFAULT_MEMORY_LIMIT, DEFAULT_SERVER_PORT};

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    re_log::setup_logging();

    serve(
        SocketAddr::V4(SocketAddrV4::new(
            Ipv4Addr::new(0, 0, 0, 0),
            DEFAULT_SERVER_PORT,
        )),
        DEFAULT_MEMORY_LIMIT,
        shutdown::never(),
    )
    .await?;

    Ok(())
}
