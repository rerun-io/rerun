//! Capture log messages and send them to some receiver over a channel.

use std::sync::LazyLock;

pub use crossbeam::channel::{Receiver, Sender};

use crate::event_visitor::FieldValue;

/// A tracing layer that pipes log messages to registered channels.
#[derive(Default)]
pub struct ChannelLayer {
    channels: parking_lot::RwLock<Vec<Channel>>,
}

#[derive(Clone, Debug)]
pub struct LogMsg {
    /// The verbosity level.
    pub level: tracing::Level,

    /// The module, starting with the crate name.
    pub target: String,

    /// The contents of the log message.
    pub message: String,

    /// Custom key-value fields captured from structured logging,
    /// e.g. `re_log::info!(?name, id = user_id(), "A person logged out")`.
    pub fields: Vec<(&'static str, FieldValue)>,
}

struct Channel {
    filter: tracing_subscriber::filter::LevelFilter,
    tx: Sender<LogMsg>,
}

pub fn channel_logger() -> &'static ChannelLayer {
    static CHANNEL_LAYER: LazyLock<ChannelLayer> = LazyLock::new(ChannelLayer::default);
    &CHANNEL_LAYER
}

/// Register a new receiver for log messages.
pub fn add_log_msg_receiver(filter: tracing_subscriber::filter::LevelFilter) -> Receiver<LogMsg> {
    // can't block on web, so we cannot apply backpressure
    #[cfg_attr(not(target_arch = "wasm32"), expect(clippy::disallowed_methods))]
    let (tx, rx) = crossbeam::channel::unbounded();
    channel_logger()
        .channels
        .write()
        .push(Channel { filter, tx });
    rx
}

impl<S> tracing_subscriber::Layer<S> for &'static ChannelLayer
where
    S: tracing::Subscriber,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let metadata = event.metadata();

        let mut channels = self.channels.write();
        if channels.is_empty() {
            return;
        }

        let mut visitor = crate::event_visitor::EventVisitor::default();
        event.record(&mut visitor);
        let (message, mut fields) = visitor.into_message_and_fields();
        let target = normalize_tracing_log_fields(&mut fields)
            .unwrap_or_else(|| metadata.target().to_owned());

        if !channels
            .iter()
            .any(|channel| crate::is_log_enabled(channel.filter, &target, metadata.level()))
        {
            return;
        }

        channels.retain(|channel| {
            if crate::is_log_enabled(channel.filter, &target, metadata.level()) {
                // Ok with a naked `send` here, because we use an unbounded channel,
                // so this can never block.
                #[cfg_attr(not(target_arch = "wasm32"), expect(clippy::disallowed_methods))]
                channel
                    .tx
                    .send(LogMsg {
                        level: *metadata.level(),
                        target: target.clone(),
                        message: message.clone(),
                        fields: fields.clone(),
                    })
                    .is_ok()
            } else {
                true
            }
        });
    }
}

fn normalize_tracing_log_fields(fields: &mut Vec<(&'static str, FieldValue)>) -> Option<String> {
    // For events from `log` the target is a metadata field `log.target` with a string value.
    let target_index = fields.iter().position(|(name, _)| *name == "log.target")?;
    let (_, FieldValue::String(target)) = fields.remove(target_index) else {
        return None;
    };

    // Remove the `log` only fields for `log.module_path`, `log.file`, and `log.line`.
    fields.retain(|(name, _)| !matches!(*name, "log.module_path" | "log.file" | "log.line"));

    Some(target)
}
