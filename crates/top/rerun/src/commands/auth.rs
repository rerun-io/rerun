use clap::{Parser, Subcommand};

#[derive(Debug, Clone, Subcommand)]
pub enum AuthCommands {
    /// Log into Rerun.
    ///
    /// This command opens a page in your default browser, allowing you
    /// to log in to the Rerun Data Platform.
    ///
    /// Once you've logged in, your credentials are stored on your machine.
    ///
    /// To sign up, contact us through the form linked at <https://rerun.io/#open-source-vs-commercial>.
    Login(LoginCommand),

    /// Retrieve the stored access token.
    ///
    /// The access token is part of the credentials produced by `rerun auth login`,
    /// and is used to authorize requests to the Rerun Data Platform.
    Token(TokenCommand),

    /// Generate a fresh access token.
    ///
    /// You can use this token to authorize requests to the Rerun Data Platform.
    ///
    /// It's closer to an API key than an access token, as it can be revoked before
    /// it expires.
    GenerateToken(GenerateTokenCommand),
}

#[derive(Debug, Clone, Parser)]
pub struct LoginCommand {
    // Double-negative because it's an opt-out flag.
    /// Post a link instead of directly opening in the browser.
    #[clap(long, default_value = "false")]
    no_open_browser: bool,

    /// Trigger the full login flow even if valid credentials already exist.
    #[clap(long, default_value = "false")]
    force: bool,
}

#[derive(Debug, Clone, Parser)]
pub struct TokenCommand {}

#[derive(Debug, Clone, Parser)]
pub struct GenerateTokenCommand {
    /// Origin of the server to request the token from.
    #[clap(long)]
    server: String,

    /// Duration of the token, either in:
    /// - "human time", e.g. `1 day`, or
    /// - ISO 8601 duration format, e.g. `P1D`.
    #[clap(long)]
    expiration: String,

    /// Which permission the token should have.
    ///
    /// [`read`, `read-write`]
    #[clap(long, default_value = "read")]
    permission: String,
}

impl AuthCommands {
    pub fn run(self, runtime: &tokio::runtime::Handle) -> Result<(), re_auth::cli::Error> {
        match self {
            Self::Login(args) => {
                let options = re_auth::cli::LoginOptions {
                    open_browser: !args.no_open_browser,
                    force_login: args.force,
                };
                runtime.block_on(re_auth::cli::login(options))
            }

            Self::Token(_) => runtime.block_on(re_auth::cli::token()),

            Self::GenerateToken(args) => {
                let server = parse_http_or_rerun_uri(&args.server)
                    .map_err(|err| re_auth::cli::Error::Generic(err.into()))?;
                let expiration = args
                    .expiration
                    .parse::<jiff::Span>()
                    .map_err(|err| re_auth::cli::Error::Generic(err.into()))?;
                let permission = args
                    .permission
                    .parse::<re_auth::Permission>()
                    .map_err(|err| re_auth::cli::Error::Generic(err.into()))?;
                let options = re_auth::cli::GenerateTokenOptions {
                    server,
                    expiration,
                    permission,
                };
                runtime.block_on(re_auth::cli::generate_token(options))
            }
        }
    }
}

#[derive(Debug)]
struct InvalidUri;

impl std::fmt::Display for InvalidUri {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid uri, expected one of: `rerun://`, `rerun+http://`, `rerun+https://`, `http://`, `https://`"
        )
    }
}

impl std::error::Error for InvalidUri {}

fn parse_http_or_rerun_uri(s: &str) -> Result<url::Origin, InvalidUri> {
    if let Ok(url) = s.parse::<re_uri::Origin>() {
        Ok(url::Url::parse(&url.as_url()).expect("valid url").origin())
    } else if let Ok(url) = url::Url::parse(s) {
        Ok(url.origin())
    } else {
        Err(InvalidUri)
    }
}
