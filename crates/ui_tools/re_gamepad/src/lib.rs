//! Gamepad input handling for the Rerun viewer.
//!
//! The crate keeps device polling separate from view-specific camera behavior.
//!
//! Its default mapping uses a conventional dual-stick gamepad layout
//! and returns 3D navigation intents in Rerun's RUB view space.

use glam::{Vec2, Vec3};

// Backends
#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod web;

// Re-exports
#[cfg(not(target_arch = "wasm32"))]
pub use native::{
    GamepadManager, clear_event_waker, navigation_from_active_gamepad, set_event_waker,
};
#[cfg(target_arch = "wasm32")]
pub use web::{clear_event_waker, navigation_from_active_gamepad, set_event_waker};

/// Raw, device-independent gamepad input used by the standard mapping.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GamepadInput {
    /// Left stick, with X positive right and Y positive up.
    pub left_stick: Vec2,

    /// Right stick, with X positive right and Y positive up.
    pub right_stick: Vec2,

    /// Left trigger pressure in `[0, 1]`.
    pub left_trigger: f32,

    /// Right trigger pressure in `[0, 1]`.
    pub right_trigger: f32,

    /// Slow movement modifier, normally the left bumper.
    pub slow_down: bool,

    /// Fast movement modifier, normally the right bumper.
    pub speed_up: bool,
}

/// Mapped navigation command for a 3D view.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GamepadNavigation {
    /// Movement in local RUB view coordinates.
    ///
    /// X moves right, Y moves up, and negative Z moves forward.
    pub local_movement: Vec3,

    /// Look/orbit delta in radians for this frame.
    ///
    /// Positive X yaws right. Positive Y pitches down.
    pub look_delta_radians: Vec2,

    /// Multiplier for translation speed.
    pub speed_multiplier: f32,
}

impl Default for GamepadNavigation {
    fn default() -> Self {
        Self {
            local_movement: Vec3::ZERO,
            look_delta_radians: Vec2::ZERO,
            speed_multiplier: 1.0,
        }
    }
}

impl GamepadNavigation {
    /// Returns true if this navigation command should change the eye.
    #[inline]
    pub fn is_active(self) -> bool {
        self.local_movement.length_squared() > 1.0e-6
            || self.look_delta_radians.length_squared() > 1.0e-6
    }
}

/// Standard dual-stick mapping from raw gamepad input to 3D navigation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StandardGamepadMapping {
    /// Radial stick dead zone.
    pub stick_dead_zone: f32,

    /// Trigger dead zone.
    pub trigger_dead_zone: f32,

    /// Right stick look/orbit speed in radians per second.
    pub look_speed_radians_per_second: f32,

    /// Translation multiplier while holding the slow modifier.
    pub slow_down_multiplier: f32,

    /// Translation multiplier while holding the speed-up modifier.
    pub speed_up_multiplier: f32,
}

impl Default for StandardGamepadMapping {
    fn default() -> Self {
        Self {
            stick_dead_zone: 0.15,
            trigger_dead_zone: 0.05,
            look_speed_radians_per_second: 2.4,
            slow_down_multiplier: 0.25,
            speed_up_multiplier: 4.0,
        }
    }
}

impl StandardGamepadMapping {
    /// Maps raw input to a navigation command for a single frame.
    #[inline]
    pub fn map_input(self, input: GamepadInput, dt: f32) -> GamepadNavigation {
        let left_stick = apply_stick_dead_zone(input.left_stick, self.stick_dead_zone);
        let right_stick = apply_stick_dead_zone(input.right_stick, self.stick_dead_zone);

        let ascend = apply_trigger_dead_zone(input.right_trigger, self.trigger_dead_zone)
            - apply_trigger_dead_zone(input.left_trigger, self.trigger_dead_zone);

        let mut local_movement = Vec3::new(left_stick.x, ascend, -left_stick.y);
        if local_movement.length_squared() > 1.0 {
            local_movement = local_movement.normalize_or_zero();
        }

        let dt = dt.max(0.0);
        let look_delta_radians =
            Vec2::new(right_stick.x, -right_stick.y) * self.look_speed_radians_per_second * dt;

        let mut speed_multiplier = 1.0;
        if input.slow_down {
            speed_multiplier *= self.slow_down_multiplier;
        }
        if input.speed_up {
            speed_multiplier *= self.speed_up_multiplier;
        }

        GamepadNavigation {
            local_movement,
            look_delta_radians,
            speed_multiplier,
        }
    }
}

fn apply_stick_dead_zone(stick: Vec2, dead_zone: f32) -> Vec2 {
    let dead_zone = dead_zone.clamp(0.0, 0.99);
    let length = stick.length();
    if length <= dead_zone {
        Vec2::ZERO
    } else {
        stick * ((length - dead_zone) / (1.0 - dead_zone) / length)
    }
}

fn apply_trigger_dead_zone(trigger: f32, dead_zone: f32) -> f32 {
    let dead_zone = dead_zone.clamp(0.0, 0.99);
    let trigger = trigger.clamp(0.0, 1.0);
    if trigger <= dead_zone {
        0.0
    } else {
        (trigger - dead_zone) / (1.0 - dead_zone)
    }
}

#[cfg(test)]
mod tests {
    use super::{GamepadInput, StandardGamepadMapping};
    use glam::{Vec2, Vec3};

    fn assert_vec2_close(actual: Vec2, expected: Vec2) {
        assert!(
            (actual - expected).abs().max_element() < 1.0e-4,
            "expected {expected:?}, got {actual:?}"
        );
    }

    fn assert_vec3_close(actual: Vec3, expected: Vec3) {
        assert!(
            (actual - expected).abs().max_element() < 1.0e-4,
            "expected {expected:?}, got {actual:?}"
        );
    }

    #[test]
    fn maps_left_stick_to_view_space_translation() {
        let mapping = StandardGamepadMapping {
            stick_dead_zone: 0.0,
            ..Default::default()
        };

        let navigation = mapping.map_input(
            GamepadInput {
                left_stick: Vec2::new(0.5, 1.0),
                ..Default::default()
            },
            1.0,
        );

        assert_vec3_close(navigation.local_movement, Vec3::new(0.4472, 0.0, -0.8944));
    }

    #[test]
    fn maps_triggers_to_vertical_translation() {
        let mapping = StandardGamepadMapping {
            trigger_dead_zone: 0.0,
            ..Default::default()
        };

        let navigation = mapping.map_input(
            GamepadInput {
                left_trigger: 0.25,
                right_trigger: 0.75,
                ..Default::default()
            },
            1.0,
        );

        assert_vec3_close(navigation.local_movement, Vec3::new(0.0, 0.5, 0.0));
    }

    #[test]
    fn maps_right_stick_to_look_delta() {
        let mapping = StandardGamepadMapping {
            stick_dead_zone: 0.0,
            look_speed_radians_per_second: 2.0,
            ..Default::default()
        };

        let navigation = mapping.map_input(
            GamepadInput {
                right_stick: Vec2::new(0.5, 1.0),
                ..Default::default()
            },
            0.5,
        );

        assert_vec2_close(navigation.look_delta_radians, Vec2::new(0.5, -1.0));
    }

    #[test]
    fn applies_speed_modifiers() {
        let mapping = StandardGamepadMapping::default();

        let navigation = mapping.map_input(
            GamepadInput {
                slow_down: true,
                speed_up: true,
                ..Default::default()
            },
            1.0,
        );

        assert_eq!(navigation.speed_multiplier, 1.0);
    }
}
