//! Helpers for extracting a plain-text message from structured [`tracing`] events.

/// A value captured by structured logging.
#[derive(Clone, Debug)]
pub enum FieldValue {
    Bool(bool),
    I64(i64),
    U64(u64),
    String(String),

    /// [`std::fmt::Debug`]-formatting of some value
    Debug(String),

    /// [`std::fmt::Display`]-formatting of an [`std::error::Error`]
    Error(String),
}

impl std::fmt::Display for FieldValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bool(value) => write!(f, "{value}"),
            Self::I64(value) => write!(f, "{value}"),
            Self::U64(value) => write!(f, "{value}"),
            Self::String(value) | Self::Debug(value) | Self::Error(value) => write!(f, "{value}"),
        }
    }
}

/// A visitor that formats a [`tracing::Event`] like the old `log::Record` message.
#[derive(Default)]
pub struct EventVisitor {
    message: Option<String>,
    fields: Vec<(&'static str, FieldValue)>,
}

impl EventVisitor {
    /// Returns the message and the structured key-value fields separately.
    pub fn into_message_and_fields(self) -> (String, Vec<(&'static str, FieldValue)>) {
        let Self { message, fields } = self;
        let message = message.unwrap_or_default();
        (message, fields)
    }

    /// Returns the formatted message followed by any structured fields on a single line.
    #[cfg(not(target_arch = "wasm32"))] // Only used by the non-wasm panic-on-warn path.
    pub fn format_as_string(self) -> String {
        let (message, fields) = self.into_message_and_fields();
        let fields = fields
            .iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect::<Vec<_>>();
        match (message.is_empty(), fields.is_empty()) {
            (_, true) => message,
            (true, false) => fields.join(" "),
            (false, false) => format!("{message} {}", fields.join(" ")),
        }
    }
}

impl tracing::field::Visit for EventVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = Some(format!("{value:?}"));
        } else {
            self.fields
                .push((field.name(), FieldValue::Debug(format!("{value:?}"))));
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.message = Some(value.to_owned());
        } else {
            self.fields
                .push((field.name(), FieldValue::String(value.to_owned())));
        }
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        if field.name() == "message" {
            self.message = Some(value.to_string());
        } else {
            self.fields.push((field.name(), FieldValue::I64(value)));
        }
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        if field.name() == "message" {
            self.message = Some(value.to_string());
        } else {
            self.fields.push((field.name(), FieldValue::U64(value)));
        }
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        if field.name() == "message" {
            self.message = Some(value.to_string());
        } else {
            self.fields.push((field.name(), FieldValue::Bool(value)));
        }
    }

    fn record_error(
        &mut self,
        field: &tracing::field::Field,
        value: &(dyn std::error::Error + 'static),
    ) {
        if field.name() == "message" {
            self.message = Some(value.to_string());
        } else {
            self.fields
                .push((field.name(), FieldValue::Error(value.to_string())));
        }
    }
}
