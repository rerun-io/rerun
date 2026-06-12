use re_viewer_context::{ViewId, ViewerContext};

use crate::backend::{WebViewBounds, WebViewInstance, create_webview};

#[derive(Default)]
pub(crate) struct WebViewLifecycle {
    webview: Option<WebViewInstance>,
    last_bounds: Option<WebViewBounds>,
}

impl WebViewLifecycle {
    pub(crate) fn ensure_webview(
        &mut self,
        ctx: &ViewerContext<'_>,
        view_id: ViewId,
        url: &str,
        bounds: WebViewBounds,
    ) -> WebViewLifecycleStatus {
        if self
            .webview
            .as_ref()
            .is_none_or(|webview| webview.url != url)
        {
            match create_webview(ctx, view_id, url, bounds) {
                Ok(Some(webview)) => {
                    self.webview = Some(webview);
                    self.last_bounds = None;
                }
                Ok(None) => {
                    self.webview = None;
                    return WebViewLifecycleStatus::Unavailable;
                }
                Err(err) => {
                    self.webview = None;
                    return WebViewLifecycleStatus::CreationFailed(err.to_string());
                }
            }
        }

        WebViewLifecycleStatus::Ready
    }

    pub(crate) fn update_bounds(&mut self, view_id: ViewId, bounds: WebViewBounds) {
        if self.last_bounds == Some(bounds) {
            return;
        }

        if let Some(webview) = &mut self.webview {
            webview.set_bounds(view_id, bounds);
        }

        self.last_bounds = Some(bounds);
    }

    pub(crate) fn go_back(&mut self) {
        if let Some(webview) = &mut self.webview {
            webview.go_back();
        }
    }

    pub(crate) fn go_forward(&mut self) {
        if let Some(webview) = &mut self.webview {
            webview.go_forward();
        }
    }

    pub(crate) fn reload(&mut self) {
        if let Some(webview) = &mut self.webview {
            webview.reload();
        }
    }

    pub(crate) fn navigate_to(&mut self, url: &str) {
        if let Some(webview) = &mut self.webview {
            webview.navigate_to(url);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WebViewLifecycleStatus {
    Ready,
    Unavailable,
    CreationFailed(String),
}
