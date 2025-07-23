use clap::{Parser, Subcommand};

#[derive(Debug, Clone, Subcommand)]
pub enum AuthCommands {
    /// Log into Rerun.
    Login(LoginCommand),
}

#[derive(Debug, Clone, Parser)]
pub struct LoginCommand {
    // TODO: default should be `rerun.io` prod
    #[clap(
        long,
        default_value = "https://landing-git-jan-login-page-rerun.vercel.app/login"
    )]
    login_url: String,

    // Double-negative because it's an opt-out flag.
    /// Post a link instead of directly opening in the browser.
    #[clap(long, default_value = "false")]
    no_open_browser: bool,
}

impl AuthCommands {
    pub fn run(&self) -> Result<(), re_auth::cli::Error> {
        match self {
            AuthCommands::Login(args) => {
                re_auth::cli::login(&args.login_url, !args.no_open_browser)
            }
        }
    }
}
