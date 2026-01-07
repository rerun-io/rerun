use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

use re_grpc_server::{DEFAULT_SERVER_PORT, serve, shutdown};

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    re_log::setup_logging();

    serve(
        SocketAddr::V4(SocketAddrV4::new(
            Ipv4Addr::UNSPECIFIED,
            DEFAULT_SERVER_PORT,
        )),
        Default::default(),
        shutdown::never(),
    )
    .await?;

    Ok(())
}
