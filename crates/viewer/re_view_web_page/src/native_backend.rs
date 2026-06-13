//! Native `wry` backend for the Web Page View.
//!
//! This module is intentionally a thin boundary: callers provide the native parent window handle,
//! and UI/lifecycle code owns when this is called and how failures are surfaced.

use re_viewer_context::ViewId;

use crate::backend::WebViewBounds;

thread_local! {
    static NATIVE_WEBVIEWS: std::cell::RefCell<ahash::HashMap<ViewId, NativeWebView>> =
        std::cell::RefCell::new(ahash::HashMap::default());
    static VISIBLE_THIS_FRAME: std::cell::RefCell<ahash::HashSet<ViewId>> =
        std::cell::RefCell::new(ahash::HashSet::default());
    static OVERLAY_CLIP_BOUNDS: std::cell::Cell<Option<WebViewBounds>> =
        const { std::cell::Cell::new(None) };
}

scoped_tls::scoped_thread_local!(static NATIVE_PARENT_WINDOW: eframe::Frame);

#[derive(Debug, Default)]
pub struct NativeWebViewBackend;

pub struct NativeWebView {
    webview: wry::WebView,
    visible: bool,
    bounds: WebViewBounds,
}

#[derive(Debug)]
pub enum NativeWebViewError {
    MissingParentWindow,
    Wry(wry::Error),
    Clip(String),
}

impl std::fmt::Display for NativeWebViewError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingParentWindow => f.write_str("missing parent native window"),
            Self::Wry(err) => write!(f, "failed to create native webview: {err}"),
            Self::Clip(err) => write!(f, "failed to clip native webview: {err}"),
        }
    }
}

impl std::error::Error for NativeWebViewError {}

impl From<wry::Error> for NativeWebViewError {
    fn from(err: wry::Error) -> Self {
        Self::Wry(err)
    }
}

impl NativeWebViewBackend {
    pub fn create_without_parent_for_smoke_test(
        &self,
        _url: &str,
    ) -> Result<NativeWebView, NativeWebViewError> {
        let _ = self;
        Err(NativeWebViewError::MissingParentWindow)
    }

    pub(crate) fn create_child(
        &self,
        url: &str,
        bounds: WebViewBounds,
    ) -> Result<NativeWebView, NativeWebViewError> {
        let _ = self;
        NATIVE_PARENT_WINDOW.with(|parent_window| {
            let webview = platform::create_child(parent_window, url, bounds)?;
            Ok(NativeWebView {
                webview,
                visible: true,
                bounds,
            })
        })
    }
}

pub fn set_overlay_clip_rect(rect: egui::Rect, pixels_per_point: f32) {
    OVERLAY_CLIP_BOUNDS.with(|overlay_clip_bounds| {
        overlay_clip_bounds.set(Some(WebViewBounds::from_egui_rect(rect, pixels_per_point)));
    });
}

pub fn with_native_parent_window<R>(frame: &eframe::Frame, f: impl FnOnce() -> R) -> R {
    NATIVE_PARENT_WINDOW.set(frame, || {
        begin_frame();
        let result = f();
        hide_webviews_not_drawn_this_frame();
        platform::pump_events();
        result
    })
}

pub(crate) fn has_native_parent_window() -> bool {
    NATIVE_PARENT_WINDOW.is_set()
}

impl NativeWebView {
    pub(crate) fn set_bounds(&mut self, bounds: WebViewBounds) -> Result<(), NativeWebViewError> {
        self.webview
            .set_bounds(bounds.into())
            .map_err(NativeWebViewError::from)?;
        self.bounds = bounds;
        self.apply_overlay_clip()
    }

    fn apply_overlay_clip(&self) -> Result<(), NativeWebViewError> {
        let overlay_clip_bounds =
            OVERLAY_CLIP_BOUNDS.with(|overlay_clip_bounds| overlay_clip_bounds.get());
        platform::apply_overlay_clip(&self.webview, self.bounds, overlay_clip_bounds)
    }

    pub(crate) fn set_visible(&mut self, visible: bool) -> Result<(), NativeWebViewError> {
        if self.visible == visible {
            return Ok(());
        }

        self.webview
            .set_visible(visible)
            .map_err(NativeWebViewError::from)?;
        self.visible = visible;
        Ok(())
    }

    pub(crate) fn go_back(&self) -> Result<(), NativeWebViewError> {
        self.webview
            .evaluate_script("history.back()")
            .map_err(Into::into)
    }

    pub(crate) fn go_forward(&self) -> Result<(), NativeWebViewError> {
        self.webview
            .evaluate_script("history.forward()")
            .map_err(Into::into)
    }

    pub(crate) fn reload(&self) -> Result<(), NativeWebViewError> {
        self.webview.reload().map_err(Into::into)
    }

    pub(crate) fn navigate_to(&self, url: &str) -> Result<(), NativeWebViewError> {
        self.webview.load_url(url).map_err(Into::into)
    }
}

pub(crate) fn insert(view_id: ViewId, webview: NativeWebView) {
    NATIVE_WEBVIEWS.with_borrow_mut(|webviews| {
        webviews.insert(view_id, webview);
    });
}

pub(crate) fn destroy(view_id: ViewId) {
    NATIVE_WEBVIEWS.with_borrow_mut(|webviews| {
        webviews.remove(&view_id);
    });
}

pub(crate) fn set_bounds(view_id: ViewId, bounds: WebViewBounds) {
    with_webview(view_id, |webview| webview.set_bounds(bounds));
}

pub(crate) fn set_visible(view_id: ViewId, visible: bool) {
    if visible {
        VISIBLE_THIS_FRAME.with_borrow_mut(|visible_this_frame| {
            visible_this_frame.insert(view_id);
        });
    }

    with_webview(view_id, |webview| webview.set_visible(visible));
}

