//! Keyboard handler for WASD movement controls that publish Twist messages.
//!
//! Converts keyboard input to robot velocity commands following teleop conventions:
//! - WASD/arrows for linear/angular motion
//! - QE for strafing
//! - Space for emergency stop
//! - Shift for speed multiplier

use super::ws::WsPublisher;
use rerun::external::{egui, re_log};

/// Base speeds for keyboard control
const BASE_LINEAR_SPEED: f64 = 0.5;   // m/s
const BASE_ANGULAR_SPEED: f64 = 0.8;  // rad/s
const FAST_MULTIPLIER: f64 = 2.0;     // Shift modifier

/// Overlay styling
const OVERLAY_PADDING: f32 = 10.0;
const OVERLAY_ROUNDING: f32 = 8.0;
const OVERLAY_BG: egui::Color32 = egui::Color32::from_rgba_premultiplied(20, 20, 30, 220);
const KEY_SIZE: f32 = 32.0;
const KEY_GAP: f32 = 3.0;
const KEY_ACTIVE_BG: egui::Color32 = egui::Color32::from_rgb(60, 180, 75);
const KEY_INACTIVE_BG: egui::Color32 = egui::Color32::from_rgba_premultiplied(60, 60, 80, 180);
const KEY_TEXT_COLOR: egui::Color32 = egui::Color32::WHITE;
const LABEL_COLOR: egui::Color32 = egui::Color32::from_rgb(180, 180, 200);
const ESTOP_ACTIVE_BG: egui::Color32 = egui::Color32::from_rgb(220, 50, 50);

/// Tracks which movement keys are currently held down.
#[derive(Debug, Clone, Default)]
struct KeyState {
    forward: bool,   // W or Up
    backward: bool,  // S or Down
    left: bool,      // A or Left
    right: bool,     // D or Right
    strafe_l: bool,  // Q
    strafe_r: bool,  // E
    fast: bool,      // Shift held
}

impl KeyState {
    fn new() -> Self {
        Default::default()
    }

    /// Returns true if any movement key is currently active
    fn any_active(&self) -> bool {
        self.forward || self.backward || self.left || self.right || self.strafe_l || self.strafe_r
    }

    /// Reset all key states (used for emergency stop)
    fn reset(&mut self) {
        self.forward = false;
        self.backward = false;
        self.left = false;
        self.right = false;
        self.strafe_l = false;
        self.strafe_r = false;
        self.fast = false;
    }
}

/// Handles keyboard input and publishes Twist via WebSocket.
/// Must be activated by clicking the overlay before keys are captured.
pub struct KeyboardHandler {
    ws: WsPublisher,
    state: KeyState,
    was_active: bool,
    estop_flash: bool,  // true briefly after space pressed
    engaged: bool,      // true when user has clicked the overlay to activate
}

impl KeyboardHandler {
    /// Create a new keyboard handler that publishes twist commands via WebSocket.
    pub fn new(ws: WsPublisher) -> Self {
        Self {
            ws,
            state: KeyState::new(),
            was_active: false,
            estop_flash: false,
            engaged: false,
        }
    }

    /// Process keyboard input from egui and publish Twist if keys are held.
    /// Called once per frame from DimosApp.ui().
    /// Only captures keys when the overlay has been clicked (engaged).
    ///
    /// Returns true if any movement key is active (for UI overlay).
    pub fn process(&mut self, ctx: &egui::Context) -> bool {
        self.estop_flash = false;

        // If not engaged, don't capture any keys
        if !self.engaged {
            if self.was_active {
                if let Err(e) = self.publish_stop() {
                    re_log::warn!("Failed to send stop on disengage: {e}");
                }
                self.was_active = false;
            }
            return false;
        }

        // Update key state from egui input (engaged flag is the only gate)
        self.update_key_state(ctx);

        // Check for emergency stop (Space key pressed - one-shot action)
        if ctx.input(|i| i.key_pressed(egui::Key::Space)) {
            self.state.reset();
            if let Err(e) = self.publish_stop() {
                re_log::warn!("Failed to send emergency stop: {e}");
            }
            self.was_active = false;
            self.estop_flash = true;
            return true; // return true so overlay shows the e-stop flash
        }

        // Publish twist command if keys are active, or stop if just released
        if self.state.any_active() {
            if let Err(e) = self.publish_twist() {
                re_log::warn!("Failed to publish twist command: {e}");
            }
            self.was_active = true;
        } else if self.was_active {
            if let Err(e) = self.publish_stop() {
                re_log::warn!("Failed to send stop on key release: {e}");
            }
            self.was_active = false;
        }

        self.state.any_active()
    }

