//! Canonical protobuf JSON for the generated types.
//!
//! Rendering a message by hand means restating every field, and the restatement drifts from the
//! `.proto` as fields come and go. Transcoding through the descriptor set keeps one source of
//! truth, and spells enums as their names rather than as the raw `i32` a `serde` derive on the
//! generated types would emit.

use std::sync::LazyLock;

use prost_reflect::{DescriptorPool, DeserializeOptions, DynamicMessage, SerializeOptions};

/// Encoded `FileDescriptorSet` covering every `rerun.*.v1alpha1` proto file and its imports.
///
/// Compiled from the protobuf sources by `build.rs`, including source locations and comments.
pub const FILE_DESCRIPTOR_SET: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/rerun.v1alpha1.fds.bin"));

/// Why a message could not be rendered as, or parsed from, JSON.
#[derive(Debug, thiserror::Error)]
pub enum JsonError {
    /// The message type is absent from [`FILE_DESCRIPTOR_SET`], which should not happen for a
    /// type generated from it.
    #[error("no descriptor for message type `{0}`")]
    UnknownMessage(String),

    /// The message did not round-trip through its own descriptor.
    #[error("failed to transcode `{type_name}`: {err}")]
    Transcode {
        /// Fully-qualified protobuf name of the message.
        type_name: String,

        /// The underlying decode failure.
        err: prost::DecodeError,
    },

    /// `serde_json` rejected the transcoded message.
    #[error("failed to serialize `{type_name}`: {err}")]
    Serialize {
        /// Fully-qualified protobuf name of the message.
        type_name: String,

        /// The underlying serialization failure.
        err: serde_json::Error,
    },

    /// The JSON does not describe a message of this type.
    #[error("invalid JSON for `{type_name}`{}: {err}", at_path(path))]
    Deserialize {
        /// Fully-qualified protobuf name of the message.
        type_name: String,

        /// Where in the JSON the parse failed, such as `set_time_cursor.store_id`.
        path: serde_path_to_error::Path,

        /// The underlying parse failure.
        err: serde_json::Error,
    },
}

/// " at `PATH`", or nothing if the path is the root (`.`) or not known (`?`).
fn at_path(path: &serde_path_to_error::Path) -> String {
    let is_known = path
        .iter()
        .any(|segment| !matches!(segment, serde_path_to_error::Segment::Unknown));
    if is_known {
        format!(" at `{path}`")
    } else {
        String::new()
    }
}

/// Every message type in the Rerun protocol, parsed once.
static DESCRIPTOR_POOL: LazyLock<DescriptorPool> = LazyLock::new(|| {
    DescriptorPool::decode(FILE_DESCRIPTOR_SET)
        .unwrap_or_else(|err| panic!("Failed to parse the built-in descriptor set: {err}"))
});

/// Every message type in the Rerun protocol, with the comments from the `.proto` files.
///
/// The descriptor set retains `SourceCodeInfo`, so callers can read the protocol's own
/// documentation rather than restate it.
pub fn descriptor_pool() -> &'static DescriptorPool {
    &DESCRIPTOR_POOL
}

/// Render a generated message as canonical protobuf JSON.
///
/// Field names stay as spelled in the `.proto` rather than being lower-camel-cased, 64-bit
/// integers stay numbers rather than becoming strings, and fields holding their default value are
/// omitted. The first two keep the output readable next to the `.proto`; the third keeps it short,
/// which matters where the result is fed to a language model.
pub fn to_json_value<M>(message: &M) -> Result<serde_json::Value, JsonError>
where
    M: prost::Message + prost::Name,
{
    let type_name = M::full_name();

    let descriptor = DESCRIPTOR_POOL
        .get_message_by_name(&type_name)
        .ok_or_else(|| JsonError::UnknownMessage(type_name.clone()))?;

    let dynamic =
        DynamicMessage::decode(descriptor, message.encode_to_vec().as_slice()).map_err(|err| {
            JsonError::Transcode {
                type_name: type_name.clone(),
                err,
            }
        })?;

    let options = SerializeOptions::new()
        .use_proto_field_name(true)
        .stringify_64_bit_integers(false)
        .skip_default_fields(true);

    dynamic
        .serialize_with_options(serde_json::value::Serializer, &options)
        .map_err(|err| JsonError::Serialize { type_name, err })
}

/// Parse a generated message from canonical protobuf JSON, as [`to_json_value`] renders it.
///
/// Unknown fields are an error rather than being dropped, so a misspelled field fails instead of
/// silently leaving its default.
pub fn from_json_str<M>(json: &str) -> Result<M, JsonError>
where
    M: prost::Message + prost::Name + Default,
{
    use prost::Message as _;

    let type_name = M::full_name();

    let descriptor = DESCRIPTOR_POOL
        .get_message_by_name(&type_name)
        .ok_or_else(|| JsonError::UnknownMessage(type_name.clone()))?;

    let mut deserializer = serde_json::Deserializer::from_str(json);
    let mut track = serde_path_to_error::Track::new();
    let options = DeserializeOptions::new().deny_unknown_fields(true);
    let dynamic = DynamicMessage::deserialize_with_options(
        descriptor,
        serde_path_to_error::Deserializer::new(&mut deserializer, &mut track),
        &options,
    )
    .and_then(|dynamic| deserializer.end().map(|()| dynamic))
    .map_err(|err| JsonError::Deserialize {
        type_name: type_name.clone(),
        path: track.path(),
        err,
    })?;

    M::decode(dynamic.encode_to_vec().as_slice())
        .map_err(|err| JsonError::Transcode { type_name, err })
}

#[cfg(test)]
mod tests {
    use crate::viewer_control::v1alpha1::{
        CloseResponse, SetTimeCursorResponse, ViewerControlRequest, ViewerLogEntry,
    };

