//! This crates implements the Redap browser feature, including the communication and UI aspects of
//! it.

mod context;
mod entries;
mod entry_meta;
mod folder_card_ui;
mod server_modal;
mod servers;

pub use re_viewer_context::open_url::EXAMPLES_ORIGIN;

pub use self::entries::{Entries, Entry, EntryInner};
pub use self::servers::{Command, RedapServers, Server};

/// Utility function to switch to the examples screen.
pub fn switch_to_welcome_screen(command_sender: &re_viewer_context::CommandSender) {
    use re_viewer_context::{SystemCommand, SystemCommandSender as _};

    command_sender.send_system(SystemCommand::SetRoute(
        re_viewer_context::Route::welcome_page(),
    ));
    command_sender.send_system(SystemCommand::set_selection(
        re_viewer_context::Item::welcome_page(),
    ));
}