    /// Draw keyboard overlay HUD anchored to the bottom-right of the viewport.
    /// Clickable: clicking the overlay toggles engaged state.
    pub fn draw_overlay(&mut self, ctx: &egui::Context) {
        let area_response = egui::Area::new("dimos_keyboard_hud_br".into())
            .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-5.0, -5.0))
            .order(egui::Order::Foreground)
            .interactable(true)
            // Sense drags too (default for a non-movable interactable Area is
            // CLICK-only), otherwise click-with-tiny-mouse-motion leaks through
            // to the 3D viewport's camera-drag handler.
            .sense(egui::Sense::click_and_drag())
            .show(ctx, |ui| {
                let border_color = if self.engaged {
                    egui::Color32::from_rgb(60, 180, 75) // green border when active
                } else {
                    egui::Color32::from_rgb(80, 80, 100) // dim border when inactive
                };

                let response = egui::Frame::new()
                    .fill(OVERLAY_BG)
                    .corner_radius(egui::CornerRadius::same(OVERLAY_ROUNDING as u8))
                    .inner_margin(egui::Margin::same(OVERLAY_PADDING as i8))
                    .stroke(egui::Stroke::new(2.0, border_color))
                    .show(ui, |ui| {
                        self.draw_hud_content(ui);
                    });

                // Make the frame rect clickable (Frame doesn't have click sense by default)
                let click_response = ui.interact(
                    response.response.rect,
                    ui.id().with("wasd_click"),
                    egui::Sense::click(),
                );

                // Force arrow cursor over the entire overlay (overrides label I-beam)
                if click_response.hovered() {
                    ctx.set_cursor_icon(egui::CursorIcon::Default);
                }

                // Toggle engaged state on click
                if click_response.clicked() {
                    self.engaged = !self.engaged;
                    if !self.engaged {
                        // Send stop when disengaging
                        if let Err(err) = self.publish_stop() {
                            re_log::warn!("Failed to send stop on disengage: {err}");
                        }
                        self.state.reset();
                        self.was_active = false;
                    }
                }
            })
            .response;

        // Disengage when clicking anywhere outside the overlay
        if self.engaged
            && !ctx.rect_contains_pointer(area_response.layer_id, area_response.interact_rect)
            && ctx.input(|i| i.pointer.primary_clicked())
        {
            self.engaged = false;
            if let Err(err) = self.publish_stop() {
                re_log::warn!("Failed to send stop on outside click: {err}");
            }
            self.state.reset();
            self.was_active = false;
        }
    }

    fn draw_hud_content(&self, ui: &mut egui::Ui) {
        // Title
        ui.label(egui::RichText::new("Keyboard Teleop").color(LABEL_COLOR).size(13.0));
        ui.add_space(4.0);

        // Key grid:  [Q] [W] [E]
        //            [A] [S] [D]
        //            [  SPACE  ]
        let row1 = [
            ("Q", self.state.strafe_l),
            ("W", self.state.forward),
            ("E", self.state.strafe_r),
        ];
        let row2 = [
            ("A", self.state.left),
            ("S", self.state.backward),
            ("D", self.state.right),
        ];

        // Row 1
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = KEY_GAP;
            for (label, pressed) in &row1 {
                self.draw_key(ui, label, *pressed);
            }
        });

        // Row 2
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = KEY_GAP;
            for (label, pressed) in &row2 {
                self.draw_key(ui, label, *pressed);
            }
        });

        // Space bar (e-stop)
        let space_width = KEY_SIZE * 3.0 + KEY_GAP * 2.0;
        let space_rect = ui.allocate_exact_size(
            egui::vec2(space_width, KEY_SIZE * 0.7),
            egui::Sense::hover(),
        ).0;
        let space_bg = if self.estop_flash {
            ESTOP_ACTIVE_BG
        } else {
            KEY_INACTIVE_BG
        };
        ui.painter().rect_filled(space_rect, egui::CornerRadius::same(4), space_bg);
        ui.painter().text(
            space_rect.center(),
            egui::Align2::CENTER_CENTER,
            "STOP",
            egui::FontId::proportional(11.0),
            KEY_TEXT_COLOR,
        );

        ui.add_space(4.0);

        // Speed indicator
        let speed_label = if self.state.fast { "⇧ FAST" } else { "⇧ shift=fast" };
        let speed_color = if self.state.fast {
            egui::Color32::from_rgb(255, 200, 50)
        } else {
            LABEL_COLOR
        };
        ui.label(egui::RichText::new(speed_label).color(speed_color).size(10.0));
    }

    fn draw_key(&self, ui: &mut egui::Ui, label: &str, pressed: bool) {
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(KEY_SIZE, KEY_SIZE),
            egui::Sense::hover(),
        );
        let bg = if pressed { KEY_ACTIVE_BG } else { KEY_INACTIVE_BG };
        ui.painter().rect_filled(rect, egui::CornerRadius::same(4), bg);
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::monospace(14.0),
            KEY_TEXT_COLOR,
        );
    }

    /// Read current key state from egui input, update self.state.
    fn update_key_state(&mut self, ctx: &egui::Context) {
        ctx.input(|i| {
            self.state.forward = i.key_down(egui::Key::W) || i.key_down(egui::Key::ArrowUp);
            self.state.backward = i.key_down(egui::Key::S) || i.key_down(egui::Key::ArrowDown);
            self.state.left = i.key_down(egui::Key::A) || i.key_down(egui::Key::ArrowLeft);
            self.state.right = i.key_down(egui::Key::D) || i.key_down(egui::Key::ArrowRight);
            self.state.strafe_l = i.key_down(egui::Key::Q);
            self.state.strafe_r = i.key_down(egui::Key::E);
            self.state.fast = i.modifiers.shift;
        });
    }

    /// Convert current KeyState to Twist and publish via WebSocket.
    fn publish_twist(&mut self) -> Result<(), super::ws::SendError> {
        let (lin_x, lin_y, lin_z, ang_x, ang_y, ang_z) = self.compute_twist();
        self.ws.send_twist(lin_x, lin_y, lin_z, ang_x, ang_y, ang_z)?;

        if std::env::var("DIMOS_DEBUG").is_ok_and(|v| v == "1") {
            eprintln!(
                "[DIMOS_DEBUG] Published twist: lin=({:.2},{:.2},{:.2}) ang=({:.2},{:.2},{:.2})",
                lin_x, lin_y, lin_z, ang_x, ang_y, ang_z
            );
        }
        Ok(())
    }

    /// Publish all-zero twist (stop command) via WebSocket.
    fn publish_stop(&mut self) -> Result<(), super::ws::SendError> {
        self.ws.send_stop()?;
        if std::env::var("DIMOS_DEBUG").is_ok_and(|v| v == "1") {
            eprintln!("[DIMOS_DEBUG] Published stop command");
        }
        Ok(())
    }

    /// Map KeyState to linear/angular velocities.
    fn compute_twist(&self) -> (f64, f64, f64, f64, f64, f64) {
        let mut linear_x = 0.0;
        let mut linear_y = 0.0;
        let mut angular_z = 0.0;

        if self.state.forward {
            linear_x += BASE_LINEAR_SPEED;
        }
        if self.state.backward {
            linear_x -= BASE_LINEAR_SPEED;
        }
        if self.state.strafe_l {
            linear_y += BASE_LINEAR_SPEED;
        }
        if self.state.strafe_r {
            linear_y -= BASE_LINEAR_SPEED;
        }
        if self.state.left {
            angular_z += BASE_ANGULAR_SPEED;
        }
        if self.state.right {
            angular_z -= BASE_ANGULAR_SPEED;
        }
        if self.state.fast {
            linear_x *= FAST_MULTIPLIER;
            linear_y *= FAST_MULTIPLIER;
            angular_z *= FAST_MULTIPLIER;
        }

        (linear_x, linear_y, 0.0, 0.0, 0.0, angular_z)
    }
}

