use re_log_types::DataSourceMessage;

/// Something producing a stream of [`DataSourceMessage`].
///
/// This could be a gRPC client, or an rrd file loader, for instance.
#[derive(Clone)]
pub struct LogSender {
    pub(crate) tx: crate::Sender<DataSourceMessage>,
}

impl std::ops::Deref for LogSender {
    type Target = crate::Sender<DataSourceMessage>;

    fn deref(&self) -> &Self::Target {
        &self.tx
    }
}

impl From<crate::Sender<DataSourceMessage>> for LogSender {
    fn from(tx: crate::Sender<DataSourceMessage>) -> Self {
        Self { tx }
    }
}
