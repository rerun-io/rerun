use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;
#[cfg(unix)]
use tokio::signal::unix::{SignalKind, signal};
#[cfg(windows)]
use tokio::signal::windows::{ctrl_break, ctrl_close};
use tracing::{info, warn};

use crate::{ServerBuilder, ServerHandle};

// ---

#[derive(Clone, Debug, clap::Parser)]
#[clap(author, version, about)]
pub struct Args {
    /// Address to listen on.
    #[clap(long, default_value = "0.0.0.0")]
    pub addr: String,

    /// Port to bind to.
    #[clap(long, short = 'p', default_value_t = 51234)]
    pub port: u16,

    /// Load a directory of RRD as dataset (can be specified multiple times).
    /// You can specify only a path or provide a name such as
    /// `-d my_dataset=./path/to/files`
    #[clap(long = "dataset", short = 'd')]
    pub datasets: Vec<NamedPath>,

    /// Load a lance file as a table (can be specified multiple times).
    /// You can specify only a path or provide a name such as
    /// `-t my_table=./path/to/table`
    #[clap(long = "table", short = 't')]
    pub tables: Vec<NamedPath>,
}

#[derive(Debug, Clone)]
pub struct NamedPath {
    pub name: Option<String>,
    pub path: PathBuf,
}

impl FromStr for NamedPath {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some((name, path)) = s.split_once('=') {
            Ok(Self {
                name: Some(name.to_owned()),
                path: PathBuf::from(path),
            })
        } else {
            Ok(Self {
                name: None,
                path: PathBuf::from(s),
            })
        }
    }
}

impl Args {
    /// Waits for the server to start, and return a handle to it together with its address.
    pub async fn create_server_handle(self) -> anyhow::Result<(ServerHandle, SocketAddr)> {
        let rerun_cloud_server = {
            use re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudServiceServer;

            let mut builder = crate::RerunCloudHandlerBuilder::new();

            for dataset in &self.datasets {
                builder = builder.with_directory_as_dataset(
                    dataset,
                    re_protos::common::v1alpha1::ext::IfDuplicateBehavior::Error,
                )?;
            }

            #[cfg_attr(not(feature = "lance"), expect(clippy::never_loop))]
            for table in &self.tables {
                cfg_if::cfg_if! {
                    if #[cfg(feature = "lance")] {
                        builder = builder
                            .with_directory_as_table(
                                table,
                                re_protos::common::v1alpha1::ext::IfDuplicateBehavior::Error,
                            )
                            .await?;
                    } else {
                        _ = table;
                        anyhow::bail!("re_server was not compiled with the 'lance' feature");
                    }
                }
            }

            RerunCloudServiceServer::new(builder.build())
                .max_decoding_message_size(re_grpc_server::MAX_DECODING_MESSAGE_SIZE)
                .max_encoding_message_size(re_grpc_server::MAX_ENCODING_MESSAGE_SIZE)
        };

        let addr = SocketAddr::new(self.addr.parse()?, self.port);

        let server_builder = ServerBuilder::default()
            .with_address(addr)
            .with_service(rerun_cloud_server)
            .with_http_route(
                "/version",
                axum::routing::get(async move || re_build_info::build_info!().to_string()),
            );

        let server = server_builder.build();

        let mut server_handle = server.start();

        let addr = server_handle.wait_for_ready().await?;

        Ok((server_handle, addr))
    }

    pub async fn run_async(self) -> anyhow::Result<()> {
        let (mut server_handle, _) = self.create_server_handle().await?;

        #[cfg(unix)]
        let mut term_signal = signal(SignalKind::terminate())?;
        #[cfg(windows)]
        let mut term_signal = ctrl_close()?;

        #[cfg(unix)]
        let mut int_signal = signal(SignalKind::interrupt())?;
        #[cfg(windows)]
        let mut int_signal = ctrl_break()?;

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
}
