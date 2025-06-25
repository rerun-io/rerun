//! This crates implements the Redap browser feature, including the communication and UI aspects of
//! it.

mod context;
mod entries;
mod server_modal;
mod servers;
mod tables_session_context;

pub use entries::{EntryKind, dataset_and_its_recordings_ui};
use re_uri::Scheme;
pub use servers::RedapServers;
use std::sync::LazyLock;

/// Origin used to show the examples ui in the redap browser.
///
/// Not actually a valid origin.
pub static EXAMPLES_ORIGIN: LazyLock<re_uri::Origin> = LazyLock::new(|| re_uri::Origin {
    scheme: Scheme::RerunHttps,
    host: url::Host::Domain(String::from("_examples.rerun.io")),
    port: 443,
});

/// Origin used to show the local ui in the redap browser.
///
/// Not actually a valid origin.
pub static LOCAL_ORIGIN: LazyLock<re_uri::Origin> = LazyLock::new(|| re_uri::Origin {
    scheme: Scheme::RerunHttps,
    host: url::Host::Domain(String::from("_local_recordings.rerun.io")),
    port: 443,
});
