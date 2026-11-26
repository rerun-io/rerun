//! Shared plot utilities for legends, tooltips and rulers.

mod axis_tick;
pub mod legend;
mod rulers;
pub mod tooltip;

pub use axis_tick::{format_axis_tick, format_axis_tick_with_max_chars};
pub use rulers::{Rulers, RulersLayout};
