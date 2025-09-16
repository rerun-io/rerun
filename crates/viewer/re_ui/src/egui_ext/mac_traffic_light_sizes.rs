use objc2_app_kit::{NSView, NSWindow, NSWindowButton};
use objc2_foundation::MainThreadMarker;
use raw_window_handle::HasWindowHandle as _;
use raw_window_handle::RawWindowHandle;

#[derive(Debug)]
pub struct WindowChromeMetrics {
    /// Height of the title bar
    pub title_bar_height: f32,

    /// Size of the "traffic lights" (red/yellow/green close/minimize/maximize buttons)
    pub traffic_lights_width: f32, // TODO: combine into an egui::Vec2
    pub traffic_lights_height: f32, // TODO: combine into an egui::Vec2
}

impl WindowChromeMetrics {
    pub fn from_window_handle(window_handle: &RawWindowHandle) -> Option<Self> {
        window_chrome_metrics(window_handle)
    }
}

pub fn window_chrome_metrics(window_handle: &RawWindowHandle) -> Option<WindowChromeMetrics> {
    let RawWindowHandle::AppKit(appkit_handle) = window_handle else {
        return None;
    };

    let mtm = MainThreadMarker::new()?;

    let ns_view_ptr = appkit_handle.ns_view.as_ptr().cast::<NSView>();
    let ns_view = unsafe { ns_view_ptr.as_ref()? };
    let ns_window = ns_view.window()?;

    // For full-size content windows, we need to calculate differently
    let title_bar_height = actual_title_bar_height(&ns_window, mtm)?;
    let (traffic_lights_width, traffic_lights_height) = traffic_lights_metrics(&ns_window, mtm)?;

    Some(WindowChromeMetrics {
        title_bar_height,
        traffic_lights_width,
        traffic_lights_height,
    })
}

fn actual_title_bar_height(ns_window: &NSWindow, mtm: MainThreadMarker) -> Option<f32> {
    // Get the close button and use its position to determine title bar bounds
    let close_button = ns_window.standardWindowButton(NSWindowButton::NSWindowCloseButton)?;
    let close_frame = close_button.frame();

    // The title bar height is approximately the button center Y * 2
    // Or we can use the button's Y position + button height + bottom margin
    let button_bottom = close_frame.origin.y;
    let button_height = close_frame.size.height;
    let estimated_top_margin = button_bottom; // Margin above button ≈ margin below button

    let title_bar_height = button_bottom + button_height + estimated_top_margin;

    Some(title_bar_height as f32)
}

fn traffic_lights_metrics(ns_window: &NSWindow, mtm: MainThreadMarker) -> Option<(f32, f32)> {
    let close_button = ns_window.standardWindowButton(NSWindowButton::NSWindowCloseButton)?;
    let zoom_button = ns_window.standardWindowButton(NSWindowButton::NSWindowZoomButton)?;

    let close_frame = close_button.frame();
    let zoom_frame = zoom_button.frame();

    // Include the left margin (from window edge to close button)
    let left_margin = close_frame.origin.x;

    // Include right margin after zoom button
    let right_margin = 8.0; // Typical right margin

    // Total width from window edge to end of traffic light area
    let total_width_from_edge = zoom_frame.origin.x + zoom_frame.size.width + right_margin;

    // Or just the traffic lights themselves plus margins:
    let traffic_lights_width = total_width_from_edge;

    // Height includes the button plus top and bottom margins
    let button_height = close_frame.size.height;
    let top_margin = close_frame.origin.y;
    let bottom_margin = top_margin; // Usually symmetric
    let traffic_lights_height = button_height + top_margin + bottom_margin;

    Some((traffic_lights_width as f32, traffic_lights_height as f32))
}

pub fn traffic_lights_width(window_handle: &RawWindowHandle) -> Option<f32> {
    let RawWindowHandle::AppKit(appkit_handle) = window_handle else {
        return None;
    };

    let mtm = MainThreadMarker::new()?;

    let ns_view_ptr = appkit_handle.ns_view.as_ptr().cast::<NSView>();
    let ns_view = unsafe { ns_view_ptr.as_ref()? };
    let ns_window = ns_view.window()?;

    let (width, _) = traffic_lights_metrics(&ns_window, mtm)?;
    Some(width)
}

// More precise version that tries multiple approaches
pub fn title_bar_height_precise(window_handle: &RawWindowHandle) -> Option<f32> {
    let RawWindowHandle::AppKit(appkit_handle) = window_handle else {
        return None;
    };

    let mtm = MainThreadMarker::new()?;

    let ns_view_ptr = appkit_handle.ns_view.as_ptr().cast::<NSView>();
    let ns_view = unsafe { ns_view_ptr.as_ref()? };
    let ns_window = ns_view.window()?;

    // Try the traditional method first
    let window_frame = ns_window.frame();
    let content_rect = unsafe {
        NSWindow::contentRectForFrameRect_styleMask(window_frame, ns_window.styleMask(), mtm)
    };

    let traditional_height = window_frame.size.height - content_rect.size.height;

    if traditional_height > 0.0 {
        // Traditional window with separate title bar
        Some(traditional_height as f32)
    } else {
        // Full-size content window, calculate from traffic lights
        actual_title_bar_height(&ns_window, mtm)
    }
}
