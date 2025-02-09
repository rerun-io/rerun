// ---
re_string_interner::declare_new_type!(
    /// The unique name of a view
    #[cfg_attr(feature = "serde", derive(::serde::Deserialize, ::serde::Serialize))]
    pub struct ViewClassIdentifier;
);

impl ViewClassIdentifier {
    pub fn invalid() -> Self {
        Self::from("invalid")
    }
}

/// Views are the panels shown in the viewer's viewport and the primary means of
/// inspecting & visualizing previously logged data.
///
/// In addition to the data that it contains via `ViewContents`, each view
/// has several view properties that configure how it behaves. Each view property
/// is a [`crate::Archetype`] that is stored in the viewer's blueprint database.
pub trait View {
    fn identifier() -> ViewClassIdentifier;
}
