//! The MCP tool surface for `ViewerControlService`, derived from the protobuf descriptors.
//!
//! Every operation of the service is a variant of `ViewerControlRequest.kind`, so the tool list,
//! the input schemas, and the descriptions an agent reads can all come from the descriptor set
//! rather than being restated in Rust. Adding an operation to `viewer_control.proto` adds a tool.
//!
//! `pixi run codegen-protos` keeps `SourceCodeInfo` in the descriptor set, which is where the
//! descriptions come from: the `.proto` comments are the agent-facing documentation.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, LazyLock};

use prost::Message as _;
use prost_reflect::{
    DeserializeOptions, DynamicMessage, FieldDescriptor, Kind, MessageDescriptor, OneofDescriptor,
};
use rmcp::model::Tool;
use serde_json::{Map, Value, json};

use re_protos::viewer_control::v1alpha1::{ViewerControlRequest, ViewerControlResponse};

/// Fully-qualified name of the request envelope whose `oneof` defines the operations.
///
/// Taken from the generated type rather than spelled out, so moving the proto to another package
/// cannot silently empty the tool list.
fn request_envelope() -> String {
    <ViewerControlRequest as prost::Name>::full_name()
}

/// Name of the `oneof` inside the request envelope.
const OPERATION_ONEOF: &str = "kind";

/// Operations that exist on the wire but are not useful as agent-facing tools.
///
/// `egui_inspect` carries a MessagePack blob that the `egui_mcp` tools already wrap; handing an
/// agent the raw bytes would be worse than the tools it backs.
const HIDDEN_OPERATIONS: &[&str] = &["egui_inspect"];

/// Appended to every generated description, since the tools share one connection.
const REQUIRES_CONNECT: &str = "\n\nRequires `rerun_connect`.";

/// How deep to expand nested message fields before emitting an opaque object.
///
/// Guards against a message that transitively contains itself, which would otherwise recurse
/// forever while building the schema.
const MAX_SCHEMA_DEPTH: usize = 8;

/// The `oneof` that names the service's operations.
///
/// The descriptor set is compiled in alongside the generated types, so a missing envelope or
/// `oneof` means the two were built from different `.proto` files.
fn operation_oneof() -> OneofDescriptor {
    let envelope = re_protos::json::descriptor_pool()
        .get_message_by_name(&request_envelope())
        .unwrap_or_else(|| {
            panic!(
                "The built-in descriptor set has no `{}`",
                request_envelope()
            )
        });
    envelope
        .oneofs()
        .find(|oneof| oneof.name() == OPERATION_ONEOF)
        .unwrap_or_else(|| panic!("`{}` has no `{OPERATION_ONEOF}` oneof", request_envelope()))
}

/// Leading `.proto` comments, keyed by the descriptor path they document.
///
/// `prost_reflect` exposes each descriptor's `SourceCodeInfo` path but not the comment itself, so
/// we index the locations once and look them up by path.
static COMMENTS: LazyLock<HashMap<(String, Vec<i32>), String>> = LazyLock::new(|| {
    let mut comments = HashMap::default();
    for file in re_protos::json::descriptor_pool().files() {
        let file_name = file.name().to_owned();
        let Some(info) = file.file_descriptor_proto().source_code_info.as_ref() else {
            continue;
        };
        for location in &info.location {
            let Some(leading) = location.leading_comments.as_ref() else {
                continue;
            };
            let text = clean_comment(leading);
            if !text.is_empty() {
                comments.insert((file_name.clone(), location.path.clone()), text);
            }
        }
    }
    comments
});

/// Turn a raw leading comment into prose: `protoc` keeps the `//` stripped but the leading space
/// and the line breaks intact.
fn clean_comment(raw: &str) -> String {
    raw.lines()
        .map(|line| line.strip_prefix(' ').unwrap_or(line))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_owned()
}

