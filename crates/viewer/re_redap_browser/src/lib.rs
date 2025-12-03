//! This crates implements the Redap browser feature, including the communication and UI aspects of
//! it.

#![warn(clippy::iter_over_hash_type)] //  TODO(#6198): enable everywhere

mod context;
mod entries;
mod server_modal;
mod servers;

use std::sync::LazyLock;

use re_uri::Scheme;
pub use re_viewer_context::open_url::EXAMPLES_ORIGIN;

pub use self::entries::{Entries, Entry, EntryInner};
pub use self::servers::{Command, RedapServers, Server};

/// Origin used to show the local ui in the redap browser.
///
/// Not actually a valid origin.
pub static LOCAL_ORIGIN: LazyLock<re_uri::Origin> = LazyLock::new(|| re_uri::Origin {
    scheme: Scheme::RerunHttps,
    host: url::Host::Domain(String::from("_local_recordings.rerun.io")),
    port: 443,
});

/// Utility function to switch to the examples screen.
pub fn switch_to_welcome_screen(command_sender: &re_viewer_context::CommandSender) {
    use re_viewer_context::{SystemCommand, SystemCommandSender as _};

    command_sender.send_system(SystemCommand::ChangeDisplayMode(
        re_viewer_context::DisplayMode::RedapServer(EXAMPLES_ORIGIN.clone()),
    ));
    command_sender.send_system(SystemCommand::set_selection(
        re_viewer_context::Item::RedapServer(EXAMPLES_ORIGIN.clone()),
    ));
}
