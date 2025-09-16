use egui::Vec2;
use objc2_app_kit::{NSView, NSWindow, NSWindowButton};
use raw_window_handle::{AppKitWindowHandle, RawWindowHandle};

#[derive(Debug)]
pub struct WindowChromeMetrics {
    /// Size of the "traffic lights" (red/yellow/green close/minimize/maximize buttons)
    pub traffic_lights_size: Vec2,
}

impl WindowChromeMetrics {
    pub fn from_window_handle(window_handle: &RawWindowHandle) -> Option<Self> {
        window_chrome_metrics(window_handle)
    }
}

fn window_chrome_metrics(window_handle: &RawWindowHandle) -> Option<WindowChromeMetrics> {
    let RawWindowHandle::AppKit(appkit_handle) = window_handle else {
        return None;
    };

    let ns_view = ns_view_from_handle(appkit_handle)?;
    let ns_window = ns_view.window()?;

    Some(WindowChromeMetrics {
        traffic_lights_size: traffic_lights_metrics(&ns_window)?,
    })
}

fn traffic_lights_metrics(ns_window: &NSWindow) -> Option<Vec2> {
    let close_button = ns_window.standardWindowButton(NSWindowButton::CloseButton)?;
    let zoom_button = ns_window.standardWindowButton(NSWindowButton::ZoomButton)?;

    let close_frame = close_button.frame();
    let zoom_frame = zoom_button.frame();

    // Include the left margin (from window edge to close button)
    let left_margin = close_frame.origin.x;

    // Include right margin after zoom button
    let right_margin = left_margin; // for symmetry

    // Total width from window edge to end of traffic light area
    let total_width_from_edge = zoom_frame.origin.x + zoom_frame.size.width + right_margin;

    // Or just the traffic lights themselves plus margins:
    let traffic_lights_width = total_width_from_edge;

    // Height includes the button plus top and bottom margins
    let button_height = close_frame.size.height;
    let top_margin = close_frame.origin.y;
    let bottom_margin = top_margin; // Usually symmetric
    let traffic_lights_height = button_height + top_margin + bottom_margin;

    Some(Vec2::new(
        traffic_lights_width as f32,
        traffic_lights_height as f32,
    ))
}

fn ns_view_from_handle(handle: &AppKitWindowHandle) -> Option<&NSView> {
    let ns_view_ptr = handle.ns_view.as_ptr().cast::<NSView>();

    // Validate the pointer is non-null
    if ns_view_ptr.is_null() {
        None
    } else {
        // SAFETY:
        // - We've verified the pointer is non-null
        // - The pointer comes from the windowing system, so it should be valid
        // - NSView pointers from AppKit are expected to remain valid for the window lifetime
        #[allow(unsafe_code)]
        unsafe {
            ns_view_ptr.as_ref()
        }
    }
}