    use super::*;

    #[test]
    fn parses_what_it_renders_and_rejects_unknown_fields() {
        let response = SetTimeCursorResponse {
            store_id: "Recording:app:rec".to_owned(),
            timeline: Some(crate::common::v1alpha1::Timeline {
                name: "frame".to_owned(),
            }),
            time_type: crate::common::v1alpha1::TimeType::Sequence as i32,
            time: Some(crate::common::v1alpha1::TimelineTime { time: 0 }),
        };
        let json = to_json_value(&response)
            .expect("a generated type must have a descriptor")
            .to_string();
        assert_eq!(
            from_json_str::<SetTimeCursorResponse>(&json).ok(),
            Some(response)
        );

        assert!(from_json_str::<SetTimeCursorResponse>(r#"{"stor_id": "x"}"#).is_err());
    }

    #[test]
    fn parses_the_requests_the_python_client_sends() {
        for json in [
            r#"{"set_time_cursor": {"store_id": "Recording:a:1", "timeline": {"name": "frame"}, "time": {"time": 0}, "play_state": "PLAY_STATE_FOLLOWING"}}"#,
            r#"{"open": {"url": "a.rrd", "navigate": false}}"#,
            r#"{"close": {"current": true}}"#,
            r#"{"close": {"urls": {"urls": ["a.rrd"]}}}"#,
            r#"{"close": {"store_ids": {"store_ids": []}}}"#,
            r#"{"get_logs": {"after_sequence": 4294967296}}"#,
            r#"{"save_screenshot": {"file_path": "a.png", "view_id": "x"}}"#,
        ] {
            assert!(
                from_json_str::<ViewerControlRequest>(json).is_ok(),
                "{json}"
            );
        }
    }

    #[test]
    fn errors_name_the_type_the_path_the_problem_and_where() {
        let error = |json: &str| {
            from_json_str::<ViewerControlRequest>(json)
                .err()
                .map(|err| err.to_string())
        };
        let prefix = "invalid JSON for `rerun.viewer_control.v1alpha1.ViewerControlRequest`";

        assert_eq!(
            error(r#"{"set_tim": {}}"#),
            Some(format!(
                "{prefix}: unrecognized field name 'set_tim' at line 1 column 10"
            ))
        );
        assert_eq!(
            error(r#"{"set_time_cursor": {"store_id": 5}}"#),
            Some(format!(
                "{prefix} at `set_time_cursor.store_id`: invalid type: integer `5`, expected a string at line 1 column 34"
            ))
        );
        assert_eq!(
            error(r#"{"set_time_cursor": {"play_state": "NOPE"}}"#),
            Some(format!(
                "{prefix} at `set_time_cursor.play_state`: unrecognized enum value 'NOPE' at line 1 column 41"
            ))
        );
        assert_eq!(
            error(r#"{"set_time_cursor": {"store": "x"}}"#),
            Some(format!(
                "{prefix} at `set_time_cursor`: unrecognized field name 'store' at line 1 column 28"
            ))
        );
        assert_eq!(
            error(r#"{"close": {"urls": {"urls": [1]}}}"#),
            Some(format!(
                "{prefix} at `close.urls.urls[0]`: invalid type: integer `1`, expected a string at line 1 column 30"
            ))
        );
        assert_eq!(
            error(r#"{"get_state": {}, "get_location": {}}"#),
            Some(format!(
                "{prefix}: multiple fields provided for oneof 'kind' at line 1 column 37"
            ))
        );
        assert_eq!(
            error(r#"{"get_location": {}"#),
            Some(format!(
                "{prefix}: EOF while parsing an object at line 1 column 19"
            ))
        );
    }

    #[test]
    fn descriptor_set_includes_imports_and_comments() {
        let pool = descriptor_pool();
        assert!(
            pool.get_message_by_name("google.protobuf.Timestamp")
                .is_some(),
            "imported well-known types must be included"
        );
        for file in pool.files() {
            let info = file
                .file_descriptor_proto()
                .source_code_info
                .as_ref()
                .expect("protocol documentation must be included");
            assert!(
                info.location.iter().any(|location| {
                    location
                        .leading_comments
                        .as_ref()
                        .is_some_and(|comment| !comment.trim().is_empty())
                }),
                "source comments must be included for {}",
                file.name()
            );
        }
    }

    #[test]
    fn spells_enums_by_name_rather_than_by_number() {
        let response = SetTimeCursorResponse {
            store_id: String::new(),
            timeline: Some(crate::common::v1alpha1::Timeline {
                name: "log_time".to_owned(),
            }),
            time_type: crate::common::v1alpha1::TimeType::TimestampNs as i32,
            time: Some(crate::common::v1alpha1::TimelineTime { time: 42 }),
        };

        let json = to_json_value(&response).expect("a generated type must have a descriptor");

        assert_eq!(json["time_type"], "TIME_TYPE_TIMESTAMP_NS");
        assert_eq!(json["timeline"]["name"], "log_time");
        assert_eq!(json["time"]["time"], 42);
    }

    #[test]
    fn omits_fields_holding_their_default() {
        let json = to_json_value(&CloseResponse { closed: Vec::new() })
            .expect("a generated type must have a descriptor");

        assert_eq!(json, serde_json::json!({}));
    }

    #[test]
    fn keeps_64_bit_integers_as_numbers() {
        let json = to_json_value(&ViewerLogEntry {
            sequence: u64::from(u32::MAX) + 1,
            level: "WARN".to_owned(),
            target: "re_viewer".to_owned(),
            message: "something".to_owned(),
        })
        .expect("a generated type must have a descriptor");

        assert_eq!(json["sequence"], 4_294_967_296_u64);
    }
}
