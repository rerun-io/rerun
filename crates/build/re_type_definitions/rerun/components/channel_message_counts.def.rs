// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A mapping of channel IDs to their respective message counts.
///
/// Used in MCAP statistics to track how many messages were recorded per channel.
#[rerun::rerun_type]
#[python(aliases = "dict[int, int]")]
#[python(array_aliases = "dict[int, int] | Sequence[dict[int, int]]")]
#[rerun(state = "unstable")]
#[rust(derive(Default, PartialEq, Eq))]
pub struct ChannelMessageCounts {
    /// The channel ID to message count pairs.
    pub counts: Vec<rerun::datatypes::ChannelCountPair>,
}
