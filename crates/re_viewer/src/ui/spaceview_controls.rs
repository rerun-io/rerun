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

// TODO(andreas: Move to egui)
/// Whether a set of modifiers contains another set of modifiers.
///
/// Handles the special case of [`egui::Modifiers::command`] vs [`egui::Modifiers::mac_cmd`].
pub fn modifier_contains(modifiers: egui::Modifiers, contains_query: egui::Modifiers) -> bool {
    if contains_query == egui::Modifiers::default() {
        return true;
    }

    let egui::Modifiers {
        alt,
        ctrl,
        shift,
        mac_cmd,
        command,
    } = modifiers;

    if alt && contains_query.alt {
        return modifier_contains(
            modifiers,
            egui::Modifiers {
                alt: false,
                ..contains_query
            },
        );
    }
    if shift && contains_query.shift {
        return modifier_contains(
            modifiers,
            egui::Modifiers {
                shift: false,
                ..contains_query
            },
        );
    }

    if (ctrl || command) && (contains_query.ctrl || contains_query.command) {
        return modifier_contains(
            modifiers,
            egui::Modifiers {
                command: false,
                ctrl: false,
                ..contains_query
            },
        );
    }
    if (mac_cmd || command) && (contains_query.mac_cmd || contains_query.command) {
        return modifier_contains(
            modifiers,
            egui::Modifiers {
                mac_cmd: false,
                ctrl: false,
                ..contains_query
            },
        );
    }

    false
}

#[cfg(test)]
mod test {
    use crate::ui::spaceview_controls::modifier_contains;
    use egui::Modifiers;

    #[test]
    fn test_modifier_contains() {
        assert!(modifier_contains(
            Modifiers::default(),
            Modifiers::default()
        ));
        assert!(modifier_contains(Modifiers::CTRL, Modifiers::default()));
        assert!(modifier_contains(Modifiers::CTRL, Modifiers::CTRL));
        assert!(modifier_contains(Modifiers::CTRL, Modifiers::COMMAND));
        assert!(modifier_contains(Modifiers::MAC_CMD, Modifiers::COMMAND));
        assert!(modifier_contains(Modifiers::COMMAND, Modifiers::MAC_CMD));
        assert!(modifier_contains(Modifiers::COMMAND, Modifiers::CTRL));
        assert!(!modifier_contains(
            Modifiers::ALT | Modifiers::CTRL,
            Modifiers::SHIFT,
        ));
        assert!(modifier_contains(
            Modifiers::CTRL | Modifiers::SHIFT,
            Modifiers::CTRL,
        ));
        assert!(!modifier_contains(
            Modifiers::CTRL,
            Modifiers::CTRL | Modifiers::SHIFT,
        ));
    }
}
