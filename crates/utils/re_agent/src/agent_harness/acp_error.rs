//! Readable text for the JSON-RPC errors that agents answer ACP requests with.

use agent_client_protocol::Error;
use re_error::StructuredError;
use serde_json::{Map, Value};

/// The messages that JSON-RPC gives its reserved error codes.
///
/// They name a class of error, not what went wrong, so they are dropped from the summary
/// whenever the error says anything more specific.
const GENERIC_MESSAGES: &[&str] = &[
    "Internal error",
    "Invalid params",
    "Invalid request",
    "Method not found",
    "Parse error",
];

/// Fields of an error's `data` object that hold its human-readable message, by priority.
const MESSAGE_FIELDS: &[&str] = &["message", "reason", "error"];

/// Describes an ACP error as [`StructuredError`] text.
///
/// Agents nest the actual message in `data`, sometimes several levels deep, and some append
/// a JSON object of metadata to the message itself.
/// The messages become the summary, and every other field becomes a detail.
pub fn describe_error(err: &Error) -> String {
    let mut parts = Parts::default();
    parts.add_text(&err.message);
    if let Some(data) = &err.data {
        parts.add_value(data.clone());
    }
    parts.into_error().to_string()
}

#[derive(Default)]
struct Parts {
    summary: Vec<String>,
    details: Vec<String>,
}

impl Parts {
    fn add_value(&mut self, value: Value) {
        match value {
            Value::Null => {}
            Value::String(text) => self.add_text(&text),
            Value::Object(fields) => self.add_fields(fields),
            other => self.details.push(other.to_string()),
        }
    }

    fn add_fields(&mut self, mut fields: Map<String, Value>) {
        let message_field = MESSAGE_FIELDS
            .iter()
            .find(|field| fields.get(**field).is_some_and(Value::is_string));
        if let Some(Value::String(message)) = message_field.and_then(|field| fields.remove(*field))
        {
            self.add_text(&message);
        }
        let inner = fields.remove("data");
        for (key, value) in fields {
            let value = match value {
                Value::Null => continue,
                Value::String(value) => value,
                value => value.to_string(),
            };
            self.details.push(format!("{key}: {value}"));
        }
        if let Some(inner) = inner {
            self.add_value(inner);
        }
    }

    /// Adds a message, splitting off a trailing JSON object such as `"Failed: {\"code\":1}"`.
    fn add_text(&mut self, text: &str) {
        let trailing_object = text.match_indices('{').find_map(|(start, _)| {
            let prefix = text[..start].trim_end();
            if !prefix.is_empty() && !prefix.ends_with(':') {
                return None;
            }
            match serde_json::from_str(&text[start..]) {
                Ok(Value::Object(fields)) => Some((prefix.trim_end_matches(':'), fields)),
                _ => None,
            }
        });
        match trailing_object {
            Some((prefix, fields)) => {
                self.add_summary(prefix);
                self.add_fields(fields);
            }
            None => self.add_summary(text),
        }
    }

    fn add_summary(&mut self, text: &str) {
        let text = text.trim();
        if !text.is_empty() && !self.summary.iter().any(|seen| seen == text) {
            self.summary.push(text.to_owned());
        }
    }

    fn into_error(self) -> StructuredError {
        let Self { summary, details } = self;
        let summary = summary.join(": ");
        let specific = GENERIC_MESSAGES
            .iter()
            .find_map(|generic| summary.strip_prefix(generic)?.strip_prefix(": "));
        StructuredError::from_summary(specific.unwrap_or(&summary)).with_details(details)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn error(message: &str, data: Option<Value>) -> Error {
        let mut err = Error::internal_error();
        err.message = message.to_owned();
        err.data = data;
        err
    }

    #[test]
    fn trailing_json_becomes_details() {
        let err = error(
            "Internal error: tools.12.custom.input_schema: input_schema does not support oneOf, allOf, or anyOf at the top level: {\"service\":\"session\",\"errorName\":\"APIError\"}",
            None,
        );
        assert_eq!(
            describe_error(&err),
            "tools.12.custom.input_schema: input_schema does not support oneOf, allOf, or anyOf at the top level\n- service: session\n- errorName: APIError"
        );
    }

    #[test]
    fn nested_data_message_replaces_generic_message() {
        let err = error(
            "Internal error",
            Some(json!({
                "service": "session",
                "data": { "message": "Rate limited", "retryAfter": 3 },
            })),
        );
        assert_eq!(
            describe_error(&err),
            "Rate limited\n- service: session\n- retryAfter: 3"
        );
    }

    #[test]
    fn generic_message_alone_is_kept() {
        assert_eq!(
            describe_error(&error("Internal error", None)),
            "Internal error"
        );
        assert_eq!(
            describe_error(&error("Internal error", Some(json!({ "code": 7 })))),
            "Internal error\n- code: 7"
        );
    }

    #[test]
    fn specific_message_keeps_data_string() {
        let err = error("Authentication required", Some(json!("Run `claude login`")));
        assert_eq!(
            describe_error(&err),
            "Authentication required: Run `claude login`"
        );
    }

    #[test]
    fn braces_inside_text_stay_in_summary() {
        let err = error("Unknown tool {name}", Some(json!("Unknown tool {name}")));
        assert_eq!(describe_error(&err), "Unknown tool {name}");
    }
}