impl std::fmt::Debug for KeyboardHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeyboardHandler")
            .field("state", &self.state)
            .field("was_active", &self.was_active)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a dummy WsPublisher for tests (connects to a non-existent server,
    /// which is fine — we only test compute_twist, never actually send).
    fn test_ws() -> WsPublisher {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async { WsPublisher::connect("ws://127.0.0.1:1/test".to_string()) })
    }

    fn handler_with(state: KeyState) -> KeyboardHandler {
        KeyboardHandler {
            ws: test_ws(),
            state,
            was_active: false,
            estop_flash: false,
            engaged: true,
        }
    }

    #[test]
    fn test_key_state_any_active() {
        let mut state = KeyState::new();
        assert!(!state.any_active());

        state.forward = true;
        assert!(state.any_active());

        state.reset();
        assert!(!state.any_active());

        state.strafe_l = true;
        assert!(state.any_active());
    }

    #[test]
    fn test_wasd_to_twist_mapping() {
        let mut state = KeyState::new();
        state.forward = true;
        let handler = handler_with(state);
        let (lin_x, lin_y, _, _, _, ang_z) = handler.compute_twist();
        assert_eq!(lin_x, BASE_LINEAR_SPEED);
        assert_eq!(lin_y, 0.0);
        assert_eq!(ang_z, 0.0);
    }

    #[test]
    fn test_turn_left_right_mapping() {
        let mut state = KeyState::new();
        state.left = true;
        let handler = handler_with(state);
        let (lin_x, lin_y, _, _, _, ang_z) = handler.compute_twist();
        assert_eq!(lin_x, 0.0);
        assert_eq!(lin_y, 0.0);
        assert_eq!(ang_z, BASE_ANGULAR_SPEED);

        let mut state = KeyState::new();
        state.right = true;
        let handler = handler_with(state);
        let (lin_x, lin_y, _, _, _, ang_z) = handler.compute_twist();
        assert_eq!(lin_x, 0.0);
        assert_eq!(lin_y, 0.0);
        assert_eq!(ang_z, -BASE_ANGULAR_SPEED);
    }

    #[test]
    fn test_strafe_mapping() {
        let mut state = KeyState::new();
        state.strafe_l = true;
        let handler = handler_with(state);
        let (lin_x, lin_y, _, _, _, ang_z) = handler.compute_twist();
        assert_eq!(lin_x, 0.0);
        assert_eq!(lin_y, BASE_LINEAR_SPEED);
        assert_eq!(ang_z, 0.0);

        let mut state = KeyState::new();
        state.strafe_r = true;
        let handler = handler_with(state);
        let (lin_x, lin_y, _, _, _, ang_z) = handler.compute_twist();
        assert_eq!(lin_x, 0.0);
        assert_eq!(lin_y, -BASE_LINEAR_SPEED);
        assert_eq!(ang_z, 0.0);
    }

    #[test]
    fn test_shift_doubles_speed() {
        let mut state = KeyState::new();
        state.forward = true;
        state.fast = true;
        let handler = handler_with(state);
        let (lin_x, lin_y, _, _, _, ang_z) = handler.compute_twist();
        assert_eq!(lin_x, BASE_LINEAR_SPEED * FAST_MULTIPLIER);
        assert_eq!(lin_y, 0.0);
        assert_eq!(ang_z, 0.0);
    }

    #[test]
    fn test_simultaneous_keys() {
        let mut state = KeyState::new();
        state.forward = true;
        state.left = true;
        let handler = handler_with(state);
        let (lin_x, lin_y, _, _, _, ang_z) = handler.compute_twist();
        assert_eq!(lin_x, BASE_LINEAR_SPEED);
        assert_eq!(lin_y, 0.0);
        assert_eq!(ang_z, BASE_ANGULAR_SPEED);
    }

    #[test]
    fn test_key_reset() {
        let mut state = KeyState::new();
        state.forward = true;
        state.left = true;
        state.fast = true;
        assert!(state.any_active());
        state.reset();
        assert!(!state.forward);
        assert!(!state.left);
        assert!(!state.fast);
        assert!(!state.any_active());
    }

    #[test]
    fn test_keyboard_handler_creation() {
        let handler = KeyboardHandler::new(test_ws());
        assert!(!handler.was_active);
        assert!(!handler.engaged);
        assert!(!handler.state.any_active());
    }

    #[test]
    fn test_opposite_keys_cancel() {
        let mut state = KeyState::new();
        state.forward = true;
        state.backward = true;
        let handler = handler_with(state);
        let (lin_x, lin_y, _, _, _, ang_z) = handler.compute_twist();
        assert_eq!(lin_x, 0.0);
        assert_eq!(lin_y, 0.0);
        assert_eq!(ang_z, 0.0);
    }

    #[test]
    fn test_compute_twist_all_zeros() {
        let handler = handler_with(KeyState::new());
        let (lin_x, lin_y, lin_z, ang_x, ang_y, ang_z) = handler.compute_twist();
        assert_eq!(lin_x, 0.0);
        assert_eq!(lin_y, 0.0);
        assert_eq!(lin_z, 0.0);
        assert_eq!(ang_x, 0.0);
        assert_eq!(ang_y, 0.0);
        assert_eq!(ang_z, 0.0);
    }
}
