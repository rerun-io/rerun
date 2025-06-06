use re_data_loader::DataLoaderError;
use re_log_types::StoreId;

use crate::{log_sink::GrpcSink, sink::LogSink, DataLoaderSettings, RecordingStreamError};

/// Sends a local file using all [`re_data_loader::DataLoader`]s available.
///
/// A single `path` might be handled by more than one loader.
///
/// Unlike [`super::RecordingStream::log_file`] this will never append to an existing recording
/// unless an rrd file with an already loaded recording id is provided.
///
/// This method blocks until either at least one [`re_data_loader::DataLoader`] starts
/// streaming data in or all of them fail.
///
/// See <https://www.rerun.io/docs/reference/data-loaders/overview> for more information.
pub fn send_file_to_sink(
    filepath: impl AsRef<std::path::Path>,
    sink: &dyn LogSink,
) -> Result<(), DataLoaderError> {
    let filepath = filepath.as_ref();

    let (tx, rx) = re_smart_channel::smart_channel(
        re_smart_channel::SmartMessageSource::Sdk,
        re_smart_channel::SmartChannelSource::File(filepath.into()),
    );

    // TODO(andreas): Expose some of these?
    let settings = DataLoaderSettings {
        application_id: None,
        opened_application_id: None,
        store_id: StoreId::random(re_log_types::StoreKind::Recording),
        opened_store_id: None,
        force_store_info: false,
        entity_path_prefix: None,
        timepoint: None,
    };

    // TODO(andreas): Add contents version.
    re_data_loader::load_from_path(&settings, re_log_types::FileSource::Sdk, filepath, &tx)?;
    drop(tx);

    // We can safely ignore the error on `recv()` as we're in complete control of both ends of
    // the channel.
    while let Some(msg) = rx.recv().ok().and_then(|msg| msg.into_data()) {
        sink.send(msg);
    }

    Ok(())
}

/// Sends a file to a Rerun server via gRPC.
///
/// Unlike [`super::RecordingStream::log_file`] this will never append to an existing recording
/// unless an rrd file with an already loaded recording id is provided.
/// This is a convenience wrapper for [`send_file_to_sink`].
///
/// See also [`send_file_grpc_opts`] if you wish to configure the connection.
pub fn send_file_grpc(filepath: impl AsRef<std::path::Path>) -> Result<(), RecordingStreamError> {
    send_file_grpc_opts(
        filepath,
        format!(
            "rerun+http://127.0.0.1:{}/proxy",
            re_grpc_server::DEFAULT_SERVER_PORT
        ),
        crate::default_flush_timeout(),
    )
}

/// Sends a file to a Rerun server via gRPC.
///
/// Unlike [`super::RecordingStream::log_file`] this will never append to an existing recording
/// unless an rrd file with an already loaded recording id is provided.
/// This is a convenience wrapper for [`send_file_to_sink`].
///
/// `flush_timeout` is the minimum time the [`GrpcSink`][`crate::log_sink::GrpcSink`] will
/// wait during a flush before potentially dropping data. Note: Passing `None` here can cause a
/// call to `flush` to block indefinitely if a connection cannot be established.
pub fn send_file_grpc_opts(
    filepath: impl AsRef<std::path::Path>,
    url: impl Into<String>,
    flush_timeout: Option<std::time::Duration>,
) -> Result<(), RecordingStreamError> {
    let url: String = url.into();
    let re_uri::RedapUri::Proxy(endpoint) = url.as_str().parse()? else {
        return Err(RecordingStreamError::NotAProxyEndpoint);
    };

    let sink = GrpcSink::new(endpoint, flush_timeout);
    send_file_to_sink(filepath, &sink)?;

    Ok(())
}
