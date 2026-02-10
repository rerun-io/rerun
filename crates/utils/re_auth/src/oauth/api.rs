//! HTTP Client for Rerun's Auth API.

use std::sync::LazyLock;

use base64::Engine as _;
use base64::prelude::BASE64_URL_SAFE_NO_PAD;
use sha2::{Digest as _, Sha256};

use super::RefreshToken;
use crate::{Permission, oauth::OAUTH_CLIENT_ID};

static WORKOS_API: LazyLock<String> = LazyLock::new(|| {
    std::env::var("RERUN_OAUTH_SERVER_URL")
        .ok()
        .unwrap_or_else(|| "https://api.workos.com".into())
});

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to serialize request: {0}")]
    Serialize(serde_json::Error),

    #[error("failed to deserialize response: {0}")]
    Deserialize(serde_json::Error),

    #[error("{0}")]
    Http(String),

    #[error("{0}")]
    Request(String),
}

#[cfg(not(target_arch = "wasm32"))]
pub fn send_native<Req: IntoRequest>(
    req: Req,
    on_response: impl FnOnce(Result<Req::Res, Error>) + Send + 'static,
) {
    let req = match req.into_request() {
        Ok(req) => req,
        Err(err) => return on_response(Err(err)),
    };
    ehttp::fetch(req, move |res| {
        let res = res.map_err(Error::Request).and_then(move |res| {
            if !res.ok {
                if !res.bytes.is_empty() {
                    re_log::trace!("error response: {:?}", res.text());
                    let err = String::from_utf8_lossy(&res.bytes).into_owned();
                    return Err(Error::Http(err));
                } else {
                    return Err(Error::Request(res.status_text.clone()));
                }
            }

            serde_json::from_slice(&res.bytes).map_err(Error::Deserialize)
        });
        on_response(res);
    });
}

fn is_allowed_error<Req: IntoRequest>(status: u16) -> bool {
    Req::ALLOW_4XX && (400..=499).contains(&status)
}

pub async fn send_async<Req: IntoRequest>(req: Req) -> Result<Req::Res, Error> {
    let req = req.into_request()?;

    // `fetch_async` holds a `JsValue` across an await point, which is not `Send`.
    // But wasm is single-threaded, so we don't care.
    let res = crate::wasm_compat::make_future_send_on_wasm(ehttp::fetch_async(req))
        .await
        .map_err(Error::Request)?;

    if !res.ok && !is_allowed_error::<Req>(res.status) {
        if !res.bytes.is_empty() {
            re_log::trace!("error response: {:?}", res.text());
            let err = String::from_utf8_lossy(&res.bytes).into_owned();
            return Err(Error::Http(err));
        } else {
            return Err(Error::Request(res.status_text.clone()));
        }
    }

    serde_json::from_slice::<Req::Res>(&res.bytes).map_err(Error::Deserialize)
}

/// Like `send_async`, but allows `4xx` status codes to go through.
///
/// The `Req::Res` type must handle deserializing the error response.
pub async fn send_async_allow_4xx<Req: IntoRequest>(req: Req) -> Result<Req::Res, Error> {
    let req = req.into_request()?;

    // `fetch_async` holds a `JsValue` across an await point, which is not `Send`.
    // But wasm is single-threaded, so we don't care.
    let res = crate::wasm_compat::make_future_send_on_wasm(ehttp::fetch_async(req))
        .await
        .map_err(Error::Request)?;

    if !res.ok && res.status < 400 || res.status > 499 {
        if !res.bytes.is_empty() {
            re_log::trace!("error response: {:?}", res.text());
            let err = String::from_utf8_lossy(&res.bytes).into_owned();
            return Err(Error::Http(err));
        } else {
            return Err(Error::Request(res.status_text.clone()));
        }
    }

    serde_json::from_slice::<Req::Res>(&res.bytes).map_err(Error::Deserialize)
}

pub trait IntoRequest: Sized {
    type Res: serde::de::DeserializeOwned;

    /// Whether to allow `4xx` error codes through.
    ///
    /// `Self::Res` must handle deserializing either the success or error responses.
    const ALLOW_4XX: bool = false;

    fn into_request(self) -> Result<ehttp::Request, Error>;
}

