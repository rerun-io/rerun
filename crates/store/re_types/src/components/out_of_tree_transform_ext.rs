use super::OutOfTreeTransform;

impl OutOfTreeTransform {
    /// Enabled out of tree transform.
    pub const ENABLED: Self = Self(crate::datatypes::Bool(true));

    /// Disabled out of tree transform.
    pub const DISABLED: Self = Self(crate::datatypes::Bool(false));
}
