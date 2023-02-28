/// A value that is either determined automatically by some heuristic, or specified by the user.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, Eq, PartialEq)]
#[serde(bound = "T: serde::Serialize, for<'de2> T: serde::Deserialize<'de2>")]
pub enum EditableAutoValue<T>
where
    T: std::fmt::Debug + Eq + PartialEq + Clone + Default + serde::Serialize,
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
    T: std::fmt::Debug + Eq + PartialEq + Clone + Default + serde::Serialize,
    for<'de2> T: serde::Deserialize<'de2>,
{
    fn default() -> Self {
        EditableAutoValue::Auto(T::default())
    }
}

impl<T> EditableAutoValue<T>
where
    T: std::fmt::Debug + Eq + PartialEq + Clone + Default + serde::Serialize,
    for<'de2> T: serde::Deserialize<'de2>,
{
    pub fn is_auto(&self) -> bool {
        matches!(self, EditableAutoValue::Auto(_))
    }

    /// Gets the value, disregarding if it was user edited or determined by a heuristic.
    pub fn get(&self) -> &T {
        match self {
            EditableAutoValue::Auto(v) | EditableAutoValue::UserEdited(v) => v,
        }
    }
}