// NOTE: We use PKCE, so refresh token doesn't require client secret.
#[derive(serde::Serialize)]
pub struct AuthenticateWithRefresh<'a> {
    grant_type: &'a str,
    client_id: &'a str,
    refresh_token: &'a str,

    #[serde(skip_serializing_if = "Option::is_none")]
    organization_id: Option<&'a str>,
}

impl IntoRequest for AuthenticateWithRefresh<'_> {
    type Res = RefreshResponse;

    fn into_request(self) -> Result<ehttp::Request, Error> {
        ehttp::Request::json(
            format_args!("{base}/user_management/authenticate", base = *WORKOS_API),
            &self,
        )
        .map_err(Error::Serialize)
    }
}

#[derive(serde::Deserialize)]
pub struct RefreshResponse {
    pub user: super::User,
    pub organization_id: Option<String>,
    pub access_token: String,
    pub refresh_token: String,
}

pub(crate) async fn refresh(
    refresh_token: &RefreshToken,
    organization_id: Option<&str>,
) -> Result<RefreshResponse, Error> {
    send_async(AuthenticateWithRefresh {
        grant_type: "refresh_token",
        client_id: &OAUTH_CLIENT_ID,
        refresh_token: &refresh_token.0,
        organization_id,
    })
    .await
}

struct JwksRequest;

impl IntoRequest for JwksRequest {
    type Res = jsonwebtoken::jwk::JwkSet;

    fn into_request(self) -> Result<ehttp::Request, Error> {
        Ok(ehttp::Request::get(format_args!(
            "{base}/sso/jwks/{client_id}",
            base = *WORKOS_API,
            client_id = *OAUTH_CLIENT_ID,
        )))
    }
}

pub async fn jwks() -> Result<jsonwebtoken::jwk::JwkSet, Error> {
    send_async(JwksRequest).await
}

pub struct Pkce {
    /// random string of bytes
    code_verifier: String,

    /// base64-encoded sha256 hash of `code_verifier`
    code_challenge: String,
}

const CHARSET: &[u8] = b"\
    ABCDEFGHIJKLMNOPQRSTUVWXYZ\
    abcdefghijklmnopqrstuvwxyz\
    0123456789-.~_\
";

impl Pkce {
    pub fn new() -> Self {
        // verifier needs to be large enough to make reversing the hash impractical

        let code_verifier = {
            // generate 128-byte string
            const LEN: usize = 128;
            let mut indices = [0u8; LEN];
            getrandom::fill(&mut indices).expect("failed to generate random numbers");

            String::from_utf8(
                indices
                    .into_iter()
                    .map(|idx| CHARSET[(idx as usize) % CHARSET.len()])
                    .collect::<Vec<u8>>(),
            )
            .expect("invalid charset")
        };

        let code_challenge = {
            // base64url(sha256(code_verifier))
            let mut sha = Sha256::new();
            sha.update(&code_verifier);
            let code_verifier_hash = sha.finalize();
            BASE64_URL_SAFE_NO_PAD.encode(code_verifier_hash)
        };

        Self {
            code_verifier,
            code_challenge,
        }
    }
}

impl Default for Pkce {
    fn default() -> Self {
        Self::new()
    }
}

pub fn authorization_url(redirect_uri: &str, state: &str, pkce: &Pkce) -> String {
    let url = format!(
        "\
        {base}/user_management/authorize\
        ?response_type=code\
        &client_id={client_id}\
        &redirect_uri={redirect_uri}\
        &state={state}\
        &provider=authkit\
        &code_challenge={code_challenge}\
        &code_challenge_method=S256\
    ",
        base = *WORKOS_API,
        client_id = *OAUTH_CLIENT_ID,
        code_challenge = pkce.code_challenge,
    );

    url
}

#[derive(Debug, serde::Serialize)]
pub struct AuthenticateWithCode<'a> {
    grant_type: &'a str,
    client_id: &'a str,
    code: &'a str,
    code_verifier: &'a str,
}

impl<'a> AuthenticateWithCode<'a> {
    pub fn new(code: &'a str, pkce: &'a Pkce) -> Self {
        Self {
            grant_type: "authorization_code",
            client_id: &*OAUTH_CLIENT_ID,
            code,
            code_verifier: &pkce.code_verifier,
        }
    }
}

