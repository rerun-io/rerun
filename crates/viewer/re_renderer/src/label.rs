/// Label for resources.
#[derive(Clone, Default, Hash, PartialEq, Eq)]
pub struct Label {
    label: String,
}

impl std::fmt::Debug for Label {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.label.fmt(f)
    }
}

impl std::fmt::Display for Label {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.label.fmt(f)
    }
}

impl Label {
    #[inline]
    pub fn get(&self) -> &str {
        &self.label
    }

    /// Returns the label as an `Option<&str>` for use with wgpu label fields.
    #[inline]
    #[expect(clippy::unnecessary_wraps)] // We want this to return an option because that's what wgpu labels take.
    pub fn wgpu_label(&self) -> Option<&str> {
        Some(&self.label)
    }
}

impl From<&str> for Label {
    #[inline]
    fn from(str: &str) -> Self {
        Self {
            label: str.to_owned(),
        }
    }
}

impl From<String> for Label {
    #[inline]
    fn from(str: String) -> Self {
        Self { label: str }
    }
}

impl From<Option<&str>> for Label {
    #[inline]
    fn from(str: Option<&str>) -> Self {
        Self {
            label: str.unwrap_or("").to_owned(),
        }
    }
}