/// The documentation attached to a field in its `.proto` file.
fn field_comment(field: &FieldDescriptor) -> Option<&'static str> {
    let file = field.parent_file().name().to_owned();
    COMMENTS
        .get(&(file, field.path().to_vec()))
        .map(String::as_str)
}

/// The documentation attached to a message in its `.proto` file.
fn message_comment(message: &MessageDescriptor) -> Option<&'static str> {
    let file = message.parent_file().name().to_owned();
    COMMENTS
        .get(&(file, message.path().to_vec()))
        .map(String::as_str)
}

/// Whether a caller must set this field.
///
/// Proto3 message fields always track presence, so presence alone says nothing. The `optional`
/// keyword does: it is the only way a `.proto` author marks a field as omittable, and a field in
/// a `oneof` or a repeated field is omittable by construction.
fn is_required(field: &FieldDescriptor) -> bool {
    !field.field_descriptor_proto().proto3_optional()
        && !field.is_list()
        && !field.is_map()
        && field.containing_oneof().is_none()
}

/// Whether this field is one arm of a `oneof` the author actually wrote.
///
/// Proto3 `optional` is implemented as a synthetic one-arm `oneof`, which carries none of the
/// mutual exclusion a written `oneof` does.
fn in_real_oneof(field: &FieldDescriptor) -> bool {
    field
        .containing_oneof()
        .is_some_and(|oneof| !oneof.is_synthetic())
}

/// The `oneof`s an author wrote in this message, skipping the synthetic proto3 `optional` ones.
fn real_oneofs(message: &MessageDescriptor) -> impl Iterator<Item = OneofDescriptor> + '_ {
    message.oneofs().filter(|oneof| !oneof.is_synthetic())
}

/// One JSON Schema branch per arm of a `oneof`.
///
/// `oneOf` demands that exactly one branch match, so listing each arm as its own `required` is
/// enough: no arm set matches nothing, and two arms set match two branches.
fn oneof_branches(oneof: &OneofDescriptor) -> Value {
    let branches: Vec<Value> = oneof
        .fields()
        .map(|field| json!({ "required": [field.name()] }))
        .collect();
    json!(branches)
}

/// JSON Schema for one field's value, ignoring whether it repeats.
fn scalar_schema(field: &FieldDescriptor, visiting: &mut HashSet<String>, depth: usize) -> Value {
    match field.kind() {
        Kind::Double | Kind::Float => json!({ "type": "number" }),
        // Unsigned on the wire, so say so: the deserializer rejects a negative anyway, and a
        // schema that allows one just invites a failed call.
        Kind::Uint32 | Kind::Uint64 | Kind::Fixed32 | Kind::Fixed64 => {
            json!({ "type": "integer", "minimum": 0 })
        }
        Kind::Int32
        | Kind::Int64
        | Kind::Sint32
        | Kind::Sint64
        | Kind::Sfixed32
        | Kind::Sfixed64 => json!({ "type": "integer" }),
        Kind::Bool if in_real_oneof(field) => json!({ "type": "boolean", "enum": [true] }),
        Kind::Bool => json!({ "type": "boolean" }),
        Kind::String => json!({ "type": "string" }),
        // Canonical protobuf JSON carries `bytes` as base64.
        Kind::Bytes => json!({ "type": "string", "contentEncoding": "base64" }),
        Kind::Enum(descriptor) => {
            let values: Vec<String> = descriptor
                .values()
                .map(|value| value.name().to_owned())
                .collect();
            json!({ "type": "string", "enum": values })
        }
        Kind::Message(descriptor) => message_schema(&descriptor, visiting, depth),
    }
}