impl IntoRequest for AuthenticateWithCode<'_> {
    type Res = AuthenticateWithCodeResponse;

    fn into_request(self) -> Result<ehttp::Request, Error> {
        ehttp::Request::json(
            format_args!("{base}/user_management/authenticate", base = *WORKOS_API),
            &self,
        )
        .map_err(Error::Serialize)
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct AuthenticateWithCodeResponse {
    user: User,
    organization_id: Option<String>,
    access_token: String,
    refresh_token: String,
}

impl From<AuthenticateWithCodeResponse> for RefreshResponse {
    fn from(value: AuthenticateWithCodeResponse) -> Self {
        Self {
            user: value.user.into(),
            organization_id: value.organization_id,
            access_token: value.access_token,
            refresh_token: value.refresh_token,
        }
    }
}

#[expect(dead_code)] // maybe these fields are useful in the future
#[derive(Debug, Clone, serde::Deserialize)]
struct User {
    id: String,
    email: String,
    email_verified: bool,
    profile_picture_url: Option<String>,
    first_name: Option<String>,
    last_name: Option<String>,
    last_sign_in_at: Option<String>,
    created_at: String,
    updated_at: String,
    external_id: Option<String>,
}

impl From<User> for crate::oauth::User {
    fn from(value: User) -> Self {
        Self {
            id: value.id,
            email: value.email,
        }
    }
}

#[derive(Debug, serde::Serialize)]
pub struct GetDeviceAuthUrl<'a> {
    pub client_id: &'a str,
}

#[derive(serde::Deserialize)]
pub struct GetDeviceAuthUrlResponse {
    pub device_code: String,
    pub expires_in: i64,
    #[serde(rename = "interval")]
    pub interval_seconds: i64,
    pub user_code: String,
    pub verification_uri: String,
    pub verification_uri_complete: String,
}

impl IntoRequest for GetDeviceAuthUrl<'_> {
    type Res = GetDeviceAuthUrlResponse;

    fn into_request(self) -> Result<ehttp::Request, Error> {
        ehttp::Request::json(
            format_args!(
                "{base}/user_management/authorize/device",
                base = *WORKOS_API,
            ),
            &self,
        )
        .map_err(Error::Serialize)
    }
}

#[derive(Debug, serde::Serialize)]
pub struct AuthenticateWithDeviceCode<'a> {
    client_id: &'a str,
    device_code: &'a str,
    grant_type: &'a str,
}

impl<'a> AuthenticateWithDeviceCode<'a> {
    pub fn new(client_id: &'a str, device_code: &'a str) -> Self {
        Self {
            client_id,
            device_code,
            grant_type: "urn:ietf:params:oauth:grant-type:device_code",
        }
    }
}

#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
pub enum AuthenticateWithDeviceCodeResponse {
    Success {
        user: super::User,
        organization_id: Option<String>,
        access_token: String,
        refresh_token: String,
    },
    Error {
        error: DeviceCodeFlowStatus,
        error_description: String,
    },
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceCodeFlowStatus {
    AuthorizationPending,
    SlowDown,
    AccessDenied,
    ExpiredToken,
    InvalidRequest,
    InvalidClient,
    InvalidGrant,
    UnsupportedGrantType,
}

impl IntoRequest for AuthenticateWithDeviceCode<'_> {
    type Res = AuthenticateWithDeviceCodeResponse;

    const ALLOW_4XX: bool = true;

    fn into_request(self) -> Result<ehttp::Request, Error> {
        ehttp::Request::json(
            format_args!("{base}/user_management/authenticate", base = *WORKOS_API,),
            &self,
        )
        .map_err(Error::Serialize)
    }
}

pub struct GenerateToken<'a> {
    pub server: url::Origin,
    pub token: &'a str,
    pub expiration: jiff::Span,
    pub permission: Permission,
}

#[derive(serde::Deserialize)]
pub struct GenerateTokenResponse {
    pub token: String,
}

impl IntoRequest for GenerateToken<'_> {
    type Res = GenerateTokenResponse;

    fn into_request(self) -> Result<ehttp::Request, Error> {
        #[derive(serde::Serialize)]
        struct Body {
            expiration: jiff::Span,
            permission: Permission,
        }

        let mut req = ehttp::Request::json(
            format_args!(
                "{origin}/generate-token",
                origin = self.server.ascii_serialization()
            ),
            &Body {
                expiration: self.expiration,
                permission: self.permission,
            },
        )
        .map_err(Error::Serialize)?;
        req.headers.insert(
            "Authorization",
            format!("Bearer {token}", token = self.token),
        );

        Ok(req)
    }
}
