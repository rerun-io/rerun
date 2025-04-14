// TODO(grtlr): It would be nice, if we could store the welcome screen state within its variant here too.

/// Which display mode are we currently in?
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisplayMode {
    /// The welcome screen with example recordings.
    WelcomeScreen,

    /// The settings dialog for application-wide configuration.
    Settings,

    /// Regular view of the local recordings, including the current recording's viewport.
    LocalRecordings,

    /// The Redap server/catalog/collection browser.
    RedapEntry(re_log_types::EntryId),
    RedapServer(re_uri::Origin),

    /// The current recording's data store browser.
    ChunkStoreBrowser,
}
