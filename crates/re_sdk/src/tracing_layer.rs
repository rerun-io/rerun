//! Loosely based on <https://github.com/rerun-io/remote-logging-experiment>.

#![allow(dead_code)] // TODO(emilk): send more of the tracing data to rerun

use std::sync::Arc;

use ahash::HashMap;
use parking_lot::Mutex;
use re_log_types::{component_types::TextEntry, EntityPath};

use crate::{sink::LogSink, MsgSenderError};

// ----------------------------------------------------------------------------

/// A place in the source code where we may be logging data from.
#[derive(Clone, Debug)]
struct Callsite {
    pub kind: CallsiteKind,
    pub name: &'static str,
    pub level: LogLevel,
    pub location: Location,
    /// Names of data that may be provided in later calls
    pub field_names: Vec<&'static str>,
}

/// Describes a source code location.
#[derive(Clone, Debug)]
struct Location {
    /// e.g. the name of the module/app that produced the log
    pub module: String,
    /// File name
    pub file: Option<String>,
    /// Line number
    pub line: Option<u32>,
}

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, PartialOrd, Ord)]
enum LogLevel {
    /// The "trace" level.
    ///
    /// Designates very low priority, often extremely verbose, information.
    Trace = 0,

    /// The "debug" level.
    ///
    /// Designates lower priority information.
    Debug = 1,

    /// The "info" level.
    ///
    /// Designates useful information.
    Info = 2,

    /// The "warn" level.
    ///
    /// Designates hazardous situations.
    Warn = 3,

    /// The "error" level.
    ///
    /// Designates very serious errors.
    Error = 4,
}

impl LogLevel {
    pub fn to_rerun_log_level_string(self) -> &'static str {
        match self {
            LogLevel::Trace => "TRACE",
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
enum CallsiteKind {
    Event,
    Span,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
struct CallsiteId(pub u64);

impl std::fmt::Display for CallsiteId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        format!("{:016x}", self.0).fmt(f)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
struct SpanId(pub u64);

impl std::fmt::Display for SpanId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        format!("{:016X}", self.0).fmt(f)
    }
}

// ----------------------------------------------------------------------------

/// Forwards [`tracing`] text logs to a Rerun [`LogSink`].
///
/// This implement [`tracing_subscriber::layer::Layer`].
pub struct RerunLayer {
    sink: Arc<dyn LogSink>,
    ent_path: re_log_types::EntityPath,
    callsites: Mutex<HashMap<CallsiteId, Callsite>>,
}

impl RerunLayer {
    /// Forward tracing log events to this Rerun [`EntityPath`].
    pub fn new(ent_path: impl Into<EntityPath>, sink: Arc<dyn LogSink>) -> Self {
        Self {
            sink,
            ent_path: ent_path.into(),
            callsites: Default::default(),
        }
    }
}

// ----------------------------------------------------------------------------

impl<S: tracing::Subscriber> tracing_subscriber::layer::Layer<S> for RerunLayer {
    fn on_layer(&mut self, _subscriber: &mut S) {
        // ignored for now
    }

    fn register_callsite(
        &self,
        metadata: &'static tracing::Metadata<'static>,
    ) -> tracing::subscriber::Interest {
        let kind = if metadata.is_event() {
            CallsiteKind::Event
        } else {
            CallsiteKind::Span
        };

        let level = if *metadata.level() == tracing::Level::ERROR {
            LogLevel::Error
        } else if *metadata.level() == tracing::Level::WARN {
            LogLevel::Warn
        } else if *metadata.level() == tracing::Level::INFO {
            LogLevel::Info
        } else if *metadata.level() == tracing::Level::DEBUG {
            LogLevel::Debug
        } else {
            LogLevel::Trace
        };

        let field_names = metadata.fields().iter().map(|field| field.name()).collect();

        let location = Location {
            module: metadata.target().to_owned(),
            file: metadata.file().map(|t| t.to_owned()),
            line: metadata.line(),
        };

        let id = to_callsite_id(&metadata.callsite());

        let rr_callsite = Callsite {
            kind,
            name: metadata.name(),
            level,
            location,
            field_names,
        };

        self.callsites.lock().insert(id, rr_callsite);

        tracing::subscriber::Interest::always()
    }

    fn enabled(
        &self,
        _metadata: &tracing::Metadata<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) -> bool {
        true
    }

    fn on_new_span(
        &self,
        _attrs: &tracing::span::Attributes<'_>,
        _id: &tracing::Id,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        // ignored for now
    }

