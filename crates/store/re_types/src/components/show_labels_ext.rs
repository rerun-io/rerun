use super::ShowLabels;

impl Default for ShowLabels {
    #[inline]
    fn default() -> Self {
        // We don't actually use this default -- visualizers choose a fallback value --
        // but it is necessary to satisfy `re_viewer::reflection::generate_component_reflection()`.
        Self(true.into())
    }
}

impl From<ShowLabels> for bool {
    #[inline]
    fn from(value: ShowLabels) -> Self {
        value.0.into()
    }
}
