mod log_receiver;
mod log_receiver_set;
mod log_sender;

pub use log_receiver::LogReceiver;
pub use log_receiver_set::LogReceiverSet;
pub use log_sender::LogSender;

/// Create a new communication channel for [`re_log_types::DataSourceMessage`].
pub fn log_channel(
    msg_src: crate::SmartMessageSource,
    channel_src: crate::SmartChannelSource,
) -> (LogSender, LogReceiver) {
    // TODO(emilk): add a back-channel to be used for controlling what data we load.
    let (tx, rx) = crate::smart_channel(msg_src, channel_src);
    (LogSender { tx }, LogReceiver { rx })
}
