/// Label for resources. Optimized out in release builds.
#[derive(Clone, Default, Debug, Hash, PartialEq, Eq)]
pub struct DebugLabel {
    #[cfg(debug_assertions)]
    label: String,
}

impl DebugLabel {
    #[allow(clippy::unnecessary_wraps)]
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
