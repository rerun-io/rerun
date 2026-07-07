/// Context to determine which context-dependent commands are currently available.
#[derive(Clone, Debug)]
pub struct CommandEnvironment {
    /// The currently active recording, if any.
    ///
    /// Only then do we have time control, for instance.
    pub recording: Option<re_log_types::StoreId>,

    /// The currently selected Redap server, if any.
    pub redap_server: Option<re_uri::Origin>,

    /// Is the selected Redap server editable (i.e. not the viewer's built-in catalog)?
    pub has_editable_redap_server: bool,

    /// The Redap entry (dataset or table) currently being viewed, if any.
    pub redap_entry: Option<(re_uri::Origin, re_log_types::EntryId)>,
}