/// JSON Schema for a message, expanding its fields.
fn message_schema(
    message: &MessageDescriptor,
    visiting: &mut HashSet<String>,
    depth: usize,
) -> Value {
    let name = message.full_name().to_owned();
    if depth >= MAX_SCHEMA_DEPTH || !visiting.insert(name.clone()) {
        // Self-referential or too deep to be worth spelling out.
        return json!({ "type": "object" });
    }

    let mut properties = Map::new();
    let mut required = Vec::new();
    for field in message.fields() {
        let mut schema = scalar_schema(&field, visiting, depth + 1);
        if field.is_list() {
            schema = json!({ "type": "array", "items": schema });
        }
        if let Some(comment) = field_comment(&field)
            && let Some(object) = schema.as_object_mut()
        {
            object.insert("description".to_owned(), json!(comment));
        }
        if is_required(&field) {
            required.push(field.name().to_owned());
        }
        properties.insert(field.name().to_owned(), schema);
    }

    visiting.remove(&name);

    let mut schema = json!({
        "type": "object",
        "properties": properties,
    });
    if !required.is_empty()
        && let Some(object) = schema.as_object_mut()
    {
        object.insert("required".to_owned(), json!(required));
    }
    // A `oneof` is an enum: exactly one arm. `oneOf` says so to a client that validates, and the
    // sentence says so to one that only reads.
    let oneofs: Vec<OneofDescriptor> = real_oneofs(message).collect();
    let mut branches: Vec<Value> = oneofs.iter().map(oneof_branches).collect();
    if let Some(object) = schema.as_object_mut() {
        match branches.len() {
            0 => {}
            // One `oneof` fits in the object itself; several would collide on the `oneOf` key.
            1 => {
                object.insert("oneOf".to_owned(), branches.swap_remove(0));
            }
            _ => {
                let all_of: Vec<Value> = branches
                    .into_iter()
                    .map(|branch| json!({ "oneOf": branch }))
                    .collect();
                object.insert("allOf".to_owned(), json!(all_of));
            }
        }
    }
    let exclusive: Vec<String> = oneofs
        .iter()
        .map(|oneof| {
            let arms: Vec<String> = oneof
                .fields()
                .map(|field| field.name().to_owned())
                .collect();
            format!("Exactly one of `{}` must be set.", arms.join("`, `"))
        })
        .collect();

    let description = match (message_comment(message), exclusive.is_empty()) {
        (None, true) => None,
        (None, false) => Some(exclusive.join(" ")),
        (Some(comment), true) => Some(comment.to_owned()),
        (Some(comment), false) => Some(format!("{comment}\n\n{}", exclusive.join(" "))),
    };
    if let Some(description) = description
        && let Some(object) = schema.as_object_mut()
    {
        object.insert("description".to_owned(), json!(description));
    }
    schema
}

/// The tools this service offers, one per operation, in `.proto` order.
pub fn tools() -> Vec<Tool> {
    operation_oneof()
        .fields()
        .filter(|field| !HIDDEN_OPERATIONS.contains(&field.name()))
        .map(|field| {
            let Kind::Message(request) = field.kind() else {
                // Every operation is a message; a scalar here would be a `.proto` mistake.
                re_log::debug_assert!(
                    false,
                    "operation `{}` is not a message, so it cannot carry a request",
                    field.name()
                );
                return Tool::new(
                    Cow::Owned(field.name().to_owned()),
                    Cow::Borrowed("Unsupported operation"),
                    Arc::new(Map::new()),
                );
            };

            let description = field_comment(&field).map_or_else(
                || format!("The `{}` viewer-control operation.", field.name()),
                |comment| format!("{comment}{REQUIRES_CONNECT}"),
            );

            let mut visiting = HashSet::default();
            let schema = message_schema(&request, &mut visiting, 0);
            let schema = schema.as_object().cloned().unwrap_or_default();

            Tool::new(
                Cow::Owned(field.name().to_owned()),
                Cow::Owned(description),
                Arc::new(schema),
            )
        })
        .collect()
}

/// Whether `name` is one of the generated operations.
pub fn is_operation(name: &str) -> bool {
    operation_oneof()
        .fields()
        .any(|field| field.name() == name && !HIDDEN_OPERATIONS.contains(&field.name()))
}

