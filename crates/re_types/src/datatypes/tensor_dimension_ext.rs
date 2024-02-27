use super::TensorDimension;

impl TensorDimension {
    const DEFAULT_NAME_WIDTH: &'static str = "width";
    const DEFAULT_NAME_HEIGHT: &'static str = "height";
    const DEFAULT_NAME_DEPTH: &'static str = "depth";

    #[inline]
    pub fn height(size: u64) -> Self {
        Self::named(size, String::from(Self::DEFAULT_NAME_HEIGHT))
    }

    #[inline]
    pub fn width(size: u64) -> Self {
        Self::named(size, String::from(Self::DEFAULT_NAME_WIDTH))
    }

    #[inline]
    pub fn depth(size: u64) -> Self {
        Self::named(size, String::from(Self::DEFAULT_NAME_DEPTH))
    }

    #[inline]
    pub fn named(size: u64, name: String) -> Self {
        Self {
            size,
            name: Some(name.into()),
        }
    }

    #[inline]
    pub fn unnamed(size: u64) -> Self {
        Self { size, name: None }
    }
}

impl std::fmt::Debug for TensorDimension {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(name) = &self.name {
            write!(f, "{}={}", name, self.size)
        } else {
            self.size.fmt(f)
        }
    }
}

impl std::fmt::Display for TensorDimension {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(name) = &self.name {
            write!(f, "{}={}", name, self.size)
        } else {
            self.size.fmt(f)
        }
    }
}
