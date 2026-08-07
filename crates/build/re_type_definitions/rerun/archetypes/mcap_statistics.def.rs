// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Recording-level statistics about an MCAP file.
///
/// This archetype contains summary information about an entire MCAP recording, including
/// counts of messages, schemas, channels, and other records, as well as timing information
/// spanning the full recording duration. It is typically logged once per recording to provide
/// an overview of the dataset's structure and content.
///
/// See also [archetypes.McapChannel] for individual channel definitions,
/// [archetypes.McapMessage] for message content, [archetypes.McapSchema] for schema definitions,
/// and the [MCAP specification](https://mcap.dev/) for complete format details.
///
/// \example archetypes/mcap_statistics_simple !api title="Simple MCAP statistics"
#[rerun::rerun_type]
#[docs(category = "MCAP")]
#[rerun(state = "unstable")]
#[rerun(visualizer_none)]
#[rust(derive(PartialEq))]
pub struct McapStatistics {
    /// Total number of data messages contained in the MCAP recording.
    ///
    /// This count includes all timestamped data messages but excludes metadata records,
    /// schema definitions, and other non-message records.
    #[rerun(no_ui_edit)]
    #[rerun(required)]
    pub message_count: rerun::components::Count,

    /// Number of unique schema definitions in the recording.
    ///
    /// Each schema defines the structure for one or more message types used by channels.
    #[rerun(no_ui_edit)]
    #[rerun(required)]
    pub schema_count: rerun::components::Count,

    /// Number of channels defined in the recording.
    ///
    /// Each channel represents a unique topic and encoding combination for publishing messages.
    #[rerun(no_ui_edit)]
    #[rerun(required)]
    pub channel_count: rerun::components::Count,

    /// Number of file attachments embedded in the recording.
    ///
    /// Attachments can include calibration files, configuration data, or other auxiliary files.
    #[rerun(no_ui_edit)]
    #[rerun(required)]
    pub attachment_count: rerun::components::Count,

    /// Number of metadata records providing additional context about the recording.
    ///
    /// Metadata records contain key-value pairs with information about the recording environment,
    /// system configuration, or other contextual data.
    #[rerun(no_ui_edit)]
    #[rerun(required)]
    pub metadata_count: rerun::components::Count,

    /// Number of data chunks used to organize messages in the file.
    ///
    /// Chunks group related messages together for efficient storage and indexed access.
    #[rerun(no_ui_edit)]
    #[rerun(required)]
    pub chunk_count: rerun::components::Count,

    /// Timestamp of the earliest message in the recording.
    ///
    /// This marks the beginning of the recorded data timeline.
    #[rerun(no_ui_edit)]
    #[rerun(required)]
    pub message_start_time: rerun::components::Timestamp,

    /// Timestamp of the latest message in the recording.
    ///
    /// Together with `message_start_time`, this defines the total duration of the recording.
    #[rerun(no_ui_edit)]
    #[rerun(required)]
    pub message_end_time: rerun::components::Timestamp,

    /// Detailed breakdown of message counts per channel.
    #[rerun(optional)]
    pub channel_message_counts: Option<rerun::components::ChannelMessageCounts>,
}
