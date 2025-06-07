//! For zooming and panning we use the modifiers in the default [`egui::InputOptions`].

use egui::{Modifiers, os::OperatingSystem};

/// Which mouse button to drag for panning a 2D view.
pub const DRAG_PAN2D_BUTTON: egui::PointerButton = egui::PointerButton::Primary;

/// Rectangles drawn with this mouse button zoom in 2D views.
pub const SELECTION_RECT_ZOOM_BUTTON: egui::PointerButton = egui::PointerButton::Secondary;

/// Clicking this button moves the timeline to where the cursor is.
pub const MOVE_TIME_CURSOR_BUTTON: egui::PointerButton = egui::PointerButton::Secondary;

/// Which mouse button to drag for panning a 2D view.
pub const DRAG_PAN3D_BUTTON: egui::PointerButton = egui::PointerButton::Secondary;

/// Which mouse button to drag for rotating a 3D view.
pub const ROTATE3D_BUTTON: egui::PointerButton = egui::PointerButton::Primary;

/// Which mouse button rolls the camera.
pub const ROLL_MOUSE: egui::PointerButton = egui::PointerButton::Middle;

/// Which mouse button rolls the camera if the roll modifier is pressed.
pub const ROLL_MOUSE_ALT: egui::PointerButton = egui::PointerButton::Primary;

/// See [`ROLL_MOUSE_ALT`].
pub const ROLL_MOUSE_MODIFIER: egui::Modifiers = egui::Modifiers::ALT;

/// Which modifier speeds up the 3D camera movement.
pub const SPEED_UP_3D_MODIFIER: egui::Modifiers = egui::Modifiers::SHIFT;

/// Key to restore the camera.
pub const TRACKED_OBJECT_RESTORE_KEY: egui::Key = egui::Key::Escape;

pub struct RuntimeModifiers {}

impl RuntimeModifiers {
    pub fn slow_down(os: &OperatingSystem) -> Modifiers {
        match os {
            egui::os::OperatingSystem::Mac => egui::Modifiers::CTRL,
            _ => egui::Modifiers::ALT,
        }
    }
}
