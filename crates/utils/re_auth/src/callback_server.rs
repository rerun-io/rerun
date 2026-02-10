use std::borrow::Cow;

use uuid::Uuid;

use crate::oauth::CredentialsStoreError;
use crate::oauth::api::{Pkce, authorization_url};
use crate::token::JwtDecodeError;

pub struct OauthCallbackServer {
    server: tiny_http::Server,

    state: String,
    auth_url: String,
}

/// This is a range of ports that's allowlisted on the authentication provider side.
const PORT_RANGE: std::ops::RangeInclusive<u16> = 17340..=17349;

impl OauthCallbackServer {
    pub fn new(pkce: &Pkce) -> Result<Self, Error> {
        let server = PORT_RANGE
            .map(|port| tiny_http::Server::http(format!("127.0.0.1:{port}")))
            .find_map(Result::ok)
            .ok_or_else(|| {
                Error::Bind(std::io::Error::new(
                    std::io::ErrorKind::AddrInUse,
                    format!("no free port found in range {PORT_RANGE:?}"),
                ))
            })?;

        let state: String = Uuid::new_v4().to_string();

        let redirect_uri = format!(
            "http://{server_addr}/logged-in",
            server_addr = server.server_addr()
        );
        let auth_url = authorization_url(&redirect_uri, &state, pkce);

        Ok(Self {
            server,
            state,
            auth_url,
        })
    }

    /// Simple web server waiting for a request from the browser to `/callback`,
    /// which provides us with the token payload.
    pub fn check_for_browser_response(&self) -> Result<Option<String>, Error> {
        let Some(req) = self.server.try_recv().map_err(Error::Http)? else {
            return Ok(None);
        };

        if let Some(res) = handle_other_requests(&req) {
            req.respond(res).map_err(Error::Http)?;
            return Ok(None);
        }

        handle_auth_request(&self.server, req, &self.state)
    }

    pub fn get_login_url(&self) -> &str {
        &self.auth_url
    }
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

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to bind listener: {0}")]
    Bind(std::io::Error),

    #[error("HTTP server error: {0}")]
    Http(std::io::Error),

    #[error(transparent)]
    Generic(#[from] Box<dyn std::error::Error + Send + Sync + 'static>),

    #[error("{0}")]
    MalformedToken(#[from] JwtDecodeError),

    #[error("{0}")]
    Store(#[from] CredentialsStoreError),
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

// Handles `/logged-in?code=<code>&state=<state>`
fn handle_auth_request(
    server: &tiny_http::Server,
    req: tiny_http::Request,
    stored_state: &str,
) -> Result<Option<String>, Error> {
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
    let Some(code) = get_query_param(&url, "code") else {
        status_page_response(req, "Missing query param <code>code</code>")?;
        return Ok(None);
    };
    let Some(state) = get_query_param(&url, "state") else {
        status_page_response(req, "Missing query param <code>state</code>")?;
        return Ok(None);
    };

    if state != stored_state {
        status_page_response(req, "Something went wrong: invalid <code>state</code>")?;
        return Ok(None);
    }

    status_page_response(req, "Success! You can close this page now.")?;

    Ok(Some(code.into_owned()))
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
