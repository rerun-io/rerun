/// This is the message type that is passed in the footer of RRD streams.
///
/// It is possible to break that invariant by concatenating streams using external tools,
/// e.g. by doing something like `cat *.rrd > all_my_recordings.rrd`.
/// Passing that stream back through Rerun tools, e.g. `cat *.rrd | rerun rrd route > all_my_recordings.rrd`,
/// would once again guarantee that only one footer is present though.
/// I.e. that invariant holds as long as one stays within our ecosystem of tools.
///
/// It is transported using the `MessageKind::End` tag.
///
/// This is an application-level type, the associated transport-level type can be found
/// over at [`re_protos::log_msg::v1alpha1::RrdFooter`].
#[derive(Default, Debug)]
pub struct RrdFooter {}
