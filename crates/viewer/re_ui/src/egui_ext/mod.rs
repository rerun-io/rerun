//! Things that should be upstream moved to egui/eframe at some point

#[cfg(target_os = "macos")]
mod mac_traffic_light_sizes;

#[cfg(target_os = "macos")]
pub use mac_traffic_light_sizes::WindowChromeMetrics;
