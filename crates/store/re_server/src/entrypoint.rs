use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::Context as _;
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
    /// IP address to listen on.
    #[clap(long, default_value = "0.0.0.0")]
    pub host: String,

    /// Port to bind to.
    #[clap(long, short = 'p', default_value_t = 51234)]
    pub port: u16,

    // TODO(ab): expose this to the CLI
    /// Load a set of RRDs as a dataset (can be specified multiple times).
    ///
    /// All the paths in the path collections must point at RRD files. Directories are not
    /// supported.
    #[clap(skip)]
    pub datasets: Vec<NamedPathCollection>,

    /// Load a directory of RRD as dataset (can be specified multiple times).
    /// You can specify only a path or provide a name such as
    /// `-d my_dataset=./path/to/files`
    #[clap(long = "dataset", short = 'd', value_name = "[NAME=]DIR_PATH")]
    pub dataset_prefixes: Vec<NamedPath>,

    /// Load a lance file as a table (can be specified multiple times).
    /// You can specify only a path or provide a name such as
    /// `-t my_table=./path/to/table`
    #[clap(long = "table", short = 't', value_name = "[NAME=]TABLE_PATH")]
    pub tables: Vec<NamedPath>,

    /// Artificial latency to add to each request (in milliseconds).
    #[clap(long, default_value_t = 0)]
    pub latency_ms: u16,

    /// Artificial bandwidth limit for responses (e.g. '10MB' for 10 megabytes per second).
    #[clap(long, value_parser = parse_bandwidth_limit)]
    pub bandwidth_limit: Option<u64>,
}

fn parse_bandwidth_limit(s: &str) -> Result<u64, String> {
    re_format::parse_bytes(s)
        .and_then(|b| u64::try_from(b).ok())
        .ok_or_else(|| format!("expected a bandwidth like '10MB' or '1GiB', got {s:?}"))
}

impl Default for Args {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".into(),
            port: 51234,
            datasets: vec![],
            dataset_prefixes: vec![],
            tables: vec![],
            latency_ms: 0,
            bandwidth_limit: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NamedPath {
    pub name: Option<String>,
    pub path: PathBuf,
}

/// A named collection of paths.
#[derive(Debug, Clone)]
pub struct NamedPathCollection {
    pub name: String,
    pub paths: Vec<PathBuf>,
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
    ///
    /// The returned address is one you can connect to, e.g. 127.0.0.1 instead of 0.0.0.0.
    pub async fn create_server_handle(self) -> anyhow::Result<(ServerHandle, SocketAddr)> {
        let Self {
            host: ip,
            port,
            datasets,
            dataset_prefixes,
            tables,
            latency_ms,
            bandwidth_limit,
        } = self;

        let rerun_cloud_server = {
            use re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudServiceServer;

            let mut builder = crate::RerunCloudHandlerBuilder::new();

            for NamedPathCollection { name, paths } in datasets {
                builder = builder
                    .with_rrds_as_dataset(
                        name,
                        paths,
                        re_protos::common::v1alpha1::ext::IfDuplicateBehavior::Error,
                        crate::OnError::Continue,
                    )
                    .await?;
            }

            for dataset_prefix in &dataset_prefixes {
                builder = builder
                    .with_directory_as_dataset(
                        dataset_prefix,
                        re_protos::common::v1alpha1::ext::IfDuplicateBehavior::Error,
                        crate::OnError::Continue,
                    )
                    .await?;
            }

            #[cfg_attr(not(feature = "lance"), expect(clippy::never_loop))]
            for table in &tables {
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

        let ip = ip.parse().with_context(|| format!("IP: {ip:?}"))?;
        let ip_port = SocketAddr::new(ip, port);

        let server_builder = ServerBuilder::default()
            .with_address(ip_port)
            .with_service(rerun_cloud_server)
            .with_http_route(
                "/version",
                axum::routing::get(async move || re_build_info::build_info!().to_string()),
            )
            .with_artificial_latency(std::time::Duration::from_millis(latency_ms as _))
            .with_bandwidth_limit(bandwidth_limit);

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
