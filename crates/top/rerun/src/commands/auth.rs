use clap::{Parser, Subcommand};

#[derive(Debug, Clone, Subcommand)]
pub enum AuthCommands {
    /// Log into Rerun.
    Login(LoginCommand),
}

#[derive(Debug, Clone, Parser)]
pub struct LoginCommand {
    #[clap(default_value = "http://localhost:6170/login")]
    login_url: String,
}

impl AuthCommands {
    pub fn run(&self) -> Result<(), re_auth::cli::Error> {
        match self {
            AuthCommands::Login(args) => re_auth::cli::login(&args.login_url),
        }
    }
}
