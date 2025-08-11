use std::{collections::HashMap, time::Duration};

use base64::prelude::*;

use crate::workos::{self, AuthContext, Credentials};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to bind listener: {0}")]
    BindError(std::io::Error),

    #[error("HTTP server error: {0}")]
    HttpError(std::io::Error),

    #[error("failed to open browser: {0}")]
    WebBrowser(std::io::Error),

    #[error("failed to load context: {0}")]
    Context(#[from] workos::ContextLoadError),

    #[error("failed to verify credentials: {0}")]
    Credentials(#[from] workos::CredentialsError),

    #[error("failed to store credentials: {0}")]
    Store(#[from] workos::CredentialsStoreError),

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

pub struct LoginOptions<'a> {
    pub login_page_url: &'a str,
    pub open_browser: bool,
    pub force_login: bool,
}

/// Login to Rerun using Authorization Code flow.
///
/// This first checks if valid credentials already exist locally,
/// and doesn't perform the login flow if so, unless `options.force_login` is set to `true`.
pub async fn login(context: &AuthContext, options: LoginOptions<'_>) -> Result<(), Error> {
    if !options.force_login {
        // NOTE: If the loading fails for whatever reason, we debug log the error
        // and have the user login again as if nothing happened.
        match workos::Credentials::load() {
            Ok(Some(mut credentials)) => {
                // Try to do a refresh to validate the token
                // TODO(jan): call redap `/verify-token` instead
                match credentials.refresh(context).await {
                    Ok(_) => {
                        println!(
                            "You're already logged in as {}, and your credentials are valid.",
                            credentials.user().email
                        );
                        println!(
                            "Use `rerun auth login --force` if you'd like to login again anyway."
                        );
                        println!("Note: We've refreshed your credentials.");
                        credentials.store()?;
                        return Ok(());
                    }
                    Err(err) => {
                        println!(
                            "Note: Credentials were already present on the machine, but validating them failed:\n    {err}"
                        );
                        println!(
                            "This is normal if you haven't used your credentials in some time."
                        );
                        // Credentials are bad, fallthrough to login path.
                    }
                }
            }

            Ok(None) => {
                // No credentials yet, login as usual.
            }

            Err(err) => {
                re_log::debug!(
                    "validating credentials failed, logging user in again anyway. reason: {err}"
                );
                // fallthrough
            }
        }
    }

    let p = indicatif::ProgressBar::new_spinner();

    // Login process:

    // 1. Start web server listening for token
    let server = tiny_http::Server::http("127.0.0.1:0")?;
    p.inc(1);

    // 2. Open authorization URL in browser
    let callback_url = format!("http://{}/callback", server.server_addr());
    let login_url = format!(
        "{login_page_url}?r={r}",
        login_page_url = options.login_page_url,
        r = BASE64_URL_SAFE_NO_PAD.encode(callback_url.as_bytes()),
    );

    // Once the user opens the link, they are redirected to the login UI.
    // If they were already logged in, it will immediately redirect them
    // to the login callback with an authorization code.
    // That code is then sent by our callback page back to the web server here.
    if options.open_browser {
        p.println("Opening login page in your browser.");
        p.println("Once you've logged in, the process will continue here.");
        webbrowser::open(&login_url).map_err(Error::WebBrowser)?;
    } else {
        p.println("Open the following page in your browser:");
        p.println(&login_url);
    }
    p.inc(1);

    // 3. Wait for callback
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
            Err(err) => {
                let err = format!("failed to deserialize response: {err}");
                p.println(&err);
                req.respond(
                    tiny_http::Response::from_string(err)
                        .with_status_code(400)
                        .cors(),
                )
                .map_err(Error::HttpError)?;
                continue;
            }
        };
        let response: AuthenticationResponse = match serde_json::from_slice(&raw_response) {
            Ok(v) => v,
            Err(err) => {
                let err = format!("failed to deserialize response: {err}");
                p.println(&err);
                req.respond(
                    tiny_http::Response::from_string(err)
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

    // 4. Verify credentials
    p.set_message("Verifying login...");
    let credentials = Credentials::verify_auth_response(context, auth.into())?;

    // 5. Store credentials
    credentials.store()?;

    p.finish_and_clear();

    println!(
        "Success! You are now logged in as {}",
        credentials.user().email
    );
    println!("Rerun will automatically use the credentials stored on your machine.");

    Ok(())
}

#[allow(dead_code)] // fields may become used at some point in the near future
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

impl From<AuthenticationResponse> for workos::api::AuthenticationResponse {
    fn from(value: AuthenticationResponse) -> Self {
        Self {
            user: value.user.into(),
            organization_id: value.organization_id,
            access_token: value.access_token,
            refresh_token: value.refresh_token,
        }
    }
}

#[allow(dead_code)] // fields may become used at some point in the near future
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

impl From<User> for workos::User {
    fn from(value: User) -> Self {
        Self {
            id: value.id,
            email: value.email,
            metadata: value.metadata,
        }
    }
}

#[allow(dead_code)] // fields may become used at some point in the near future
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

#[allow(dead_code)] // fields may become used at some point in the near future
#[derive(Debug, Clone, serde::Deserialize)]
struct OauthTokens {
    access_token: String,
    refresh_token: String,
    expires_at: u64,
    scopes: Vec<String>,
}
