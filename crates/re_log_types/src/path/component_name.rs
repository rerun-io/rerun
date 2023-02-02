re_string_interner::declare_new_type!(
    /// The name of an entity component, e.g. `pos` or `color`.
    pub struct ComponentName;
);

impl ComponentName {
    /// Includes namespace, e.g. `rerun.color` or `ext.confidence`.
    ///
    /// This is also the default `Display` etc for [`ComponentName`].
    #[inline]
    pub fn full_name(&self) -> &'static str {
        self.0.as_str()
    }

    /// Excludes namespace, e.g. `color` or `confidence`.
    ///
    /// Used for most UI elements.
    pub fn short_name(&self) -> &'static str {
        let full_name = self.0.as_str();
        if let Some(short_name) = full_name.strip_prefix("rerun.") {
            short_name
        } else if let Some(short_name) = full_name.strip_prefix("ext.") {
            short_name
        } else {
            full_name
        }
    }
}
