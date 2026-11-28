//! Rerun Web View
//!
//! Shows a web page, loaded from a URL or given as raw HTML in the blueprint.
//! On the web it shows the page in an `<iframe>`.
//! Native platforms can't show pages yet: the native backend of `egui_webview` is CEF
//! (Chromium), which downloads about 1 GB when it is built.
//!
//! Create a [`Runtime`] early in `main`.

mod view_class;

pub use view_class::WebView;

pub use egui_webview::{Runtime, WebViews, is_supported};
