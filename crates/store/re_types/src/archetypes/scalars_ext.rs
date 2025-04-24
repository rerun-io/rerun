impl crate::archetypes::Scalars {
    /// Constructor for a single scalar.
    pub fn single(value: impl Into<crate::components::Scalar>) -> Self {
        Self::new([value.into()])
    }
}
