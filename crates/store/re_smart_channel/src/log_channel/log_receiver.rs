use re_log_types::DataSourceMessage;

/// Something receiving a stream of [`DataSourceMessage`].
///
/// This is usually the viewer.
pub struct LogReceiver {
    pub(crate) rx: crate::Receiver<DataSourceMessage>,
}

impl LogReceiver {
    /// Call this on each sent message.
    ///
    /// Can be used to wake up the receiver thread.
    pub fn set_waker(&self, waker: impl Fn() + Send + Sync + 'static) {
        *self.rx.channel.waker.write() = Some(Box::new(waker));
    }
}

impl std::ops::Deref for LogReceiver {
    type Target = crate::Receiver<DataSourceMessage>;

    fn deref(&self) -> &Self::Target {
        &self.rx
    }
}

impl From<crate::Receiver<DataSourceMessage>> for LogReceiver {
    fn from(rx: crate::Receiver<DataSourceMessage>) -> Self {
        Self { rx }
    }
}
