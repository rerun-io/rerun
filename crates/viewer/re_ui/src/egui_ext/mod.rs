//! Things that should be upstream moved to egui/eframe at some point

pub mod boxed_widget;
#[cfg(target_os = "macos")]
mod mac_traffic_light_sizes;
pub(crate) mod widget_ext;

#[cfg(target_os = "macos")]
pub use mac_traffic_light_sizes::WindowChromeMetrics;
