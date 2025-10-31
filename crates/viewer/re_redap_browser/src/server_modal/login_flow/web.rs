use crate::web_tools::string_from_js_value;
use base64::prelude::*;
use re_auth::oauth::{Credentials, api::AuthenticationResponse};
use re_log::ResultExt;
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{JsCast, prelude::Closure};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to open window, popup was blocked: {0}")]
    OpenWindow(String),
}

impl From<wasm_bindgen::JsValue> for Error {
    fn from(value: wasm_bindgen::JsValue) -> Self {
        Error::FailedToOpenWindow(string_from_js_value(value))
    }
}

type StorageEventCallback = dyn FnMut(web_sys::StorageEvent);

pub struct State {
    child_window: web_sys::Window,
    on_storage_event: Closure<StorageEventCallback>,

    result: Rc<RefCell<Option<Credentials>>>,
}

impl Drop for State {
    fn drop(&mut self) {
        re_log::debug!("dropping auth state");
        self.child_window.close().ok();
        if let Some(window) = web_sys::window() {
            window
                .remove_event_listener_with_callback(
                    "storage",
                    self.on_storage_event.as_ref().unchecked_ref(),
                )
                .ok();
        };
    }
}

#[derive(Debug, serde::Deserialize)]
struct AuthEventPayload {
    #[serde(rename = "type")]
    type_: String,
    url: String,
}

impl State {
    pub fn done(&self) -> Option<Credentials> {
        self.result.borrow_mut().take()
    }

    pub fn open(ui: &mut egui::Ui) -> Result<Option<Self>, Error> {
        let egui_ctx = ui.ctx().clone();

        let parent_window = web_sys::window().expect("no window available");
        let origin = parent_window.location().origin()?;

        let nonce = parent_window.crypto()?.random_uuid();
        let return_to = format!(
            "{origin}/signed-in?n={nonce}",
            nonce = BASE64_URL_SAFE.encode(&nonce),
        );
        let login_url = format!(
            "/login/v2?r={return_to}",
            return_to = BASE64_URL_SAFE.encode(&return_to),
        );

        let Some(child_window) = parent_window.open_with_url_and_target_and_features(
            &login_url,
            "auth",
            "width=480,height=640",
        )?
        else {
            return Ok(None);
        };

        // TODO: clean up this mess
        let result = Rc::new(RefCell::new(None));
        let on_storage_event = Closure::wrap(Box::new({
            let result = Rc::clone(&result);
            move |e: web_sys::StorageEvent| {
                web_sys::console::log_1(&e);

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
                let Some(url) = url::Url::parse(&payload.url).ok_or_log_error() else {
                    return;
                };

                let n = url.query_pairs().find(|(k, _)| k == "n").map(|(_, v)| v);
                let t = url.query_pairs().find(|(k, _)| k == "t").map(|(_, v)| v);

                let Some((encoded_nonce, encoded_tokens)) = n.zip(t) else {
                    re_log::error!("authentication failed: missing n/t params");
                    return;
                };

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

                #[allow(unsafe_code)] // misusing does not cause UB, only a bad day
                let Some(credentials) =
                    unsafe { re_auth::oauth::Credentials::from_auth_response(response) }
                        .ok_or_log_error()
                else {
                    return;
                };

                let Some(credentials) = credentials.ensure_stored().ok_or_log_error() else {
                    return;
                };

                *result.borrow_mut() = Some(credentials);
                egui_ctx.request_repaint();
            }
        }) as Box<StorageEventCallback>);

        parent_window
            .add_event_listener_with_callback("storage", on_storage_event.as_ref().unchecked_ref())
            .ok();

        Ok(Some(Self {
            child_window,
            on_storage_event,
            result,
        }))
    }
}