/// Build the request envelope for `name` from the JSON arguments an MCP client sent.
///
/// The arguments are parsed against the operation's own descriptor, so the `.proto` is what
/// validates them, and the result is re-encoded into the generated type the gRPC client wants.
pub fn build_request(
    name: &str,
    arguments: Option<Map<String, Value>>,
) -> Result<ViewerControlRequest, String> {
    let field = operation_oneof()
        .fields()
        .find(|field| field.name() == name)
        .ok_or_else(|| format!("unknown operation `{name}`"))?;
    let Kind::Message(request_descriptor) = field.kind() else {
        return Err(format!("operation `{name}` has no request message"));
    };

    let arguments = Value::Object(arguments.unwrap_or_default());

    // Protobuf has no required fields, so the `required` and `oneOf` the schema advertises would
    // otherwise go unchecked: an omitted `time` would become 0 and move the cursor to the start
    // of the recording rather than failing.
    validate(&request_descriptor, &arguments, "", 0)
        .map_err(|err| format!("invalid arguments for `{name}`: {err}"))?;

    // A field we do not recognize is a typo, or a call written against an older surface. Dropping
    // it silently would turn `{"target": "all"}` into an empty request, which closes the current
    // recording rather than every one of them.
    let options = DeserializeOptions::new().deny_unknown_fields(true);
    let request =
        DynamicMessage::deserialize_with_options(request_descriptor, &arguments, &options)
            .map_err(|err| format!("invalid arguments for `{name}`: {err}"))?;

    let envelope_descriptor = re_protos::json::descriptor_pool()
        .get_message_by_name(&request_envelope())
        .ok_or_else(|| format!("missing descriptor for `{}`", request_envelope()))?;
    let mut envelope = DynamicMessage::new(envelope_descriptor);
    envelope.set_field(&field, prost_reflect::Value::Message(request));

    ViewerControlRequest::decode(envelope.encode_to_vec().as_slice())
        .map_err(|err| format!("failed to build the request envelope for `{name}`: {err}"))
}

/// Check the arguments against what the generated schema promises: every `required` field is
/// present, and every `oneof` has exactly one arm set.
///
/// An explicit `null` counts as absent: canonical protobuf JSON reads it as "clear this field",
/// which is the very omission the check is here to catch.
///
/// Recurses into set message fields, so a present-but-empty submessage is rejected too. Type
/// errors are left to the deserializer, which reports them better.
fn validate(
    message: &MessageDescriptor,
    value: &Value,
    path: &str,
    depth: usize,
) -> Result<(), String> {
    if depth >= MAX_SCHEMA_DEPTH {
        return Ok(());
    }
    let Value::Object(fields) = value else {
        return Ok(());
    };

    let present = |field: &FieldDescriptor| fields.get(field.name()).filter(|v| !v.is_null());

    for oneof in real_oneofs(message) {
        let set: Vec<String> = oneof
            .fields()
            .filter(|field| present(field).is_some())
            .map(|field| field.name().to_owned())
            .collect();
        let arms: Vec<String> = oneof
            .fields()
            .map(|field| field.name().to_owned())
            .collect();
        if set.len() != 1 {
            return Err(format!(
                "`{path}{}` takes exactly one of `{}`, but {}",
                oneof.name(),
                arms.join("`, `"),
                if set.is_empty() {
                    "none were set".to_owned()
                } else {
                    format!("`{}` were set", set.join("`, `"))
                }
            ));
        }
    }

    for field in message.fields() {
        let child = present(&field);
        if is_required(&field) && child.is_none() {
            return Err(format!("missing required field `{path}{}`", field.name()));
        }
        if let (Kind::Message(nested), Some(child)) = (field.kind(), child)
            && !field.is_list()
            && !field.is_map()
        {
            validate(
                &nested,
                child,
                &format!("{path}{}.", field.name()),
                depth + 1,
            )?;
        }
    }
    Ok(())
}

