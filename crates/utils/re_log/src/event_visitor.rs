//! Helpers for extracting a plain-text message from structured [`tracing`] events.

/// A visitor that formats a [`tracing::Event`] like the old `log::Record` message.
#[derive(Default)]
pub struct EventVisitor {
    message: Option<String>,
    fields: Vec<String>,
}

impl EventVisitor {
    /// Returns the formatted message followed by any structured fields.
    pub fn finish(self) -> String {
        let Self { message, fields } = self;
        match (message, fields.is_empty()) {
            (Some(message), true) => message,
            (Some(message), false) => format!("{message} {}", fields.join(" ")),
            (None, true) => String::new(),
            (None, false) => fields.join(" "),
        }
    }
}

impl tracing::field::Visit for EventVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = Some(format!("{value:?}"));
        } else {
            self.fields.push(format!("{}={value:?}", field.name()));
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.message = Some(value.to_owned());
        } else {
            self.fields.push(format!("{}={value:?}", field.name()));
        }
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.record_debug(field, &value);
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.record_debug(field, &value);
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.record_debug(field, &value);
    }

    fn record_error(
        &mut self,
        field: &tracing::field::Field,
        value: &(dyn std::error::Error + 'static),
    ) {
        if field.name() == "message" {
            self.message = Some(value.to_string());
        } else {
            self.fields.push(format!("{}={value}", field.name()));
        }
    }
}
