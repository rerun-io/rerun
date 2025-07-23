use std::{
    collections::HashMap,
    io::Cursor,
    sync::mpsc,
    time::{Duration, Instant},
};

use base64::prelude::*;

use crate::{Jwt, TokenError, workos};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to bind listener: {0}")]
    BindError(std::io::Error),

    #[error("HTTP server error: {0}")]
    HttpError(std::io::Error),

    #[error("failed to open browser: {0}")]
    WebBrowser(std::io::Error),

    #[error("failed to fetch JWKS: {0}")]
    JwksFetch(String),

    #[error("request timed out while fetching JWKS")]
    JwksFetchTimeout,

    #[error("failed to decode JWKS: {0}")]
    JwksDecode(workos::JwksDecodeError),

    #[error("{0}")]
    InvalidJwt(TokenError),

    #[error("failed to verify token: {0}")]
    Verify(#[from] workos::VerifyError),

    #[error("{0}")]
    Generic(#[from] Box<dyn std::error::Error + Send + Sync + 'static>),
}

trait ResponseCorsExt<R> {
    fn cors(self) -> Self;
}

fn header(key: &[u8], value: &[u8]) -> tiny_http::Header {
    tiny_http::Header::from_bytes(key, value).expect("valid header")
}

impl<R: std::io::Read> ResponseCorsExt<R> for tiny_http::Response<R> {
    fn cors(self) -> Self {
        self.with_header(header(b"Access-Control-Allow-Origin", b"*"))
            .with_header(header(
                b"Access-Control-Allow-Methods",
                b"GET, OPTIONS, HEAD",
            ))
            .with_header(header(b"Access-Control-Allow-Headers", b"*"))
            .with_header(header(b"Access-Control-Max-Age", b"86400"))
    }
}

fn fetch_jwks() -> mpsc::Receiver<Result<workos::Jwks, Error>> {
    let (tx, rx) = mpsc::channel();

    ehttp::fetch(
        ehttp::Request::get(workos::jwks_url()),
        move |res| match res {
            Ok(res) => {
                let res: workos::JwksResponse = match serde_json::from_slice(&res.bytes) {
                    Ok(v) => v,
                    Err(err) => {
                        tx.send(Err(Error::JwksFetch(err.to_string()))).ok();
                        return;
                    }
                };
                let jwks = match res.decode() {
                    Ok(v) => v,
                    Err(err) => {
                        tx.send(Err(Error::JwksDecode(err))).ok();
                        return;
                    }
                };
                tx.send(Ok(jwks)).ok();
            }
            Err(err) => {
                tx.send(Err(Error::JwksFetch(err))).ok();
            }
        },
    );

    rx
}

pub fn login(login_page_url: &str) -> Result<(), Error> {
    let p = indicatif::ProgressBar::new_spinner();

    // Login process:

    // 1. Start web server listening for token
    let server = tiny_http::Server::http("127.0.0.1:0")?;
    p.inc(1);

    // 2. Open authorization URL in browser
    let callback_url = format!("http://{}/callback", server.server_addr());
    let login_url = format!(
        // TODO: don't hardcode this
        "{login_page_url}?r={}",
        BASE64_STANDARD.encode(callback_url.as_bytes()),
    );
    p.println("Opening login page in your browser.");
    p.println("Once you've logged in, the process will continue here.");
    webbrowser::open(&login_url).map_err(Error::WebBrowser)?;
    p.inc(1);

    // 3. Wait for callback, then verify and store tokens
    // While we wait, we can start fetching JWKS in the meantime.
    let rx = fetch_jwks();

    p.set_message("Waiting for browser...");
    let auth = loop {
        p.inc(1);
        let Some(req) = server
            .recv_timeout(Duration::from_millis(100))
            .map_err(Error::HttpError)?
        else {
            continue;
        };

        match req.method() {
            tiny_http::Method::Get => {}
            tiny_http::Method::Options => {
                req.respond(
                    tiny_http::Response::empty(204)
                        .cors()
                        .with_header(header(b"Allow", b"GET, HEAD, OPTIONS")),
                )
                .map_err(Error::HttpError)?;
                continue;
            }
            tiny_http::Method::Head => {
                req.respond(tiny_http::Response::empty(200).cors())
                    .map_err(Error::HttpError)?;
                continue;
            }
            _ => {
                req.respond(
                    tiny_http::Response::empty(405)
                        .cors()
                        .with_header(header(b"Allow", b"GET, HEAD, OPTIONS")),
                )
                .map_err(Error::HttpError)?;
                continue;
            }
        }

        let url = match url::Url::parse(&format!("http://{}{}", server.server_addr(), req.url())) {
            Ok(url) => url,
            Err(_) => {
                req.respond(tiny_http::Response::empty(400).cors())
                    .map_err(Error::HttpError)?;
                continue;
            }
        };

        if url.path() != "/callback" {
            req.respond(tiny_http::Response::empty(404).cors())
                .map_err(Error::HttpError)?;
            continue;
        }

        let Some(serialized_response) = url.query_pairs().find(|(k, _)| k == "t").map(|(_, v)| v)
        else {
            req.respond(
                tiny_http::Response::from_string("missing `t` query param")
                    .with_status_code(400)
                    .cors(),
            )
            .map_err(Error::HttpError)?;
            continue;
        };

        let raw_response = match BASE64_STANDARD.decode(serialized_response.as_ref()) {
            Ok(v) => v,
            Err(e) => {
                p.println(format!("{e}"));
                req.respond(
                    tiny_http::Response::from_string("failed to deserialize response")
                        .with_status_code(400)
                        .cors(),
                )
                .map_err(Error::HttpError)?;
                continue;
            }
        };
        let response: AuthenticationResponse = match serde_json::from_slice(&raw_response) {
            Ok(v) => v,
            Err(_) => {
                req.respond(
                    tiny_http::Response::from_string("failed to deserialize response")
                        .with_status_code(400)
                        .cors(),
                )
                .map_err(Error::HttpError)?;
                continue;
            }
        };

        req.respond(tiny_http::Response::empty(200).cors())
            .map_err(Error::HttpError)?;
        break response;
    };

    p.set_message("Verifying login...");
    let start = Instant::now();
    let timeout = Duration::from_secs(10);
    let jwks = loop {
        p.inc(1);
        if start.elapsed() >= timeout {
            return Err(Error::JwksFetchTimeout);
        }

        match rx.try_recv() {
            Ok(v) => break v?,
            Err(mpsc::TryRecvError::Empty) => {
                std::thread::sleep(Duration::from_millis(10));
                continue;
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                // this shouldn't really happen
                unreachable!("JWKS fetch thread disconnected early");
            }
        }
    };

    let jwt = match Jwt::try_from(auth.access_token.clone()) {
        Ok(jwt) => jwt,
        Err(err) => {
            return Err(Error::InvalidJwt(err));
        }
    };

    match workos::verify_token(jwt, &jwks)? {
        workos::Status::NeedsRefresh => {
            // TODO: can this actually happen?
            unreachable!("Token needs to be refreshed immediately after generation");
        }
        workos::Status::Valid => {} // OK!
    };

    // TODO: store tokens and exit
    p.println(format!("{auth:?}"));

    p.finish_and_clear();

    Ok(())
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct AuthenticationResponse {
    user: User,
    organization_id: Option<String>,
    access_token: String,
    refresh_token: String,
    impersonator: Option<Impersonator>,
    authentication_method: Option<AuthenticationMethod>,
    sealed_session: Option<String>,
    oauth_tokens: Option<OauthTokens>,
}

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

#[derive(Debug, Clone, serde::Deserialize)]
struct Impersonator {
    email: String,
    reason: Option<String>,
}

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

#[derive(Debug, Clone, serde::Deserialize)]
struct OauthTokens {
    access_token: String,
    refresh_token: String,
    expires_at: u64,
    scopes: Vec<String>,
}
