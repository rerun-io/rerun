use clap::Subcommand;

// ---

#[derive(Debug, Clone, Subcommand)]
pub enum AnalyticsCommands {
    /// Prints extra information about analytics.
    Details,

    /// Deletes everything related to analytics.
    ///
    /// This will remove all pending data that hasn't yet been sent to our servers, as well as
    /// reset your analytics ID.
    Clear,

    /// Associate an email address with the current user.
    Email { email: String },

    /// Enable analytics.
    Enable,

    /// Disable analytics.
    Disable,

    /// Prints the current configuration.
    Config,
}

impl AnalyticsCommands {
    pub fn run(&self) -> Result<(), re_analytics::cli::CliError> {
        let build_info = re_build_info::build_info!();
        match self {
            #[expect(clippy::unit_arg)]
            Self::Details => Ok(re_analytics::cli::print_details(
                &build_info.git_hash_or_tag(),
            )),
            Self::Clear => re_analytics::cli::clear(),
            Self::Email { email } => {
                re_analytics::cli::set([("email".to_owned(), email.clone().into())])
            }
            Self::Enable => re_analytics::cli::opt(true),
            Self::Disable => re_analytics::cli::opt(false),
            Self::Config => re_analytics::cli::print_config(),
        }
    }
}
