// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A channel within an MCAP file that defines how messages are structured and encoded.
///
/// Channels in MCAP files group messages by topic and define their encoding format.
/// Each channel has a unique identifier and specifies the message schema and encoding used
/// for all messages published to that topic.
///
/// See also [archetypes.McapMessage] for individual messages within a channel,
/// [archetypes.McapSchema] for the data structure definitions, and the
/// [MCAP specification](https://mcap.dev/) for complete format details.
///
/// \example archetypes/mcap_channel_simple !api title="Simple MCAP channel"
#[rerun::rerun_type]
#[docs(category = "MCAP")]
#[rerun(state = "unstable")]
#[rerun(visualizer_none)]
#[rust(derive(PartialEq))]
pub struct McapChannel {
    /// Unique identifier for this channel within the MCAP file.
    ///
    /// Channel IDs must be unique within a single MCAP file and are used to associate
    /// messages with their corresponding channel definition.
    #[rerun(no_ui_edit)]
    #[rerun(required)]
    pub id: rerun::components::ChannelId,

    /// The topic name that this channel publishes to.
    ///
    /// Topics are hierarchical paths from the original robotics system (e.g., "/sensors/camera/image")
    /// that categorize and organize different data streams.
    /// Topics are separate from Rerun's entity paths, but they often can be mapped to them.
    #[rerun(no_ui_edit)]
    #[rerun(required)]
    pub topic: rerun::components::Text,

    /// The encoding format used for messages in this channel.
    ///
    /// Common encodings include:
    /// * `ros1` - ROS1 message format
    /// * `cdr` - Common Data Representation (CDR) message format, used by ROS2
    /// * `protobuf` - Protocol Buffers
    /// * `json` - JSON encoding
    #[rerun(no_ui_edit)]
    #[rerun(required)]
    pub message_encoding: rerun::components::Text,

    /// Additional metadata for this channel stored as key-value pairs.
    ///
    /// This can include channel-specific configuration, description, units, coordinate frames,
    /// or any other contextual information that helps interpret the data in this channel.
    #[rerun(optional)]
    pub metadata: Option<rerun::components::KeyValuePairs>,
}
