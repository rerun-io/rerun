// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// The binary payload of a single MCAP message, without metadata.
///
/// This archetype represents only the raw message data from an MCAP file. It does not include
/// MCAP message metadata such as timestamps, channel IDs, sequence numbers, or publication times.
/// The binary payload represents sensor data, commands, or other information encoded according
/// to the format specified by the associated channel.
///
/// See [archetypes.McapChannel] for channel definitions that specify message encoding,
/// [archetypes.McapSchema] for data structure definitions, and the
/// [MCAP specification](https://mcap.dev/) for complete format details.
///
/// \example archetypes/mcap_message_simple !api title="Simple MCAP message"
#[rerun::rerun_type]
#[docs(category = "MCAP")]
#[rerun(state = "unstable")]
#[rerun(visualizer_none)]
#[rust(derive(PartialEq))]
pub struct McapMessage {
    /// The raw message payload as a binary blob.
    ///
    /// This contains the actual message data encoded according to the format specified
    /// by the associated channel's `message_encoding` field. The structure and interpretation
    /// of this binary data depends on the encoding format (e.g., ros1, cdr, protobuf)
    /// and the message schema defined for the channel.
    #[rerun(no_ui_edit)]
    #[rerun(required)]
    pub data: rerun::components::Blob,
}
