use std::cell::RefCell;
use std::rc::Rc;

use re_auth::oauth::Credentials;
use re_auth::oauth::api::{AuthenticateWithCode, Pkce, authorization_url, send_async};
use re_log::ResultExt as _;
use re_viewer_context::AsyncRuntimeHandle;
use url::Url;
use uuid::Uuid;
use wasm_bindgen::JsCast as _;
use wasm_bindgen::prelude::Closure;

#[expect(clippy::needless_pass_by_value)]
fn js_value_to_string(s: wasm_bindgen::JsValue) -> String {
    // it's already a string
    if let Some(s) = s.as_string() {
        return s;
    }

    // it's an Error, call `toString` instead
    if let Some(s) = s.dyn_ref::<js_sys::Error>() {
        return format!("{}", s.to_string());
    }

    format!("{s:#?}")
}

type StorageEventCallback = dyn FnMut(web_sys::StorageEvent);

pub struct State {
    child_window: Option<web_sys::Window>,
    on_storage_event: Option<Closure<StorageEventCallback>>,

    pkce: Rc<Pkce>,
    state: String,

    result: Rc<RefCell<Option<Result<Credentials, String>>>>,
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
    pub fn start(&mut self) -> Result<(), String> {
        if self.child_window.is_some() {
            // If already started, do nothing
            return Ok(());
        }

        let parent_window = web_sys::window().expect("no window available");

        // Open popup window:
        //   <origin>/signed-in
        let redirect_uri = {
            let location = parent_window.location();
            let origin = location.origin().map_err(js_value_to_string)?;
            let mut url = Url::parse(&origin).map_err(|err| err.to_string())?;
            if url.host_str() == Some("localhost") {
                // For `localhost`, we have to map it to `127.0.0.1`, otherwise
                // it's not a valid redirect URI.
                let port = url.port();
                url.set_host(Some("127.0.0.1")).ok();
                url.set_port(port).ok();
            }

            url.set_path("signed-in");
            url.to_string()
        };
        let login_url = authorization_url(&redirect_uri, &self.state, &self.pkce);

        let Some(child_window) = parent_window
            .open_with_url_and_target_and_features(&login_url, "auth", "width=480,height=640")
            .map_err(js_value_to_string)?
        else {
            return Err("window.open did not return a handle".into());
        };

        // Keep a handle to the opened window, so we can close it later:
        self.child_window = Some(child_window);
        Ok(())
    }

    #[expect(clippy::unused_self)] // compat with native api
    pub fn ui(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label("Waiting for login…");
        });
    }

    #[expect(clippy::needless_pass_by_ref_mut)] // compat with native api
    pub fn done(&mut self) -> Result<Option<Credentials>, String> {
        // Check if we have credentials
        if let Some(credentials) = self.result.borrow_mut().take() {
            return credentials.map(Some);
        }

        // Check if popup window was manually closed by user
        if let Some(child_window) = &self.child_window {
            // ignoring the error here
            if child_window
                .closed()
                .map_err(js_value_to_string)
                .ok_or_log_error()
                .unwrap_or_default()
            {
                return Err("Login popup was closed before completing authentication".into());
            }
        }

        Ok(None)
    }

    #[expect(clippy::needless_pass_by_ref_mut, clippy::unnecessary_wraps)]
    pub fn open(ui: &mut egui::Ui) -> Result<Self, String> {
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
            let egui_ctx = ui.ctx().clone();
            move |e: web_sys::StorageEvent| {
                AsyncRuntimeHandle::new_web().spawn_future(try_handle_storage_event(
                    e,
                    pkce.clone(),
                    stored_state.clone(),
                    result.clone(),
                    egui_ctx.clone(),
                ));
            }
        }) as Box<StorageEventCallback>);

        parent_window
            .add_event_listener_with_callback("storage", on_storage_event.as_ref().unchecked_ref())
            .ok();

        Ok(Self {
            child_window: None,
            on_storage_event: Some(on_storage_event),
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
    result: Rc<RefCell<Option<Result<Credentials, String>>>>,
    egui_ctx: egui::Context,
) {
    macro_rules! bail {
        ($err:expr) => {{
            let err = $err.to_string();
            re_log::error!("{err}");
            *result.borrow_mut() = Some(Err(err));
            return;
        }};
    }

    // Instead of setting the credentials in localstorage directly,
    // we instead first set it to a different key, and then only
    // when everything has succeeded do we store the final credentials
    // in localstorage. This ensures that if something goes wrong at
    // any point during the login flow, we do not invalidate the
    // user's existing credentials until the _very_ end.

    // The payload is received on the `_auth` key,
    // see `AuthEventPayload` above for what it looks like.
    if e.key().as_deref() != Some("_auth") {
        return;
    }
    let Some(new_value) = e.new_value() else {
        bail!("auth storage event without new value");
    };
    let payload = match serde_json::from_str::<AuthEventPayload>(&new_value) {
        Ok(payload) => payload,
        Err(err) => {
            bail!(err);
        }
    };
    if payload.type_ != "auth" {
        re_log::error!("storage event payload.type != auth");
        return;
    }
    // The payload contains the URL to which the user was redirected to by
    // the login flow, which stores the code and state in its search params:
    let Some(url) = url::Url::parse(&payload.url).ok_or_log_error() else {
        return;
    };

    let Some(code) = url.query_pairs().find(|(k, _)| k == "code").map(|(_, v)| v) else {
        bail!("missing code in url");
    };
    let Some(state) = url
        .query_pairs()
        .find(|(k, _)| k == "state")
        .map(|(_, v)| v)
    else {
        bail!("missing state in url");
    };

    if state != stored_state {
        bail!("invalid state");
    }

    // Now we need to exchange the code for tokens:
    let res = match send_async(AuthenticateWithCode::new(&code, &pkce)).await {
        Ok(res) => res,
        Err(err) => {
            bail!(err);
        }
    };

    let credentials = match re_auth::oauth::Credentials::from_auth_response(res.into()) {
        Ok(v) => v,
        Err(err) => {
            bail!(err);
        }
    };

    // As the last step, we store the credentials in local storage:
    let credentials = match credentials.ensure_stored() {
        Ok(v) => v,
        Err(err) => {
            bail!(err);
        }
    };

    // And then notify the UI that the login succeeded:
    *result.borrow_mut() = Some(Ok(credentials));
    egui_ctx.request_repaint();
}
