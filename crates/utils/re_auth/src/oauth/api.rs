//! HTTP Client for Rerun's Auth API.

use std::{collections::HashMap, sync::LazyLock};

use base64::{Engine as _, prelude::BASE64_URL_SAFE_NO_PAD};
use sha2::{Digest as _, Sha256};

use crate::oauth::OAUTH_CLIENT_ID;

use super::RefreshToken;

static API_BASE_URL: LazyLock<String> = LazyLock::new(|| {
    std::env::var("RERUN_AUTH_API_BASE_URL")
        .ok()
        .unwrap_or_else(|| "https://rerun.io/api".into())
});

fn endpoint(endpoint: impl std::fmt::Display) -> String {
    format!("{base_url}{endpoint}", base_url = *API_BASE_URL)
}

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

#[cfg(not(target_arch = "wasm32"))]
pub fn send_native<Req: IntoRequest>(
    req: Req,
    on_response: impl FnOnce(Result<Req::Res, Error>) + Send + 'static,
) {
    let req = match req.into_request() {
        Ok(req) => req,
        Err(err) => return on_response(Err(err)),
    };
    ehttp::fetch(req, |res| {
        let res = res.map_err(Error::Request).and_then(|res| {
            if !res.ok {
                if !res.bytes.is_empty() {
                    re_log::trace!("error response: {:?}", res.text());
                    let err = serde_json::from_slice(&res.bytes).map_err(Error::Deserialize)?;
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

pub async fn send_async<Req: IntoRequest>(req: Req) -> Result<Req::Res, Error> {
    let req = req.into_request()?;

    // `fetch_async` holds a `JsValue` across an await point, which is not `Send`.
    // But wasm is single-threaded, so we don't care.
    let res = crate::wasm_compat::make_future_send_on_wasm(ehttp::fetch_async(req))
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

    serde_json::from_slice::<Req::Res>(&res.bytes).map_err(Error::Deserialize)
}

pub trait IntoRequest: Sized {
    type Res: serde::de::DeserializeOwned;

    fn into_request(self) -> Result<ehttp::Request, Error>;
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshRequest {
    refresh_token: String,
}

impl IntoRequest for RefreshRequest {
    type Res = RefreshResponse;

    fn into_request(self) -> Result<ehttp::Request, Error> {
        ehttp::Request::json(endpoint("/refresh"), &self).map_err(Error::Serialize)
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshResponse {
    pub user: super::User,
    pub organization_id: Option<String>,
    pub access_token: String,
    pub refresh_token: String,
}

pub(crate) async fn refresh(refresh_token: &RefreshToken) -> Result<RefreshResponse, Error> {
    send_async(RefreshRequest {
        refresh_token: refresh_token.0.clone(),
    })
    .await
}

struct JwksRequest;

impl IntoRequest for JwksRequest {
    type Res = jsonwebtoken::jwk::JwkSet;

    fn into_request(self) -> Result<ehttp::Request, Error> {
        Ok(ehttp::Request::get(endpoint("/jwks")))
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
    let endpoint = endpoint("/authorize");
    let client_id = &*OAUTH_CLIENT_ID;
    let code_challenge = &pkce.code_challenge;
    format!(
        "\
        {endpoint}\
        ?response_type=code\
        &client_id={client_id}\
        &redirect_uri={redirect_uri}\
        &state={state}\
        &provider=authkit\
        &code_challenge={code_challenge}\
        &code_challenge_method=S256\
    "
    )
}

#[derive(Debug, serde::Serialize)]
pub struct AuthenticateWithCode<'a> {
    code: &'a str,
    code_verifier: &'a str,
    user_agent: &'a str,
}

impl<'a> AuthenticateWithCode<'a> {
    pub fn new(code: &'a str, pkce: &'a Pkce, user_agent: &'a str) -> Self {
        Self {
            code,
            code_verifier: &pkce.code_verifier,
            user_agent,
        }
    }
}

impl IntoRequest for AuthenticateWithCode<'_> {
    type Res = AuthenticationResponse;

    fn into_request(self) -> Result<ehttp::Request, Error> {
        ehttp::Request::json(endpoint("/authenticate"), &self).map_err(Error::Serialize)
    }
}

#[expect(dead_code)] // fields may become used at some point in the near future
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthenticationResponse {
    user: User,
    organization_id: Option<String>,
    access_token: String,
    refresh_token: String,
    impersonator: Option<Impersonator>,
    authentication_method: Option<AuthenticationMethod>,
    sealed_session: Option<String>,
    oauth_tokens: Option<OauthTokens>,
}

impl From<AuthenticationResponse> for RefreshResponse {
    fn from(value: AuthenticationResponse) -> Self {
        Self {
            user: value.user.into(),
            organization_id: value.organization_id,
            access_token: value.access_token,
            refresh_token: value.refresh_token,
        }
    }
}

#[expect(dead_code)] // fields may become used at some point in the near future
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
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
    metadata: HashMap<String, String>,
}

impl From<User> for crate::oauth::User {
    fn from(value: User) -> Self {
        Self {
            id: value.id,
            email: value.email,
            metadata: value.metadata,
        }
    }
}

#[expect(dead_code)] // fields may become used at some point in the near future
#[derive(Debug, Clone, serde::Deserialize)]
struct Impersonator {
    email: String,
    reason: Option<String>,
}

#[expect(clippy::upper_case_acronyms)] // It's better than a serde(rename)
#[derive(Debug, Clone, serde::Deserialize)]
enum AuthenticationMethod {
    SSO,
    Password,
    Passkey,
    AppleOAuth,
    GitHubOAuth,
    GoogleOAuth,
    MicrosoftOAuth,
    MagicAuth,
    Impersonation,
}

#[expect(dead_code)] // fields may become used at some point in the near future
#[derive(Debug, Clone, serde::Deserialize)]
struct OauthTokens {
    access_token: String,
    refresh_token: String,
    expires_at: u64,
    scopes: Vec<String>,
}
