// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A pair representing a channel ID and its associated message count.
#[rerun::rerun_type]
#[python(aliases = "Tuple[datatypes.UInt16Like, datatypes.UInt64Like]")]
#[rust(derive(Default, PartialEq, Eq, PartialOrd, Ord))]
#[rerun(state = "stable")]
pub struct ChannelCountPair {
    /// The channel ID.
    pub channel_id: rerun::datatypes::UInt16,

    /// The message count for this channel.
    pub message_count: rerun::datatypes::UInt64,
}