/// Render a response envelope as the JSON an agent sees, unwrapping the operation's variant.
///
/// The envelope adds one level of nesting that only repeats the tool's own name, so we drop it.
pub fn response_json(operation: &str, response: &ViewerControlResponse) -> Result<Value, String> {
    // The service promises the response variant matches the request. Unwrapping whatever arrived
    // would let a mismatched peer pass another operation's payload off as this one's result.
    let actual = response.kind_name();
    if actual != operation {
        return Err(format!("expected a `{operation}` response, got `{actual}`"));
    }

    let envelope = re_protos::json::to_json_value(response).map_err(|err| format!("{err}"))?;

    let Value::Object(mut fields) = envelope else {
        return Ok(envelope);
    };
    match fields.len() {
        // An operation whose response carries nothing at all.
        0 => Ok(json!({})),
        1 => Ok(fields
            .values_mut()
            .next()
            .map(Value::take)
            .unwrap_or_else(|| json!({}))),
        _ => Ok(Value::Object(fields)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offers_one_tool_per_visible_operation() {
        let names: Vec<String> = tools().iter().map(|tool| tool.name.to_string()).collect();

        assert!(names.contains(&"set_time_cursor".to_owned()));
        assert!(names.contains(&"get_viewer_state".to_owned()));
        assert!(
            !names.contains(&"egui_inspect".to_owned()),
            "the egui transport must not be offered as a tool"
        );
    }

    #[test]
    fn carries_the_proto_comments_into_the_descriptions() {
        let tools = tools();
        let open_url = tools
            .iter()
            .find(|tool| tool.name == "open_url")
            .expect("open_url is an operation");

        let description = open_url
            .description
            .as_ref()
            .expect("generated description");
        assert!(description.contains("rerun://"), "{description}");
        assert!(
            description.ends_with("Requires `rerun_connect`."),
            "{description}"
        );

        let url = &open_url.input_schema["properties"]["url"];
        assert_eq!(url["type"], "string");
        assert!(
            url["description"]
                .as_str()
                .is_some_and(|d| d.contains("URL"))
        );
    }

    #[test]
    fn marks_only_the_fields_the_proto_leaves_unoptional_as_required() {
        let tools = tools();
        let set_time = tools
            .iter()
            .find(|tool| tool.name == "set_time_cursor")
            .expect("set_time_cursor is an operation");

        let required: Vec<&str> = set_time.input_schema["required"]
            .as_array()
            .expect("set_time_cursor requires a time")
            .iter()
            .filter_map(Value::as_str)
            .collect();

        assert_eq!(required, ["time"]);
    }

    #[test]
    fn spells_enum_fields_as_their_names() {
        let response = re_protos::json::descriptor_pool()
            .get_message_by_name(
                <re_protos::viewer_control::v1alpha1::SetTimeCursorResponse as prost::Name>::full_name().as_str(),
            )
            .expect("a generated type has a descriptor");

        let schema = message_schema(&response, &mut HashSet::default(), 0);

        let time_type = &schema["properties"]["time_type"];
        assert_eq!(time_type["type"], "string");
        assert!(
            time_type["enum"]
                .as_array()
                .is_some_and(|values| values.iter().any(|v| v == "TIME_TYPE_SEQUENCE")),
            "{time_type}"
        );
    }

    #[test]
    fn carries_store_ids_as_documented_strings() {
        let tools = tools();
        let close = tools
            .iter()
            .find(|tool| tool.name == "close_recordings")
            .expect("close_recordings is an operation");

        let items =
            &close.input_schema["properties"]["store_ids"]["properties"]["store_ids"]["items"];
        assert_eq!(items["type"], "string");

        let description = close.input_schema["properties"]["store_ids"]["properties"]["store_ids"]
            ["description"]
            .as_str()
            .expect("the format an agent has to produce");
        assert!(description.contains("{kind}:"), "{description}");
    }

    #[test]
    fn builds_a_request_envelope_from_json_arguments() {
        let arguments = serde_json::json!({ "url": "rerun://example" })
            .as_object()
            .cloned()
            .expect("an object");

        let request = build_request("open_url", Some(arguments)).expect("valid arguments");

        match request.kind {
            Some(re_protos::viewer_control::v1alpha1::viewer_control_request::Kind::OpenUrl(
                open,
            )) => {
                assert_eq!(open.url, "rerun://example");
            }
            other => panic!("expected an open_url request, got {other:?}"),
        }
    }

    /// Build a request from a JSON literal, for the validation tests.
    fn build(name: &str, arguments: &Value) -> Result<ViewerControlRequest, String> {
        build_request(name, arguments.as_object().cloned())
    }

    #[test]
    fn rejects_a_missing_required_field() {
        let err = build("set_time_cursor", &json!({}))
            .expect_err("`time` is required, and defaulting it would seek to the start");
        assert!(err.contains("time"), "{err}");

        let err = build("set_time_cursor", &json!({ "time": {} }))
            .expect_err("an empty `time` is as wrong as an absent one");
        assert!(err.contains("time.time"), "{err}");

        let err = build("set_time_cursor", &json!({ "time": null }))
            .expect_err("an explicit null clears the field, so it is as wrong as an absent one");
        assert!(err.contains("time"), "{err}");
    }

    #[test]
    fn rejects_an_unknown_field() {
        // The shape of a call written against the older, hand-written surface.
        let err = build("close_recordings", &json!({ "target": "all" }))
            .expect_err("silently dropping this would close the current recording instead");
        assert!(err.contains("target"), "{err}");
    }

    #[test]
    fn accepts_a_complete_request() {
        let request = build(
            "set_time_cursor",
            &json!({ "timeline": { "name": "log_time" }, "time": { "time": 7 } }),
        )
        .expect("a complete request");

        match request.kind {
            Some(
                re_protos::viewer_control::v1alpha1::viewer_control_request::Kind::SetTimeCursor(
                    set_time,
                ),
            ) => {
                assert_eq!(set_time.time.map(|t| t.time), Some(7));
                assert_eq!(set_time.play, None, "`play` is omittable");
            }
            other => panic!("expected a set_time_cursor request, got {other:?}"),
        }
    }

    #[test]
    fn rejects_a_response_for_a_different_operation() {
        let response =
            ViewerControlResponse::from(re_protos::viewer_control::v1alpha1::OpenUrlResponse {});

        let err = response_json("get_viewer_state", &response)
            .expect_err("another operation's payload must not pass as this one's result");
        assert!(err.contains("open_url"), "{err}");

        response_json("open_url", &response).expect("the matching operation");
    }

    #[test]
    fn constrains_oneof_selectors_to_true() {
        let tools = tools();
        let close = tools
            .iter()
            .find(|tool| tool.name == "close_recordings")
            .expect("close_recordings is an operation");

        let all = &close.input_schema["properties"]["all"];
        assert_eq!(all["enum"], json!([true]), "a selector is not a flag");

        let description = close.input_schema["description"]
            .as_str()
            .expect("a description listing the exclusive arms");
        assert!(description.contains("Exactly one of"), "{description}");

        // A `oneof` is an enum, so the schema has to reject both no arm and two arms.
        assert_eq!(
            close.input_schema["oneOf"],
            json!([
                { "required": ["current"] },
                { "required": ["all"] },
                { "required": ["store_ids"] },
            ])
        );
    }

    #[test]
    fn rejects_a_request_that_sets_no_oneof_arm_or_several() {
        let err =
            build("close_recordings", &json!({})).expect_err("an unset target names no recording");
        assert!(err.contains("none were set"), "{err}");

        let err = build("close_recordings", &json!({ "current": true, "all": true }))
            .expect_err("two selectors contradict each other");
        assert!(err.contains("`current`, `all` were set"), "{err}");
    }

    #[test]
    fn rejects_arguments_of_the_wrong_type() {
        let arguments = serde_json::json!({ "url": 7 })
            .as_object()
            .cloned()
            .expect("an object");

        build_request("open_url", Some(arguments)).expect_err("a number is not a URL");
    }
}
