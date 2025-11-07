use std::time::Duration;

use indicatif::ProgressBar;

pub use crate::callback_server::Error;
use crate::callback_server::OauthCallbackServer;
use crate::oauth::api::{AuthenticateWithCode, Pkce, send_async};
use crate::oauth::{self, Credentials};

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
    let mut login_hint = None;
    if !options.force_login {
        // NOTE: If the loading fails for whatever reason, we debug log the error
        // and have the user login again as if nothing happened.
        match oauth::load_credentials() {
            Ok(Some(credentials)) => {
                login_hint = Some(credentials.user().email.clone());
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
    let pkce = Pkce::new();
    let server = OauthCallbackServer::new(&pkce, login_hint.as_deref())?;
    p.inc(1);

    // 2. Open authorization URL in browser
    let login_url = server.get_login_url();

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
        webbrowser::open(login_url).ok(); // Ok to ignore error here. The user can just open the above url themselves.
    } else {
        p.println("Open the following page in your browser:");
        p.println(login_url);
    }
    p.inc(1);

    // 3. Wait for callback
    p.set_message("Waiting for browser…");
    let code = loop {
        match server.check_for_browser_response()? {
            None => {
                p.inc(1);
                std::thread::sleep(Duration::from_millis(10));
            }
            Some(response) => break response,
        }
    };

    // 4. Exchange code for credentials
    let auth = send_async(AuthenticateWithCode::new(&code, &pkce))
        .await
        .map_err(|err| Error::Generic(err.into()))?;

    // 5. Store credentials
    let credentials = Credentials::from_auth_response(auth.into())?.ensure_stored()?;

    p.finish_and_clear();

    println!(
        "Success! You are now logged in as {}",
        credentials.user().email
    );
    println!("Rerun will automatically use the credentials stored on your machine.");

    Ok(())
}
