/// A value that is either determined automatically by some heuristic, or specified by the user.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, Eq, PartialEq)]
#[serde(bound = "T: serde::Serialize, for<'de2> T: serde::Deserialize<'de2>")]
pub enum EditableAutoValue<T>
where
    T: std::fmt::Debug + Clone + Default + PartialEq + serde::Serialize,
    for<'de2> T: serde::Deserialize<'de2>,
{
    /// The user explicitly specified what they wanted
    UserEdited(T),

    /// The value is determined automatically.
    ///
    /// We may update this at any time or interpret the value stored here differently under certain circumstances.
    Auto(T),
}

impl<T> Default for EditableAutoValue<T>
where
    T: std::fmt::Debug + Clone + Default + PartialEq + serde::Serialize,
    for<'de2> T: serde::Deserialize<'de2>,
{
    #[inline]
    fn default() -> Self {
        EditableAutoValue::Auto(T::default())
    }
}

impl<T> EditableAutoValue<T>
where
    T: std::fmt::Debug + Clone + Default + PartialEq + serde::Serialize,
    for<'de2> T: serde::Deserialize<'de2>,
{
    #[inline]
    pub fn is_auto(&self) -> bool {
        matches!(self, EditableAutoValue::Auto(_))
    }

    /// Gets the value, disregarding if it was user edited or determined by a heuristic.
    #[inline]
    pub fn get(&self) -> &T {
        match self {
            EditableAutoValue::Auto(v) | EditableAutoValue::UserEdited(v) => v,
        }
    }

    /// Returns other if self is auto, self otherwise.
    #[inline]
    pub fn or<'a>(&'a self, other: &'a EditableAutoValue<T>) -> &'a EditableAutoValue<T> {
        if self.is_auto() {
            other
        } else {
            self
        }
    }

    /// Determine whether this `EditableAutoValue` has user-edits relative to another `EditableAutoValue`
    /// If both values are `Auto`, then it is not considered edited.
    #[inline]
    pub fn has_edits(&self, other: &Self) -> bool {
        match (self, other) {
            (EditableAutoValue::UserEdited(s), EditableAutoValue::UserEdited(o)) => s != o,
            (EditableAutoValue::Auto(_), EditableAutoValue::Auto(_)) => false,
            _ => true,
        }
    }
}

impl<T> std::ops::Deref for EditableAutoValue<T>
where
    T: std::fmt::Debug + Clone + Default + PartialEq + serde::Serialize,
    for<'de2> T: serde::Deserialize<'de2>,
{
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        match self {
            EditableAutoValue::Auto(v) | EditableAutoValue::UserEdited(v) => v,
        }
    }
}
