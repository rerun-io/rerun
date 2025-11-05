use std::{borrow::Cow, collections::HashMap, time::Duration};

use base64::prelude::*;
use indicatif::ProgressBar;
use rand::{Rng as _, SeedableRng as _, rngs::StdRng};

use crate::oauth::{self, Credentials, CredentialsStoreError, MalformedTokenError};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to bind listener: {0}")]
    Bind(std::io::Error),

    #[error("HTTP server error: {0}")]
    Http(std::io::Error),

    #[error(transparent)]
    Generic(#[from] Box<dyn std::error::Error + Send + Sync + 'static>),

    #[error("{0}")]
    MalformedToken(#[from] MalformedTokenError),

    #[error("{0}")]
    Store(#[from] CredentialsStoreError),
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

const DEFAULT_LOGIN_URL: &str = "https://rerun.io/login/v2";

pub struct LoginOptions<'a> {
    pub login_page_url: Option<&'a str>,
    pub open_browser: bool,
    pub force_login: bool,
}

#[derive(Debug, thiserror::Error)]
#[error("No credentials are stored on your machine, run `rerun auth login` first")]
struct NoCredentialsError;

#[derive(Debug, thiserror::Error)]
#[error("Your session ended due to inactivity, run `rerun auth login` first")]
struct ExpiredCredentialsError;

/// Prints the token to stdout
pub async fn token() -> Result<(), Error> {
    match oauth::load_and_refresh_credentials().await {
        Ok(Some(credentials)) => {
            println!("{}", credentials.access_token().as_str());
            Ok(())
        }

        Ok(None) => Err(Error::Generic(NoCredentialsError.into())),

        Err(err) => {
            re_log::debug!("invalid credentials: {err}");
            Err(Error::Generic(Box::new(ExpiredCredentialsError)))
        }
    }
}

/// Login to Rerun using Authorization Code flow.
///
/// This first checks if valid credentials already exist locally,
/// and doesn't perform the login flow if so, unless `options.force_login` is set to `true`.
pub async fn login(options: LoginOptions<'_>) -> Result<(), Error> {
    if !options.force_login {
        // NOTE: If the loading fails for whatever reason, we debug log the error
        // and have the user login again as if nothing happened.
        match oauth::load_credentials() {
            Ok(Some(credentials)) => {
                match oauth::refresh_credentials(credentials).await {
                    Ok(credentials) => {
                        println!("You're already logged in as: {}", credentials.user().email);
                        println!("Note: We've refreshed your credentials.");
                        println!("Note: Run `rerun auth login --force` to login again.");
                        return Ok(());
                    }
                    Err(err) => {
                        re_log::debug!("refreshing credentials failed: {err}");
                        // Credentials are bad, login again.
                        // fallthrough
                    }
                }
            }

            Ok(None) => {
                // No credentials yet, login as usual.
                // fallthrough
            }

            Err(err) => {
                re_log::debug!(
                    "validating credentials failed, logging user in again anyway. reason: {err}"
                );
                // fallthrough
            }
        }
    }

    let p = ProgressBar::new_spinner();

    // Login process:

    // 1. Start web server listening for token
    let server = tiny_http::Server::http("127.0.0.1:0")?;
    p.inc(1);

    // 2. Open authorization URL in browser
    let nonce: u128 = StdRng::from_os_rng().random();
    // User is sent to this URL after logging in:
    let logged_in_url = format!(
        "http://{addr}/logged-in?n={n}",
        addr = server.server_addr(),
        n = BASE64_URL_SAFE.encode(nonce.to_le_bytes()),
    );
    // This is the URL the user should open to log in:
    let login_url = format!(
        "{login_page_url}?r={r}",
        login_page_url = options.login_page_url.unwrap_or(DEFAULT_LOGIN_URL),
        r = BASE64_URL_SAFE.encode(logged_in_url.as_bytes()),
    );

    // Once the user opens the link, they are redirected to the login UI.
    // If they were already logged in, it will immediately redirect them
    // to the login callback with an authorization code.
    // That code is then sent by our callback page back to the web server here.
    if options.open_browser {
        p.println("Opening login page in your browser.");
        p.println("Once you've logged in, the process will continue here.");
        p.println(format!(
            "Alternatively, manually open this url: {login_url}"
        ));
        webbrowser::open(&login_url).ok(); // Ok to ignore error here. The user can just open the above url themselves.
    } else {
        p.println("Open the following page in your browser:");
        p.println(&login_url);
    }
    p.inc(1);

    // 3. Wait for callback
    p.set_message("Waiting for browser…");
    let auth = wait_for_browser_response(&server, nonce, &p)?;

    // 4. Deserialize credentials
    let credentials = Credentials::from_auth_response(auth.into())?;
    let credentials = credentials.ensure_stored()?;

    p.finish_and_clear();

    println!(
        "Success! You are now logged in as {}",
        credentials.user().email
    );
    println!("Rerun will automatically use the credentials stored on your machine.");

    Ok(())
}

/// Simple web server waiting for a request from the browser to `/callback`,
/// which provides us with the token payload.
fn wait_for_browser_response(
    server: &tiny_http::Server,
    nonce: u128,
    p: &ProgressBar,
) -> Result<AuthenticationResponse, Error> {
    loop {
        p.inc(1);
        let Some(req) = server
            .recv_timeout(Duration::from_millis(100))
            .map_err(Error::Http)?
        else {
            continue;
        };

        if let Some(res) = handle_other_requests(&req) {
            req.respond(res).map_err(Error::Http)?;
            continue;
        }

        let Some(res) = handle_auth_request(server, req, nonce)? else {
            continue;
        };

        return Ok(res);
    }
}

/// Handles CORS (Options) and HEAD requests
fn handle_other_requests(req: &tiny_http::Request) -> Option<tiny_http::Response<std::io::Empty>> {
    match req.method() {
        tiny_http::Method::Get => None,
        tiny_http::Method::Options => Some(
            tiny_http::Response::empty(204)
                .cors()
                .with_header(header(b"Allow", b"GET, HEAD, OPTIONS")),
        ),
        tiny_http::Method::Head => Some(tiny_http::Response::empty(200).cors()),
        _ => Some(
            tiny_http::Response::empty(405)
                .cors()
                .with_header(header(b"Allow", b"GET, HEAD, OPTIONS")),
        ),
    }
}

// Handles `/logged-in?t=<base64-encoded token payload>&n=<base64-encoded nonce>`
fn handle_auth_request(
    server: &tiny_http::Server,
    req: tiny_http::Request,
    nonce: u128,
) -> Result<Option<AuthenticationResponse>, Error> {
    // Parse and check the URL pathname
    let Ok(url) = url::Url::parse(&format!("http://{}{}", server.server_addr(), req.url())) else {
        req.respond(tiny_http::Response::empty(400).cors())
            .map_err(Error::Http)?;
        return Ok(None);
    };

    if url.path() != "/logged-in" {
        req.respond(tiny_http::Response::empty(404).cors())
            .map_err(Error::Http)?;
        return Ok(None);
    }

    // get required query params
    let Some(serialized_response) = get_query_param(&url, "t") else {
        status_page_response(req, "Missing query param <code>t</code>")?;
        return Ok(None);
    };
    let Some(serialized_nonce) = get_query_param(&url, "n") else {
        status_page_response(req, "Missing query param <code>n</code>")?;
        return Ok(None);
    };

    // decode base64
    let raw_response = match BASE64_URL_SAFE.decode(serialized_response.as_ref()) {
        Ok(v) => v,
        Err(err) => {
            status_page_response(req, format!("Failed to deserialize response: {err}"))?;
            return Ok(None);
        }
    };
    let received_nonce_bytes = match BASE64_URL_SAFE.decode(serialized_nonce.as_ref()) {
        Ok(v) => v,
        Err(err) => {
            status_page_response(req, format!("Failed to deserialize nonce: {err}"))?;
            return Ok(None);
        }
    };

    // deserialize
    let response: AuthenticationResponse = match serde_json::from_slice(&raw_response) {
        Ok(v) => v,
        Err(err) => {
            status_page_response(req, format!("Failed to deserialize response: {err}"))?;
            return Ok(None);
        }
    };
    let received_nonce: u128 = match received_nonce_bytes.as_slice().try_into() {
        Ok(nonce_bytes) => u128::from_le_bytes(nonce_bytes),
        Err(err) => {
            status_page_response(req, format!("Failed to deserialize nonce: {err}"))?;
            return Ok(None);
        }
    };

    if nonce != received_nonce {
        status_page_response(
            req,
            "Request expired, try running <code>rerun auth login</code> again.",
        )?;
        return Ok(None);
    }

    status_page_response(req, "Success! You can close this page now.")?;

    Ok(Some(response))
}

fn status_page_response(req: tiny_http::Request, message: impl Into<String>) -> Result<(), Error> {
    let message: String = message.into();
    let data = include_str!("./status_page.html").replace("$MESSAGE$", message.as_str());
    req.respond(
        tiny_http::Response::from_data(data)
            .with_header(header(b"Content-Type", b"text/html; charset=utf-8"))
            .cors(),
    )
    .map_err(Error::Http)
}

fn get_query_param<'a>(url: &'a url::Url, key: &str) -> Option<Cow<'a, str>> {
    url.query_pairs().find(|(k, _)| k == key).map(|(_, v)| v)
}

#[expect(dead_code)] // fields may become used at some point in the near future
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

impl From<AuthenticationResponse> for oauth::api::AuthenticationResponse {
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

impl From<User> for oauth::User {
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
