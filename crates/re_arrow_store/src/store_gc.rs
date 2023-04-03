#[derive(Debug, Clone, Copy)]
pub enum GarbageCollectionTarget {
    /// Try to drop _at least_ the given percentage.
    ///
    /// The percentage must be a float in the range [0.0 : 1.0].
    DropAtLeastPercentage(f64),
}

impl std::fmt::Display for GarbageCollectionTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GarbageCollectionTarget::DropAtLeastPercentage(p) => f.write_fmt(format_args!(
                "DropAtLeast({}%)",
                re_format::format_f64(*p * 100.0)
            )),
        }
    }
}

// TODO(#1619): Implement garbage collection.
