use std::time::Duration;

use indicatif::ProgressBar;
use re_error::try_println;

pub use crate::callback_server::Error;
use crate::oauth::api::{GenerateToken, send_async};
use crate::oauth::login_flow::OauthLoginFlowState;
use crate::{OauthLoginFlow, Permission, oauth};

pub struct LogoutOptions {
    pub open_browser: bool,
}

pub struct LoginOptions {
    pub open_browser: bool,
    pub force_login: bool,

    /// If set, switch to this `WorkOS` organization after login.
    pub org_id: Option<String>,
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
            try_println!("{}", credentials.access_token().as_str()).map_err(Error::Stdout)
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
pub async fn login(options: LoginOptions) -> Result<(), Error> {
    // Login process:

    // 1. Start web server listening for token
    let mut credentials = match OauthLoginFlow::init(options.force_login).await? {
        OauthLoginFlowState::AlreadyLoggedIn(credentials) => {
            if options.org_id.is_none() {
                try_println!("You're already logged in as: {}", credentials.user().email)
                    .map_err(Error::Stdout)?;
                try_println!("Note: We've refreshed your credentials.").map_err(Error::Stdout)?;
                try_println!("Note: Run `rerun auth login --force` to login again.")
                    .map_err(Error::Stdout)?;
                return Ok(());
            }
            *credentials
        }
        OauthLoginFlowState::LoginFlowStarted(login_flow) => {
            let progress_bar = ProgressBar::new_spinner();

            // 2. Open authorization URL in browser
            let login_url = login_flow.get_login_url();
            if options.open_browser {
                progress_bar.println("Opening login page in your browser.");
                progress_bar.println("Once you've logged in, the process will continue here.");
                progress_bar.println(format!(
                    "Alternatively, manually open this url: {login_url}"
                ));
                webbrowser::open(login_url).ok(); // Ok to ignore error here. The user can just open the above url themselves.
            } else {
                progress_bar.println("Open the following page in your browser:");
                progress_bar.println(login_url);
            }
            progress_bar.inc(1);

            // 3. Wait for login to finish
            progress_bar.set_message("Waiting for browser…");
            let credentials = loop {
                if let Some(code) = login_flow.poll().await? {
                    break code;
                }
                progress_bar.inc(1);
                std::thread::sleep(Duration::from_millis(10));
            };

            progress_bar.finish_and_clear();
            credentials
        }
    };

    // 4. If an org was specified, switch to it via a refresh
    if let Some(org_id) = &options.org_id {
        credentials = oauth::refresh_credentials_with_org(credentials, Some(org_id))
            .await
            .map_err(|err| Error::Generic(err.into()))?;
    }

    try_println!(
        "Success! You are now logged in as {}",
        credentials.user().email
    )
    .map_err(Error::Stdout)?;
    try_println!("Rerun will automatically use the credentials stored on your machine.")
        .map_err(Error::Stdout)?;

    Ok(())
}

/// Log out of Rerun by clearing stored credentials.
pub fn logout(options: &LogoutOptions) -> Result<(), Error> {
    match crate::oauth::clear_credentials(None) {
        Ok(Some(outcome)) => {
            if options.open_browser {
                try_println!("Opening browser to end your session…").map_err(Error::Stdout)?;
                webbrowser::open(&outcome.logout_url).ok();
            } else {
                try_println!("Open the following URL in your browser to end your session:")
                    .map_err(Error::Stdout)?;
                try_println!("{}", outcome.logout_url).map_err(Error::Stdout)?;
            }
            try_println!("You have been logged out.").map_err(Error::Stdout)?;

            // Wait for the callback server to serve the "logged out" page
            // before the process exits.
            if let Some(handle) = outcome.server_handle {
                handle.join().ok();
            }

            Ok(())
        }
        Ok(None) => {
            try_println!("No credentials found. You are already logged out.").map_err(Error::Stdout)
        }
        Err(err) => Err(Error::Generic(err.into())),
    }
}

pub struct GenerateTokenOptions {
    pub server: url::Origin,
    pub expiration: jiff::Span,
    pub permission: Permission,
}

pub async fn generate_token(options: GenerateTokenOptions) -> Result<(), Error> {
    let GenerateTokenOptions {
        server,
        expiration,
        permission,
    } = options;

    let credentials = match oauth::load_and_refresh_credentials().await {
        Ok(Some(credentials)) => credentials,

        Ok(None) => return Err(Error::Generic(NoCredentialsError.into())),

        Err(err) => {
            re_log::debug!("invalid credentials: {err}");
            return Err(Error::Generic(Box::new(ExpiredCredentialsError)));
        }
    };

    let jwt = credentials.access_token().jwt();
    let server_url =
        url::Url::parse(&server.ascii_serialization()).map_err(|err| Error::Generic(err.into()))?;
    let server_host = server_url
        .host_str()
        .ok_or_else(|| Error::Generic("server URL has no host".into()))?;
    let token = jwt
        .for_host(server_host)
        .map_err(|err| Error::Generic(err.into()))?;

    let res = send_async(GenerateToken {
        server,
        token,
        expiration,
        permission,
    })
    .await
    .map_err(|err| Error::Generic(err.into()))?;

    try_println!("{}", res.token).map_err(Error::Stdout)
}
