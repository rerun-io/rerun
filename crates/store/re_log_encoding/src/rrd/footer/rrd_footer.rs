use std::collections::HashMap;

use re_log_types::StoreId;

use super::RawRrdManifest;

/// This is the payload that is carried in messages of type `::End` in RRD streams.
///
/// It keeps track of various useful information about the associated recording.
///
/// During normal operations, there can only be a single `::End` message in an RRD stream, and
/// therefore a single `RrdFooter`.
/// It is possible to break that invariant by concatenating streams using external tools,
/// e.g. by doing something like `cat *.rrd > all_my_recordings.rrd`.
/// Passing that stream back through Rerun tools, e.g. `cat *.rrd | rerun rrd merge > all_my_recordings.rrd`,
/// would once again guarantee that only one `::End` message is present though.
/// I.e. that invariant holds as long as one stays within our ecosystem of tools.
///
/// This is an application-level type, the associated transport-level type can be found
/// over at [`re_protos::log_msg::v1alpha1::RrdFooter`].
#[derive(Default, Debug)]
pub struct RrdFooter {
    /// All the [`RawRrdManifest`]s that were found in this RRD footer.
    ///
    /// Each [`RawRrdManifest`] corresponds to one, and exactly one, RRD stream (i.e. recording).
    ///
    /// The order is unspecified.
    pub manifests: HashMap<StoreId, RawRrdManifest>,
}
