use base64::prelude::*;
use re_auth::oauth::{Credentials, api::AuthenticationResponse};
use re_log::ResultExt as _;
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{JsCast as _, prelude::Closure};

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

    nonce: String,
    result: Rc<RefCell<Option<Credentials>>>,
}

impl Drop for State {
    fn drop(&mut self) {
        re_log::debug!("dropping auth state");
        if let Some(child_window) = &self.child_window {
            child_window.close().ok();
        }
        if let Some(window) = web_sys::window() {
            if let Some(on_storage_event) = &self.on_storage_event {
                window
                    .remove_event_listener_with_callback(
                        "storage",
                        on_storage_event.as_ref().unchecked_ref(),
                    )
                    .ok();
            }
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

        // Open popup window at `/login/v2`:
        let parent_window = web_sys::window().expect("no window available");
        let origin = parent_window
            .location()
            .origin()
            .map_err(js_value_to_string)?;

        let return_to = format!(
            "{origin}/signed-in?n={nonce}",
            nonce = BASE64_URL_SAFE.encode(&self.nonce),
        );
        let login_url = format!(
            "{login_page_url}?r={return_to}",
            login_page_url = &*re_auth::oauth::api::DEFAULT_LOGIN_URL,
            return_to = BASE64_URL_SAFE.encode(&return_to),
        );

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
            return Ok(Some(credentials));
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

    #[expect(clippy::needless_pass_by_ref_mut)]
    pub fn open(ui: &mut egui::Ui) -> Result<Self, String> {
        let egui_ctx = ui.ctx().clone();

        let parent_window = web_sys::window().expect("no window available");
        let nonce = parent_window
            .crypto()
            .map_err(js_value_to_string)?
            .random_uuid();

        let result = Rc::new(RefCell::new(None));

        // To receive the auth payload after the user logs in, we use
        // a `storage` event callback. When the user is redirected after
        // logging in, they land on a page which is on the same domain
        // as us, where any `localStorage.setItem` calls will result in
        // this event listener being fired.
        let on_storage_event = Closure::wrap(Box::new({
            let result = Rc::clone(&result);
            let nonce = nonce.clone();
            move |e: web_sys::StorageEvent| {
                web_sys::console::log_1(&e);

                // Instead of setting the credentials in localstorage directly,
                // we instead first set it to a different key, and then only
                // when everything has succeeded do we store the final credentials
                // in localstorage. This ensures that if something goes wrong at
                // any point during the login flow, we do not invalidate the
                // user's existing credentials until the _very_ end.

                // The payload is received on the `_auth` key,
                // see `AuthEventPayload` above for what it looks like.
                if e.key().as_deref() != Some("_auth") {
                    re_log::debug!("invalid storage event key: _auth");
                    return;
                }
                let Some(new_value) = e.new_value() else {
                    re_log::debug!("storage event without new value");
                    return;
                };
                let Some(payload) =
                    serde_json::from_str::<AuthEventPayload>(&new_value).ok_or_log_error()
                else {
                    return;
                };
                if payload.type_ != "auth" {
                    re_log::error!("storage event payload.type != auth");
                    return;
                }
                // The payload contains the URL to which the user was redirected to by
                // the login flow, which stores the nonce and auth response in its search params:
                let Some(url) = url::Url::parse(&payload.url).ok_or_log_error() else {
                    return;
                };

                let n = url.query_pairs().find(|(k, _)| k == "n").map(|(_, v)| v);
                let t = url.query_pairs().find(|(k, _)| k == "t").map(|(_, v)| v);

                let Some((encoded_nonce, encoded_tokens)) = n.zip(t) else {
                    re_log::error!("authentication failed: missing n/t params");
                    return;
                };

                // The nonce is a base64-encoded v4 uuid.
                let Some(decoded_nonce) = BASE64_URL_SAFE
                    .decode(encoded_nonce.as_bytes())
                    .ok_or_log_error()
                else {
                    return;
                };
                let Some(decoded_nonce) = String::from_utf8(decoded_nonce).ok_or_log_error() else {
                    return;
                };
                if decoded_nonce != nonce {
                    re_log::error!("authentication failed: n mismatch");
                    return;
                }

                // The auth response is a base64-encoded json object which
                // holds the token pair, and also user info, such as their email:
                let Some(response) = BASE64_URL_SAFE
                    .decode(encoded_tokens.as_bytes())
                    .ok_or_log_error()
                else {
                    return;
                };
                let Some(response) =
                    serde_json::from_slice::<AuthenticationResponse>(&response).ok_or_log_error()
                else {
                    return;
                };

                #[expect(unsafe_code)]
                let Some(credentials) =
                // SAFETY: credentials come from a trusted source
                    unsafe { re_auth::oauth::Credentials::from_auth_response(response) }
                        .ok_or_log_error()
                else {
                    return;
                };

                // As a last step, we store the actual credentials in local storage:
                let Some(credentials) = credentials.ensure_stored().ok_or_log_error() else {
                    return;
                };

                // And then notify the UI that the login succeeded:
                *result.borrow_mut() = Some(credentials);
                egui_ctx.request_repaint();
            }
        }) as Box<StorageEventCallback>);

        parent_window
            .add_event_listener_with_callback("storage", on_storage_event.as_ref().unchecked_ref())
            .ok();

        Ok(Self {
            child_window: None,
            on_storage_event: Some(on_storage_event),
            nonce,
            result,
        })
    }
}
