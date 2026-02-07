//! For zooming and panning we use the modifiers in the default [`egui::InputOptions`].

use egui::os::OperatingSystem;
use egui::{Key, KeyboardShortcut, Modifiers, PointerButton};

/// Which mouse button to drag for panning a 2D view.
pub const DRAG_PAN2D_BUTTON: PointerButton = PointerButton::Primary;

/// Rectangles drawn with this mouse button zoom in 2D views.
pub const SELECTION_RECT_ZOOM_BUTTON: PointerButton = PointerButton::Secondary;

/// Clicking this button moves the timeline to where the cursor is.
pub const MOVE_TIME_CURSOR_BUTTON: PointerButton = PointerButton::Secondary;

/// Which mouse button to drag for panning a 3D view.
pub const DRAG_PAN3D_BUTTON: PointerButton = PointerButton::Secondary;

/// Which mouse button to drag for rotating a 3D view.
pub const ROTATE3D_BUTTON: PointerButton = PointerButton::Primary;

/// Which mouse button rolls the camera.
pub const ROLL_MOUSE: PointerButton = PointerButton::Middle;

/// Which mouse button rolls the camera if the roll modifier is pressed.
pub const ROLL_MOUSE_ALT: PointerButton = PointerButton::Primary;

/// See [`ROLL_MOUSE_ALT`].
pub const ROLL_MOUSE_MODIFIER: Modifiers = Modifiers::ALT;

/// Which modifier speeds up the 3D camera movement.
pub const SPEED_UP_3D_MODIFIER: Modifiers = Modifiers::SHIFT;

/// Key to restore the camera.
pub const TRACKED_OBJECT_RESTORE_KEY: Key = Key::Escape;

/// Toggle the currently selected view to be maximized or not.
// NOTE: we use CTRL and not COMMAND, because ⌘+M minimizes the whole window on macOS.
pub const TOGGLE_MAXIMIZE_VIEW: KeyboardShortcut = KeyboardShortcut::new(Modifiers::CTRL, Key::M);

pub struct RuntimeModifiers {}

impl RuntimeModifiers {
    pub fn slow_down(os: &OperatingSystem) -> Modifiers {
        match os {
            egui::os::OperatingSystem::Mac => Modifiers::CTRL,
            _ => Modifiers::ALT,
        }
    }
}
