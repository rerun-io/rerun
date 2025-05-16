use std::net::SocketAddr;
use std::path::PathBuf;

use tokio::signal::unix::{SignalKind, signal};
use tracing::{info, warn};

use crate::ServerBuilder;

// ---

#[derive(Clone, Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    /// Address to listen on.
    #[clap(long, default_value = "0.0.0.0")]
    addr: String,

    /// Port to bind to.
    #[clap(long, short = 'p', default_value_t = 51234)]
    port: u16,

    /// Load a directory of RRD as dataset (can be specified multiple times).
    #[clap(long = "dataset", short = 'd')]
    datasets: Vec<PathBuf>,
}

pub fn run<I, T>(args: I) -> Result<(), Box<dyn std::error::Error>>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    use clap::Parser as _;
    let args = Args::parse_from(args);

    info!("starting redap-server...");

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(run_async(args))?;

    Ok(())
}

async fn run_async(args: Args) -> anyhow::Result<()> {
    let frontend_server = {
        use re_protos::frontend::v1alpha1::frontend_service_server::FrontendServiceServer;

        let mut builder = crate::FrontendHandlerBuilder::new();

        for dataset in &args.datasets {
            builder = builder.with_directory_as_dataset(dataset)?;
        }

        FrontendServiceServer::new(builder.build())
    };

    let addr = SocketAddr::new(args.addr.parse()?, args.port);

    let server_builder = ServerBuilder::default()
        .with_address(addr)
        .with_service(frontend_server);

    let server = server_builder.build();

    let mut server_handle = server.start();

    server_handle.wait_for_ready().await?;

    let mut term_signal = signal(SignalKind::terminate())?;
    let mut int_signal = signal(SignalKind::interrupt())?;

    tokio::select! {
        _ = term_signal.recv() => {
            info!("received SIGTERM, gracefully shutting down");
        }

        _ = int_signal.recv() => {
            info!("received SIGINT, gracefully shutting down");
        }

        _ = server_handle.wait_for_shutdown() => {
            warn!("gRPC endpoint shut down on its own, terminating redap-server");
        }
    }

    Ok(())
}
