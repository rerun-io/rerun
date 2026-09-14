//! Gamepad backend for web (wasm), currently only no-op.

// TODO(RR-4792): add gamepad support for wasm.

use crate::GamepadNavigation;

pub fn set_event_waker(_wake_callback: impl Fn() + Send + Sync + 'static) {}

pub fn clear_event_waker() {}

pub fn navigation_from_active_gamepad(_dt: f32) -> Option<GamepadNavigation> {
    None
}
