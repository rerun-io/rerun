#[cfg(not(target_arch = "wasm32"))]
mod clipboard;
mod image_cache;
pub(crate) mod log_db;
pub(crate) mod mesh_loader;
#[cfg(all(feature = "puffin", not(target_arch = "wasm32")))]
pub(crate) mod profiler;
pub(crate) mod time_axis;
mod time_control;
mod viewer_context;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use clipboard::Clipboard;

use image_cache::ImageCache;
pub(crate) use log_db::LogDb;
pub(crate) use time_control::{TimeControl, TimeView};

pub(crate) use viewer_context::{Selection, ViewerContext};

// ----------------------------------------------------------------------------

use log_types::{TimePoint, TimeValue};
use std::collections::{BTreeMap, BTreeSet};

/// An aggregate of `TimePoint`:s.
#[derive(Default, serde::Deserialize, serde::Serialize)]
pub struct TimePoints(pub BTreeMap<String, BTreeSet<TimeValue>>);

impl TimePoints {
    pub fn insert(&mut self, time_point: &TimePoint) {
        for (time_key, value) in &time_point.0 {
            self.0.entry(time_key.clone()).or_default().insert(*value);
        }
    }
}
