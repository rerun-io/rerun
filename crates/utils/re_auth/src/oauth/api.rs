//! HTTP Client for Rerun's Auth API.

use std::sync::LazyLock;

use base64::{Engine as _, prelude::BASE64_URL_SAFE_NO_PAD};
use sha2::{Digest as _, Sha256};

use crate::oauth::OAUTH_CLIENT_ID;

use super::RefreshToken;

static WORKOS_API: LazyLock<String> = LazyLock::new(|| {
    std::env::var("WORKOS_API_BASE_URL")
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

    fn into_request(self) -> Result<ehttp::Request, Error>;
}

// NOTE: We use PKCE, so refresh token doesn't require client secret.
#[derive(serde::Serialize)]
pub struct AuthenticateWithRefresh<'a> {
    grant_type: &'a str,
    client_id: &'a str,
    refresh_token: &'a str,
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

pub(crate) async fn refresh(refresh_token: &RefreshToken) -> Result<RefreshResponse, Error> {
    send_async(AuthenticateWithRefresh {
        grant_type: "refresh_token",
        client_id: &OAUTH_CLIENT_ID,
        refresh_token: &refresh_token.0,
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

pub fn authorization_url(
    redirect_uri: &str,
    state: &str,
    pkce: &Pkce,
    login_hint: Option<&str>,
) -> String {
    let mut url = format!(
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

    if let Some(login_hint) = login_hint {
        url = format!("{url}&login_hint={login_hint}");
    }

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
