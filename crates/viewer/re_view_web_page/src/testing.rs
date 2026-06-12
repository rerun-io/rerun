//! Test support for the Web Page View backend seam.

use std::{cell::RefCell, collections::HashMap, sync::Arc};

use parking_lot::Mutex;
use re_viewer_context::{ViewId, ViewerContext};

use crate::backend::{
    WebViewBackend, WebViewBackendError, WebViewBounds, WebViewInstance, WebViewSession,
};

thread_local! {
    static INSTALLED_BACKEND: RefCell<Option<FakeWebViewBackend>> = const { RefCell::new(None) };
}

#[derive(Debug, Clone, Default)]
pub struct FakeWebViewBackend {
    state: Arc<Mutex<FakeWebViewBackendState>>,
}

#[derive(Debug, Default)]
struct FakeWebViewBackendState {
    creation_error: Option<String>,
    created_instances: Vec<FakeCreatedWebView>,
    bounds_updates: Vec<FakeBoundsUpdate>,
    destroyed_instances: Vec<FakeDestroyedWebView>,
    navigation_commands: Vec<FakeNavigationCommandRequest>,
    current_urls: HashMap<ViewId, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FakeCreatedWebView {
    pub view_id: ViewId,
    pub url: String,
    pub session: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FakeDestroyedWebView {
    pub view_id: ViewId,
    pub url: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FakeWebViewBounds {
    pub min: [f32; 2],
    pub size: [f32; 2],
}

impl From<WebViewBounds> for FakeWebViewBounds {
    fn from(bounds: WebViewBounds) -> Self {
        Self {
            min: bounds.min,
            size: bounds.size,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FakeBoundsUpdate {
    pub view_id: ViewId,
    pub bounds: FakeWebViewBounds,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FakeNavigationCommand {
    Back,
    Forward,
    Reload,
    NavigateTo(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FakeNavigationCommandRequest {
    pub view_id: ViewId,
    pub command: FakeNavigationCommand,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FakeNavigationRequest {
    pub view_id: ViewId,
    pub url: String,
}

impl FakeWebViewBackend {
    pub fn failing(message: impl Into<String>) -> Self {
        Self {
            state: Arc::new(Mutex::new(FakeWebViewBackendState {
                creation_error: Some(message.into()),
                ..Default::default()
            })),
        }
    }

    pub fn install(&self) -> FakeWebViewBackendGuard {
        INSTALLED_BACKEND.with(|installed_backend| {
            let previous_backend = installed_backend.replace(Some(self.clone()));
            assert!(
                previous_backend.is_none(),
                "a fake Web Page View backend is already installed on this thread"
            );
        });

        FakeWebViewBackendGuard {}
    }

    pub fn created_instance_count(&self) -> usize {
        self.state.lock().created_instances.len()
    }

    pub fn created_urls(&self) -> Vec<String> {
        self.state
            .lock()
            .created_instances
            .iter()
            .map(|created_instance| created_instance.url.clone())
            .collect()
    }

    pub fn created_instances(&self) -> Vec<FakeCreatedWebView> {
        self.state.lock().created_instances.clone()
    }

    pub fn bounds_updates(&self) -> Vec<FakeBoundsUpdate> {
        self.state.lock().bounds_updates.clone()
    }

    pub fn destroyed_instance_count(&self) -> usize {
        self.state.lock().destroyed_instances.len()
    }

    pub fn destroyed_instances(&self) -> Vec<FakeDestroyedWebView> {
        self.state.lock().destroyed_instances.clone()
    }

    pub fn simulate_navigation(&self, view_id: ViewId, url: &str) {
        self.state
            .lock()
            .current_urls
            .insert(view_id, url.to_owned());
    }

    pub fn navigation_requests(&self) -> Vec<FakeNavigationRequest> {
        self.state
            .lock()
            .navigation_commands
            .iter()
            .filter_map(|request| match &request.command {
                FakeNavigationCommand::NavigateTo(url) => Some(FakeNavigationRequest {
                    view_id: request.view_id,
                    url: url.clone(),
                }),
                FakeNavigationCommand::Back
                | FakeNavigationCommand::Forward
                | FakeNavigationCommand::Reload => None,
            })
            .collect()
    }

    pub(crate) fn record_bounds_update(&self, view_id: ViewId, bounds: WebViewBounds) {
        self.state.lock().bounds_updates.push(FakeBoundsUpdate {
            view_id,
            bounds: bounds.into(),
        });
    }

    pub(crate) fn record_destroyed_instance(&self, view_id: ViewId, url: &str) {
        self.state
            .lock()
            .destroyed_instances
            .push(FakeDestroyedWebView {
                view_id,
                url: url.to_owned(),
            });
    }

    pub(crate) fn record_navigation_command(
        &self,
        view_id: ViewId,
        command: FakeNavigationCommand,
    ) {
        if let FakeNavigationCommand::NavigateTo(url) = &command {
            self.state.lock().current_urls.insert(view_id, url.clone());
        }

        self.state
            .lock()
            .navigation_commands
            .push(FakeNavigationCommandRequest { view_id, command });
    }
}

pub struct FakeWebViewBackendGuard;

impl Drop for FakeWebViewBackendGuard {
    fn drop(&mut self) {
        INSTALLED_BACKEND.with(|installed_backend| {
            installed_backend.replace(None);
        });
    }
}

impl WebViewBackend for FakeWebViewBackend {
    fn create(
        &self,
        _ctx: &ViewerContext<'_>,
        view_id: ViewId,
        url: &str,
        _bounds: WebViewBounds,
        session: WebViewSession,
    ) -> Result<WebViewInstance, WebViewBackendError> {
        let mut state = self.state.lock();

        if let Some(message) = &state.creation_error {
            return Err(WebViewBackendError::CreationFailed(message.clone()));
        }

        state.created_instances.push(FakeCreatedWebView {
            view_id,
            url: url.to_owned(),
            session: session.as_str().to_owned(),
        });

        Ok(WebViewInstance::new_fake(
            view_id,
            url.to_owned(),
            self.clone(),
        ))
    }
}

pub(crate) fn installed_backend() -> Option<FakeWebViewBackend> {
    INSTALLED_BACKEND.with(|installed_backend| installed_backend.borrow().clone())
}
