/// Which display mode are we currently in?
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisplayMode {
    WelcomeScreen,

    /// Regular view of the local recordings, including the current recording's viewport.
    LocalRecordings,

    /// The Redap server/catalog/collection browser.
    RedapEntry(re_log_types::EntryId),
    RedapServer(re_uri::Origin),

    /// The current recording's data store browser.
    ChunkStoreBrowser,
}
