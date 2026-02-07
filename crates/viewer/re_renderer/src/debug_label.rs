/// Label for resources. Optimized out in release builds.
#[derive(Clone, Default, Hash, PartialEq, Eq)]
pub struct DebugLabel {
    #[cfg(debug_assertions)]
    label: String,
}

impl std::fmt::Debug for DebugLabel {
    #[cfg(debug_assertions)]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.label.fmt(f)
    }

    #[cfg(not(debug_assertions))]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DebugLabel").finish_non_exhaustive()
    }
}

impl std::fmt::Display for DebugLabel {
    #[cfg(debug_assertions)]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.label.fmt(f)
    }

    #[cfg(not(debug_assertions))]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DebugLabel").finish_non_exhaustive()
    }
}

impl DebugLabel {
    #[expect(clippy::unnecessary_wraps)]
    #[inline]
    pub fn get(&self) -> Option<&str> {
        #[cfg(debug_assertions)]
        {
            Some(&self.label)
        }
        #[cfg(not(debug_assertions))]
        {
            None
        }
    }
}

impl From<&str> for DebugLabel {
    #[inline]
    fn from(str: &str) -> Self {
        #[cfg(not(debug_assertions))]
        {
            _ = str;
        }

        Self {
            #[cfg(debug_assertions)]
            label: str.to_owned(),
        }
    }
}

impl From<String> for DebugLabel {
    #[inline]
    fn from(str: String) -> Self {
        #[cfg(not(debug_assertions))]
        {
            _ = str;
        }

        Self {
            #[cfg(debug_assertions)]
            label: str,
        }
    }
}

impl From<Option<&str>> for DebugLabel {
    #[inline]
    fn from(str: Option<&str>) -> Self {
        #[cfg(not(debug_assertions))]
        {
            _ = str;
        }

        Self {
            #[cfg(debug_assertions)]
            label: str.unwrap_or("").to_owned(),
        }
    }
}
