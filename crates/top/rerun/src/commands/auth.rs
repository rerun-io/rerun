use clap::{Parser, Subcommand};

#[derive(Debug, Clone, Subcommand)]
pub enum AuthCommands {
    /// Log into Rerun.
    ///
    /// This command opens a page in your default browser, allowing you
    /// to log in to the Rerun data platform.
    ///
    /// Once you've logged in, your credentials are stored on your machine.
    ///
    /// To sign up, contact us through the form linked at <https://rerun.io/#open-source-vs-commercial>.
    Login(LoginCommand),

    /// Retrieve the stored access token.
    ///
    /// The access token is part of the credentials produced by `rerun auth login`,
    /// and is used to authorize requests to the Rerun data platform.
    Token(TokenCommand),
}

#[derive(Debug, Clone, Parser)]
pub struct LoginCommand {
    #[clap(long)]
    login_url: Option<String>,

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

impl AuthCommands {
    pub fn run(&self, runtime: &tokio::runtime::Handle) -> Result<(), re_auth::cli::Error> {
        match self {
            Self::Login(args) => {
                let options = re_auth::cli::LoginOptions {
                    login_page_url: args.login_url.as_deref(),
                    open_browser: !args.no_open_browser,
                    force_login: args.force,
                };
                runtime.block_on(re_auth::cli::login(options))
            }

            Self::Token(_) => runtime.block_on(re_auth::cli::token()),
        }
    }
}
