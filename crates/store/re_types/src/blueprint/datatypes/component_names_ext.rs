use super::ComponentNames;

impl ComponentNames {
    /// Returns an iterator over the component names.
    #[inline]
    pub fn name_iter(&self) -> impl Iterator<Item = re_types_core::ComponentName> + '_ {
        self.0.iter().map(|s| s.as_str().into())
    }

    /// Set content to the provided component names.
    pub fn set_names(&mut self, names: impl IntoIterator<Item = re_types_core::ComponentName>) {
        self.0.clear();
        self.0
            .extend(names.into_iter().map(|name| name.as_str().into()));
    }
}
