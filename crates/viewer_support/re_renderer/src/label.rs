/// Label for resources.
///
/// Labels are kept around in release builds (they're cheap and make debugging
/// released applications far easier), but they must *not* influence GPU resource
/// pooling: two otherwise-identical descriptors that differ only by label should
/// still be able to re-use the same texture/buffer/pipeline.
///
/// We therefore ignore the label contents in `Hash`/`Eq` on release builds, so
/// pool matching is label-independent. On debug builds we keep the label in
/// `Hash`/`Eq` — this preserves the previous behavior and keeps labels stable in
/// graphics debuggers (the downside being that a pooled resource might have been
/// created under a different, still-valid label; see #8640).
#[derive(Clone, Default)]
pub struct Label {
    label: String,
}

// On debug builds the label participates in pool matching (previous behavior).
// On release builds it's ignored so that pooling doesn't depend on the label.
#[cfg(debug_assertions)]
impl std::hash::Hash for Label {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.label.hash(state);
    }
}
#[cfg(not(debug_assertions))]
impl std::hash::Hash for Label {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, _state: &mut H) {}
}

#[cfg(debug_assertions)]
impl PartialEq for Label {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.label == other.label
    }
}
#[cfg(not(debug_assertions))]
impl PartialEq for Label {
    #[inline]
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl Eq for Label {}

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

#[cfg(test)]
mod tests {
    use super::Label;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash as _, Hasher as _};

    fn hash(label: &Label) -> u64 {
        let mut hasher = DefaultHasher::new();
        label.hash(&mut hasher);
        hasher.finish()
    }

    #[test]
    fn label_kept_but_ignored_for_matching_on_release() {
        let a = Label::from("foo");
        let b = Label::from("bar");

        // The label text is always preserved (release included), so debuggers still see it.
        assert_eq!(a.get(), "foo");
        assert_eq!(b.get(), "bar");

        if cfg!(debug_assertions) {
            // Debug builds: label participates in matching (previous behavior).
            assert_ne!(a, b);
            assert_ne!(hash(&a), hash(&b));
        } else {
            // Release builds: label is ignored for pool matching, so two resources
            // that differ only by label can still be re-used. See #8640.
            assert_eq!(a, b);
            assert_eq!(hash(&a), hash(&b));
        }
    }
}
