use std::cell::RefCell;
use std::rc::Rc;

use re_async::{AsyncRuntimeHandle, UiWaker};
use re_auth::oauth::Credentials;
use re_auth::oauth::api::{AuthenticateWithCode, Pkce, authorization_url, send_async};
use re_log::ResultExt as _;
use re_ui::{ContextExt as _, UiExt as _};
use uuid::Uuid;
use wasm_bindgen::JsCast as _;
use wasm_bindgen::prelude::Closure;

use super::LoginError;

type StorageEventCallback = dyn FnMut(web_sys::StorageEvent);

pub struct State {
    child_window: Option<web_sys::Window>,
    on_storage_event: Option<Closure<StorageEventCallback>>,

    signed_in_url: String,
    pkce: Rc<Pkce>,
    state: String,

    result: Rc<RefCell<Option<Result<Credentials, LoginError>>>>,
}

impl Drop for State {
    fn drop(&mut self) {
        re_log::debug!("dropping auth state");
        if let Some(child_window) = &self.child_window {
            child_window.close().ok();
        }
        if let Some(window) = web_sys::window()
            && let Some(on_storage_event) = &self.on_storage_event
        {
            window
                .remove_event_listener_with_callback(
                    "storage",
                    on_storage_event.as_ref().unchecked_ref(),
                )
                .ok();
        }
    }
}

#[derive(Debug, serde::Deserialize)]
struct AuthEventPayload {
    #[serde(rename = "type")]
    type_: String,
    url: String,
}

impl State {
    pub fn start(&mut self) -> Result<(), LoginError> {
        if self.child_window.is_some() {
            // If already started, do nothing
            return Ok(());
        }

        let parent_window = web_sys::window().expect("no window available");

        let login_url = authorization_url(&self.signed_in_url, &self.state, &self.pkce);

        let Some(child_window) = parent_window
            .open_with_url_and_target_and_features(&login_url, "auth", "width=480,height=640")
            .map_err(|err| LoginError::OpenPopup {
                source: re_web::Error::from(err),
            })?
        else {
            return Err(LoginError::NoPopupHandle);
        };

        // Keep a handle to the opened window, so we can close it later:
        self.child_window = Some(child_window);
        Ok(())
    }

    #[expect(clippy::unused_self)] // compat with native api
    pub fn ui(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.loading_indicator("Waiting for web login");
            ui.label("Waiting for login…");
        });
    }

    #[expect(clippy::needless_pass_by_ref_mut)] // compat with native api
    pub fn done(&mut self) -> Result<Option<Credentials>, LoginError> {
        // Check if we have credentials
        if let Some(credentials) = self.result.borrow_mut().take() {
            return credentials.map(Some);
        }

        // Check if popup window was manually closed by user
        if let Some(child_window) = &self.child_window {
            // ignoring the error here
            if child_window
                .closed()
                .map_err(re_web::Error::from)
                .ok_or_log_error()
                .unwrap_or_default()
            {
                return Err(LoginError::PopupClosed);
            }
        }

        Ok(None)
    }

    #[expect(clippy::unnecessary_wraps)]
    pub fn open(egui_ctx: &egui::Context, signed_in_url: String) -> Result<Self, LoginError> {
        let parent_window = web_sys::window().expect("no window available");
        let pkce = Rc::new(Pkce::new());
        let state = Uuid::new_v4().to_string();

        let result = Rc::new(RefCell::new(None));

        // To receive the auth payload after the user logs in, we use
        // a `storage` event callback. When the user is redirected after
        // logging in, they land on a page which is on the same domain
        // as us, where any `localStorage.setItem` calls will result in
        // this event listener being fired.
        let on_storage_event = Closure::wrap(Box::new({
            let result = Rc::clone(&result);
            let pkce = pkce.clone();
            let stored_state = state.clone();
            let waker = egui_ctx.ui_waker();
            move |e: web_sys::StorageEvent| {
                AsyncRuntimeHandle::new_web().spawn_future(try_handle_storage_event(
                    e,
                    pkce.clone(),
                    stored_state.clone(),
                    result.clone(),
                    waker.clone(),
                ));
            }
        }) as Box<StorageEventCallback>);

        parent_window
            .add_event_listener_with_callback("storage", on_storage_event.as_ref().unchecked_ref())
            .ok();

        Ok(Self {
            child_window: None,
            on_storage_event: Some(on_storage_event),
            signed_in_url,
            pkce,
            state,
            result,
        })
    }
}

async fn try_handle_storage_event(
    e: web_sys::StorageEvent,
    pkce: Rc<Pkce>,
    stored_state: String,
    result: Rc<RefCell<Option<Result<Credentials, LoginError>>>>,
    waker: UiWaker,
) {
    match handle_storage_event(e, &pkce, &stored_state).await {
        Ok(Some(credentials)) => {
            // Notify the UI that the login succeeded:
            *result.borrow_mut() = Some(Ok(credentials));
            waker.wake();
        }
        Ok(None) => {}
        Err(err) => {
            re_log::error!("{err}");
            *result.borrow_mut() = Some(Err(err));
        }
    }
}

/// Returns `Ok(None)` if the event is not a login event.
async fn handle_storage_event(
    e: web_sys::StorageEvent,
    pkce: &Pkce,
    stored_state: &str,
) -> Result<Option<Credentials>, LoginError> {
    // Instead of setting the credentials in localstorage directly,
    // we instead first set it to a different key, and then only
    // when everything has succeeded do we store the final credentials
    // in localstorage. This ensures that if something goes wrong at
    // any point during the login flow, we do not invalidate the
    // user's existing credentials until the _very_ end.

    // The payload is received on the `_auth` key,
    // see `AuthEventPayload` above for what it looks like.
    if e.key().as_deref() != Some("_auth") {
        return Ok(None);
    }
    let new_value = e.new_value().ok_or(LoginError::MissingStorageValue)?;
    let payload = serde_json::from_str::<AuthEventPayload>(&new_value)
        .map_err(|source| LoginError::ParseStorageEvent { source })?;
    if payload.type_ != "auth" {
        re_log::error!("storage event payload.type != auth");
        return Ok(None);
    }
    // The payload contains the URL to which the user was redirected to by
    // the login flow, which stores the code and state in its search params:
    let Some(url) = url::Url::parse(&payload.url).ok_or_log_error() else {
        return Ok(None);
    };

    let code = url
        .query_pairs()
        .find(|(k, _)| k == "code")
        .map(|(_, v)| v)
        .ok_or(LoginError::MissingCode)?;
    let state = url
        .query_pairs()
        .find(|(k, _)| k == "state")
        .map(|(_, v)| v)
        .ok_or(LoginError::MissingState)?;

    if state != stored_state {
        return Err(LoginError::InvalidState);
    }

    // Now we need to exchange the code for tokens:
    let res = send_async(AuthenticateWithCode::new(&code, pkce))
        .await
        .map_err(|source| LoginError::Authenticate { source })?;

    let credentials = re_auth::oauth::Credentials::from_auth_response(res.into())
        .map_err(|source| LoginError::DecodeToken { source })?;

    // As the last step, we store the credentials in local storage:
    let credentials = credentials
        .ensure_stored()
        .map_err(|source| LoginError::StoreCredentials { source })?;

    Ok(Some(credentials))
}
