//! Localization support for the Rerun Viewer.
//!
//! Provides a [`Localizer`] trait for translating UI strings,
//! and a global static that can be set at startup.

use std::sync::OnceLock;

/// Trait for translating UI string keys into localized text.
pub trait Localizer: Send + Sync {
    /// Translate a key to its localized form.
    /// If the key is not recognized, returns the key itself (English fallback).
    fn t<'a>(&self, key: &'a str) -> &'a str;
}

/// A no-op localizer that returns keys as-is.
pub struct NullLocalizer;

impl Localizer for NullLocalizer {
    fn t<'a>(&self, key: &'a str) -> &'a str {
        key
    }
}

static GLOBAL_LOCALIZER: OnceLock<&'static dyn Localizer> = OnceLock::new();

/// Set the global localizer. Must be called once at startup.
pub fn set_global_localizer(loc: &'static dyn Localizer) {
    GLOBAL_LOCALIZER.set(loc).ok();
}

/// Get the current localizer. Falls back to [`NullLocalizer`] if none is set.
pub fn localizer() -> &'static dyn Localizer {
    GLOBAL_LOCALIZER.get().copied().unwrap_or(&NullLocalizer)
}

/// Convenience function to translate a string key.
pub fn t<'a>(key: &'a str) -> &'a str {
    localizer().t(key)
}
