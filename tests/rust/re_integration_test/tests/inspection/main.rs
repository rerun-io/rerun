//! Integration tests driven through the out-of-process `InspectionHarness`.

mod analytics;
mod datasets;
#[cfg(feature = "browser")]
mod exports_browser;
mod internal_catalog;
mod navigation;
