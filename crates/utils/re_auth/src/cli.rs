use std::time::Duration;

use indicatif::ProgressBar;

pub use crate::callback_server::Error;
use crate::oauth::api::{GenerateToken, send_async};
use crate::oauth::login_flow::OauthLoginFlowState;
use crate::{OauthLoginFlow, oauth};

pub struct LoginOptions {
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
pub async fn login(options: LoginOptions) -> Result<(), Error> {
    // Login process:

    // 1. Start web server listening for token
    let login_flow = match OauthLoginFlow::init(options.force_login).await? {
        OauthLoginFlowState::AlreadyLoggedIn(credentials) => {
            println!("You're already logged in as: {}", credentials.user().email);
            println!("Note: We've refreshed your credentials.");
            println!("Note: Run `rerun auth login --force` to login again.");
            return Ok(());
        }
        OauthLoginFlowState::LoginFlowStarted(login_flow) => login_flow,
    };

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

    println!(
        "Success! You are now logged in as {}",
        credentials.user().email
    );
    println!("Rerun will automatically use the credentials stored on your machine.");

    Ok(())
}

pub struct GenerateTokenOptions {
    pub server: url::Origin,
    pub expiration: jiff::Span,
}

pub async fn generate_token(options: GenerateTokenOptions) -> Result<(), Error> {
    let credentials = match oauth::load_and_refresh_credentials().await {
        Ok(Some(credentials)) => credentials,

        Ok(None) => return Err(Error::Generic(NoCredentialsError.into())),

        Err(err) => {
            re_log::debug!("invalid credentials: {err}");
            return Err(Error::Generic(Box::new(ExpiredCredentialsError)));
        }
    };

    let res = send_async(GenerateToken {
        server: options.server,
        token: credentials.access_token().as_str(),
        expiration: options.expiration,
    })
    .await
    .map_err(|err| Error::Generic(err.into()))?;

    println!("{}", res.token);

    Ok(())
}
