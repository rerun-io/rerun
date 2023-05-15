/// Modifier to press for scroll to zoom.
pub const ZOOM_SCROLL_MODIFIER: egui::Modifiers = egui::Modifiers::COMMAND;

/// Modifier to press for scroll to pan horizontally.
pub const HORIZONTAL_SCROLL_MODIFIER: egui::Modifiers = egui::Modifiers::SHIFT;

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

/// Which modifier slows down the 3D camera movement.
pub const SLOW_DOWN_3D_MODIFIER: egui::Modifiers = egui::Modifiers::CTRL;

/// Key to restore the camera.
pub const TRACKED_CAMERA_RESTORE_KEY: egui::Key = egui::Key::Escape;

/// Description text for which action resets a space view.
pub const RESET_VIEW_BUTTON_TEXT: &str = "double click";