    fn on_record(
        &self,
        _span: &tracing::Id,
        _values: &tracing::span::Record<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        // ignored for now
    }

    fn on_follows_from(
        &self,
        _span: &tracing::Id,
        _follows: &tracing::Id,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        // ignored for now
    }

    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        // let parent_span_id = event
        //     .parent()
        //     .or_else(|| ctx.current_span().id())
        //     .map(|id| id.into_u64());

        let callsite_id = to_callsite_id(&event.metadata().callsite());

        let level = {
            let callsites = self.callsites.lock();
            let callsite = callsites.get(&callsite_id);
            if let Some(callsite) = callsite {
                if callsite.level < LogLevel::Debug {
                    return; // too much spam frm wgpu etc
                }

                callsite.level
            } else {
                return; // Not sure why this happens, but it happens a lot
            }
            // callsite.map(|callsite| callsite.level.to_rerun_log_level_string().to_owned())
        };

        let mut kv_collector = KvCollector::default();
        event.record(&mut kv_collector);

        // TODO(emilk): we should log all the other key-value-pairs too.
        if let Some(message) = kv_collector.get("message") {
            let body = message.to_string();

            let text_entry = TextEntry {
                body,
                level: Some(level.to_rerun_log_level_string().to_owned()),
            };

            (|| -> Result<(), MsgSenderError> {
                crate::MsgSender::new(self.ent_path.clone())
                    .with_component(&[text_entry])?
                    .send(&self.sink)
            })()
            .ok();
        }
    }

    fn on_enter(&self, _id: &tracing::Id, _ctx: tracing_subscriber::layer::Context<'_, S>) {
        // ignored for now
    }

    fn on_exit(&self, _id: &tracing::Id, _ctx: tracing_subscriber::layer::Context<'_, S>) {
        // ignored for now
    }

    fn on_close(&self, _id: tracing::Id, _ctx: tracing_subscriber::layer::Context<'_, S>) {
        // ignored for now
    }

    fn on_id_change(
        &self,
        _old: &tracing::Id,
        _new: &tracing::Id,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        // ignored for now
    }
}

// ----------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum Value {
    String(String),
    I64(i64),
    U64(u64),
    F64(f64),
    Bool(bool),
    Debug(String),
    Error {
        description: String,
        details: String,
    },
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::String(v) | Value::Debug(v) => v.fmt(f),
            Value::I64(v) => v.fmt(f),
            Value::U64(v) => v.fmt(f),
            Value::F64(v) => v.fmt(f),
            Value::Bool(v) => v.fmt(f),
            Value::Error {
                description,
                details,
            } => write!(f, "{description}: {details}"),
        }
    }
}

#[derive(Default)]
struct KvCollector {
    pub values: Vec<(&'static str, Value)>,
}

impl KvCollector {
    fn get(&self, key: &str) -> Option<&Value> {
        self.values
            .iter()
            .find_map(|(k, v)| if *k == key { Some(v) } else { None })
    }
}

impl tracing::field::Visit for KvCollector {
    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        let value = Value::F64(value);
        self.values.push((field.name(), value));
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        let value = Value::I64(value);
        self.values.push((field.name(), value));
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        let value = Value::U64(value);
        self.values.push((field.name(), value));
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        let value = Value::Bool(value);
        self.values.push((field.name(), value));
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        let value = Value::String(value.to_owned());
        self.values.push((field.name(), value));
    }

    fn record_error(
        &mut self,
        field: &tracing::field::Field,
        value: &(dyn std::error::Error + 'static),
    ) {
        let value = Value::Error {
            description: value.to_string(),
            details: format!("{:#?}", value),
        };
        self.values.push((field.name(), value));
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        let value = Value::Debug(format!("{:#?}", value));
        self.values.push((field.name(), value));
    }
}

fn to_callsite_id(id: &tracing::callsite::Identifier) -> CallsiteId {
    CallsiteId(hash(id))
}

/// Hash the given value with a predictable hasher.
#[inline]
fn hash(value: impl std::hash::Hash) -> u64 {
    use std::hash::Hasher as _;
    // Don't use ahash::AHasher::default() since it uses a random number for seeding the hasher on every application start.
    let mut hasher =
        std::hash::BuildHasher::build_hasher(&ahash::RandomState::with_seeds(0, 1, 2, 3));
    value.hash(&mut hasher);
    hasher.finish()
}