pub(crate) fn go_back(view_id: ViewId) {
    with_webview(view_id, |webview| webview.go_back());
}

pub(crate) fn go_forward(view_id: ViewId) {
    with_webview(view_id, |webview| webview.go_forward());
}

pub(crate) fn reload(view_id: ViewId) {
    with_webview(view_id, |webview| webview.reload());
}

pub(crate) fn navigate_to(view_id: ViewId, url: &str) {
    with_webview(view_id, |webview| webview.navigate_to(url));
}

fn with_webview(
    view_id: ViewId,
    f: impl FnOnce(&mut NativeWebView) -> Result<(), NativeWebViewError>,
) {
    NATIVE_WEBVIEWS.with_borrow_mut(|webviews| {
        if let Some(webview) = webviews.get_mut(&view_id) {
            match f(webview) {
                Ok(()) | Err(_) => {}
            }
        }
    });
}

fn begin_frame() {
    VISIBLE_THIS_FRAME.with_borrow_mut(|visible_this_frame| {
        visible_this_frame.clear();
    });
}

fn hide_webviews_not_drawn_this_frame() {
    VISIBLE_THIS_FRAME.with_borrow(|visible_this_frame| {
        NATIVE_WEBVIEWS.with_borrow_mut(|webviews| {
            for (view_id, webview) in webviews {
                if !visible_this_frame.contains(view_id) {
                    match webview.set_visible(false) {
                        Ok(()) | Err(_) => {}
                    }
                }
            }
        });
    });
}

impl From<WebViewBounds> for wry::Rect {
    fn from(bounds: WebViewBounds) -> Self {
        let min_x = bounds.min[0].max(0.0).round() as u32;
        let min_y = bounds.min[1].max(0.0).round() as u32;
        let width = bounds.size[0].max(1.0).round() as u32;
        let height = bounds.size[1].max(1.0).round() as u32;

        Self {
            position: wry::dpi::LogicalPosition::new(min_x, min_y).into(),
            size: wry::dpi::LogicalSize::new(width, height).into(),
        }
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use gtk::prelude::WidgetExt;
    use raw_window_handle::HasWindowHandle;
    use wry::WebViewExtUnix as _;

    use super::NativeWebViewError;

    pub(super) fn pump_events() {
        if gtk::is_initialized_main_thread() {
            // Keep WebKitGTK responsive without letting its event queue monopolize an egui frame.
            // Further events will be drained on subsequent frames.
            for _ in 0..16 {
                if !gtk::events_pending() {
                    break;
                }
                gtk::main_iteration_do(false);
            }
        }
    }

    pub(super) fn create_child<W: HasWindowHandle>(
        parent_window: &W,
        url: &str,
        bounds: crate::backend::WebViewBounds,
    ) -> wry::Result<wry::WebView> {
        gtk::init()?;

        // `build_as_child` is the direct child-window path and is X11-only on Linux.
        // Wayland support requires the GTK container path (`WebViewBuilderExtUnix::build_gtk`),
        // which is intentionally hidden behind this platform module for a later host/widget bridge.
        wry::WebViewBuilder::new()
            .with_bounds(bounds.into())
            .with_url(url)
            .build_as_child(parent_window)
    }

    pub(super) fn apply_overlay_clip(
        webview: &wry::WebView,
        webview_bounds: crate::backend::WebViewBounds,
        overlay_bounds: Option<crate::backend::WebViewBounds>,
    ) -> Result<(), NativeWebViewError> {
        let width = webview_bounds.size[0].max(1.0).round() as i32;
        let height = webview_bounds.size[1].max(1.0).round() as i32;

        let region = gtk::cairo::Region::create_rectangle(&gtk::cairo::RectangleInt::new(
            0, 0, width, height,
        ));

        if let Some(overlay_bounds) = overlay_bounds {
            let left = webview_bounds.min[0].max(overlay_bounds.min[0]);
            let top = webview_bounds.min[1].max(overlay_bounds.min[1]);
            let right = (webview_bounds.min[0] + webview_bounds.size[0])
                .min(overlay_bounds.min[0] + overlay_bounds.size[0]);
            let bottom = (webview_bounds.min[1] + webview_bounds.size[1])
                .min(overlay_bounds.min[1] + overlay_bounds.size[1]);

            if right > left && bottom > top {
                region
                    .subtract_rectangle(&gtk::cairo::RectangleInt::new(
                        (left - webview_bounds.min[0]).round() as i32,
                        (top - webview_bounds.min[1]).round() as i32,
                        (right - left).round().max(1.0) as i32,
                        (bottom - top).round().max(1.0) as i32,
                    ))
                    .map_err(|err| NativeWebViewError::Clip(err.to_string()))?;
            }
        }

        if let Some(window) = webview.webview().window() {
            window.shape_combine_region(Some(&region), 0, 0);
            window.input_shape_combine_region(&region, 0, 0);
        }

        Ok(())
    }
}

#[cfg(not(target_os = "linux"))]
mod platform {
    use raw_window_handle::HasWindowHandle;

    pub(super) fn pump_events() {}

    pub(super) fn create_child<W: HasWindowHandle>(
        parent_window: &W,
        url: &str,
        bounds: crate::backend::WebViewBounds,
    ) -> wry::Result<wry::WebView> {
        wry::WebViewBuilder::new()
            .with_bounds(bounds.into())
            .with_url(url)
            .build_as_child(parent_window)
    }

    pub(super) fn apply_overlay_clip(
        _webview: &wry::WebView,
        _webview_bounds: crate::backend::WebViewBounds,
        _overlay_bounds: Option<crate::backend::WebViewBounds>,
    ) -> Result<(), super::NativeWebViewError> {
        Ok(())
    }
}
