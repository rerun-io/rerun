use re_viewer_context::{ViewId, ViewerContext};

pub(crate) struct WebViewInstance {
    view_id: ViewId,
    pub(crate) url: String,
    #[cfg(debug_assertions)]
    fake_backend: Option<crate::testing::FakeWebViewBackend>,
    #[cfg(all(not(target_arch = "wasm32"), feature = "native_webview"))]
    has_native_webview: bool,
}

impl std::fmt::Debug for WebViewInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebViewInstance")
            .field("view_id", &self.view_id)
            .field("url", &self.url)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WebViewSession {
    SharedDefault,
}

impl WebViewSession {
    pub(crate) const fn shared_default() -> Self {
        Self::SharedDefault
    }

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::SharedDefault => "shared-default",
        }
    }
}

impl WebViewInstance {
    #[cfg(debug_assertions)]
    pub(crate) fn new_fake(
        view_id: ViewId,
        url: String,
        fake_backend: crate::testing::FakeWebViewBackend,
    ) -> Self {
        Self {
            view_id,
            url,
            fake_backend: Some(fake_backend),
            #[cfg(all(not(target_arch = "wasm32"), feature = "native_webview"))]
            has_native_webview: false,
        }
    }

    #[cfg(all(not(target_arch = "wasm32"), feature = "native_webview"))]
    pub(crate) fn new_native(view_id: ViewId, url: String) -> Self {
        Self {
            view_id,
            url,
            #[cfg(debug_assertions)]
            fake_backend: None,
            has_native_webview: true,
        }
    }

    pub(crate) fn set_bounds(&self, view_id: ViewId, bounds: WebViewBounds) {
        #[cfg(debug_assertions)]
        if let Some(fake_backend) = &self.fake_backend {
            fake_backend.record_bounds_update(view_id, bounds);
        }

        #[cfg(all(not(target_arch = "wasm32"), feature = "native_webview"))]
        if self.has_native_webview {
            crate::native_backend::set_bounds(self.view_id, bounds);
        }
    }

    pub(crate) fn set_visible(&self, visible: bool) {
        #[cfg(not(all(not(target_arch = "wasm32"), feature = "native_webview")))]
        let _ = self;
        let _ = visible;

        #[cfg(all(not(target_arch = "wasm32"), feature = "native_webview"))]
        if self.has_native_webview {
            crate::native_backend::set_visible(self.view_id, visible);
        }
    }

    pub(crate) fn go_back(&self) {
        #[cfg(debug_assertions)]
        if let Some(fake_backend) = &self.fake_backend {
            fake_backend.record_navigation_command(self.view_id, FakeNavigationCommand::Back);
        }

        #[cfg(all(not(target_arch = "wasm32"), feature = "native_webview"))]
        if self.has_native_webview {
            crate::native_backend::go_back(self.view_id);
        }
    }

    pub(crate) fn go_forward(&self) {
        #[cfg(debug_assertions)]
        if let Some(fake_backend) = &self.fake_backend {
            fake_backend.record_navigation_command(self.view_id, FakeNavigationCommand::Forward);
        }

        #[cfg(all(not(target_arch = "wasm32"), feature = "native_webview"))]
        if self.has_native_webview {
            crate::native_backend::go_forward(self.view_id);
        }
    }

    pub(crate) fn reload(&self) {
        #[cfg(debug_assertions)]
        if let Some(fake_backend) = &self.fake_backend {
            fake_backend.record_navigation_command(self.view_id, FakeNavigationCommand::Reload);
        }

        #[cfg(all(not(target_arch = "wasm32"), feature = "native_webview"))]
        if self.has_native_webview {
            crate::native_backend::reload(self.view_id);
        }
    }

    pub(crate) fn navigate_to(&self, url: &str) {
        #[cfg(debug_assertions)]
        if let Some(fake_backend) = &self.fake_backend {
            fake_backend.record_navigation_command(
                self.view_id,
                FakeNavigationCommand::NavigateTo(url.to_owned()),
            );
        }

        #[cfg(all(not(target_arch = "wasm32"), feature = "native_webview"))]
        if self.has_native_webview {
            crate::native_backend::navigate_to(self.view_id, url);
        }
    }
}

#[cfg(debug_assertions)]
use crate::testing::FakeNavigationCommand;

impl Drop for WebViewInstance {
    fn drop(&mut self) {
        #[cfg(debug_assertions)]
        if let Some(fake_backend) = &self.fake_backend {
            fake_backend.record_destroyed_instance(self.view_id, &self.url);
        }

        #[cfg(all(not(target_arch = "wasm32"), feature = "native_webview"))]
        if self.has_native_webview {
            crate::native_backend::destroy(self.view_id);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct WebViewBounds {
    pub(crate) min: [f32; 2],
    pub(crate) size: [f32; 2],
}

impl WebViewBounds {
    pub(crate) fn from_egui_rect(rect: egui::Rect, pixels_per_point: f32) -> Self {
        let min = rect.min * pixels_per_point;
        let size = rect.size() * pixels_per_point;
        Self {
            min: [min.x.round(), min.y.round()],
            size: [size.x.round(), size.y.round()],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WebViewBackendError {
    CreationFailed(String),
}

impl std::fmt::Display for WebViewBackendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CreationFailed(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for WebViewBackendError {}

pub(crate) trait WebViewBackend {
    fn create(
        &self,
        ctx: &ViewerContext<'_>,
        view_id: ViewId,
        url: &str,
        bounds: WebViewBounds,
        session: WebViewSession,
    ) -> Result<WebViewInstance, WebViewBackendError>;
}

pub(crate) fn create_webview(
    ctx: &ViewerContext<'_>,
    view_id: ViewId,
    url: &str,
    bounds: WebViewBounds,
) -> Result<Option<WebViewInstance>, WebViewBackendError> {
    #[cfg(debug_assertions)]
    if let Some(fake_backend) = crate::testing::installed_backend() {
        return fake_backend
            .create(ctx, view_id, url, bounds, WebViewSession::shared_default())
            .map(Some);
    }

    #[cfg(all(not(target_arch = "wasm32"), feature = "native_webview"))]
    if crate::native_backend::has_native_parent_window() {
        let native_webview = crate::native_backend::NativeWebViewBackend
            .create_child(url, bounds)
            .map_err(|err| WebViewBackendError::CreationFailed(err.to_string()))?;
        crate::native_backend::insert(view_id, native_webview);
        return Ok(Some(WebViewInstance::new_native(view_id, url.to_owned())));
    }

    let _ = (ctx, view_id, url, bounds);
    Ok(None)
}
