//! HTTP Client for Rerun's Auth API.

use std::sync::LazyLock;

use super::{RefreshToken, User};

static API_BASE_URL: LazyLock<String> = LazyLock::new(|| {
    std::env::var("RERUN_AUTH_API_BASE_URL")
        .ok()
        .unwrap_or_else(|| "https://rerun.io/api".into())
});

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to serialize request: {0}")]
    Serialize(serde_json::Error),

    #[error("failed to deserialize response: {0}")]
    Deserialize(serde_json::Error),

    #[error("{0}")]
    Http(HttpError),

    #[error("{0}")]
    Request(String),
}

#[derive(Debug, Clone, serde::Deserialize, thiserror::Error)]
#[error("{message}")]
pub struct HttpError {
    pub code: String,
    pub message: String,
}

async fn send<Res: serde::de::DeserializeOwned>(request: ehttp::Request) -> Result<Res, Error> {
    // `fetch_async` holds a `JsValue` across an await point, which is not `Send`.
    // But wasm is single-threaded, so we don't care.
    let res = crate::wasm_compat::make_future_send_on_wasm(ehttp::fetch_async(request))
        .await
        .map_err(Error::Request)?;

    if !res.ok {
        if !res.bytes.is_empty() {
            re_log::trace!("error response: {:?}", res.text());
            let err = serde_json::from_slice(&res.bytes).map_err(Error::Deserialize)?;
            return Err(Error::Http(err));
        } else {
            return Err(Error::Request(res.status_text.clone()));
        }
    }

    serde_json::from_reader(std::io::Cursor::new(res.bytes)).map_err(Error::Deserialize)
}

async fn get<Res: serde::de::DeserializeOwned>(
    endpoint: impl std::fmt::Display,
) -> Result<Res, Error> {
    send(ehttp::Request::get(format!(
        "{base_url}{endpoint}",
        base_url = *API_BASE_URL
    )))
    .await
}

async fn post<Body: serde::Serialize, Res: serde::de::DeserializeOwned>(
    endpoint: impl std::fmt::Display,
    body: Body,
) -> Result<Res, Error> {
    send(
        ehttp::Request::json(
            format!("{base_url}{endpoint}", base_url = *API_BASE_URL),
            &body,
        )
        .map_err(Error::Deserialize)?,
    )
    .await
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct RefreshRequest {
    refresh_token: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthenticationResponse {
    pub user: User,
    pub organization_id: Option<String>,
    pub access_token: String,
    pub refresh_token: String,
}

pub(crate) async fn refresh(refresh_token: &RefreshToken) -> Result<AuthenticationResponse, Error> {
    post(
        "/refresh",
        RefreshRequest {
            refresh_token: refresh_token.0.clone(),
        },
    )
    .await
}

pub async fn jwks() -> Result<jsonwebtoken::jwk::JwkSet, Error> {
    get("/jwks").await
}
