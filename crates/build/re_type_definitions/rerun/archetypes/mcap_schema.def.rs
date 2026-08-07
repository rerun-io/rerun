// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A schema definition that describes the structure of messages in an MCAP file.
///
/// Schemas define the data types and field structures used by messages in MCAP channels.
/// They provide the blueprint for interpreting message payloads, specifying field names,
/// types, and organization. Each schema is referenced by channels to indicate how their
/// messages should be decoded and understood.
///
/// See also [archetypes.McapChannel] for channels that reference these schemas,
/// [archetypes.McapMessage] for the messages that conform to these schemas, and the
/// [MCAP specification](https://mcap.dev/) for complete format details.
///
/// \example archetypes/mcap_schema_simple !api title="Simple MCAP schema"
#[rerun::rerun_type]
#[docs(category = "MCAP")]
#[rerun(state = "unstable")]
#[rerun(visualizer_none)]
#[rust(derive(PartialEq))]
pub struct McapSchema {
    /// Unique identifier for this schema within the MCAP file.
    ///
    /// Schema IDs must be unique within an MCAP file and are referenced by channels
    /// to specify their message structure. A single schema can be shared across multiple channels.
    #[rerun(no_ui_edit)]
    #[rerun(required)]
    pub id: rerun::components::SchemaId,

    /// Human-readable name identifying this schema.
    ///
    /// Schema names typically describe the message type or data structure
    /// (e.g., `"geometry_msgs/msg/Twist"`, `"sensor_msgs/msg/Image"`, `"MyCustomMessage"`).
    #[rerun(no_ui_edit)]
    #[rerun(required)]
    pub name: rerun::components::Text,

    /// The schema definition format used to describe the message structure.
    ///
    /// Common schema encodings include:
    /// * `protobuf` - [Protocol Buffers](https://mcap.dev/spec/registry#protobuf-1) schema definition
    /// * `ros1msg` - [ROS1](https://mcap.dev/spec/registry#ros1msg) message definition format
    /// * `ros2msg` - [ROS2](https://mcap.dev/spec/registry#ros2msg) message definition format
    /// * `jsonschema` - [JSON Schema](https://mcap.dev/spec/registry#jsonschema) specification
    /// * `flatbuffer` - [FlatBuffers](https://mcap.dev/spec/registry#flatbuffer) schema definition
    #[rerun(no_ui_edit)]
    #[rerun(required)]
    pub encoding: rerun::components::Text,

    /// The schema definition content as binary data.
    ///
    /// This contains the actual schema specification in the format indicated by the
    /// `encoding` field. For text-based schemas (like ROS message definitions or JSON Schema),
    /// this is typically UTF-8 encoded text. For binary schema formats, this contains
    /// the serialized schema data.
    #[rerun(no_ui_edit)]
    #[rerun(required)]
    pub data: rerun::components::Blob,
}
