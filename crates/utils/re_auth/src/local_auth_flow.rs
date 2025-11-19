use std::mem;
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;

use crate::callback_server::Error;
use crate::callback_server::OauthCallbackServer;
use crate::oauth::Credentials;
use crate::oauth::api::AuthenticateWithCode;
use crate::oauth::api::Pkce;
use crate::oauth::api::send_async;

#[derive(Default)]
enum OauthLoginFlowState {
    InProgress(OauthLoginInProgress),
    Finished(Result<Credentials, Error>),

    #[default]
    Invalid,
}

struct OauthLoginInProgress {
    server: OauthCallbackServer,
    pkce: Pkce,
    login_url: String,
}

pub struct OauthLoginFlow {
    state: Arc<Mutex<OauthLoginFlowState>>,
}

impl OauthLoginFlow {
    pub fn new() -> Result<Self, Error> {
        // let mut login_hint = None;
        // if !options.force_login {
        //     // NOTE: If the loading fails for whatever reason, we debug log the error
        //     // and have the user login again as if nothing happened.
        //     match oauth::load_credentials() {
        //         Ok(Some(credentials)) => {
        //             login_hint = Some(credentials.user().email.clone());
        //             match oauth::refresh_credentials(credentials).await {
        //                 Ok(credentials) => {
        //                     println!("You're already logged in as: {}", credentials.user().email);
        //                     println!("Note: We've refreshed your credentials.");
        //                     println!("Note: Run `rerun auth login --force` to login again.");
        //                     return Ok(());
        //                 }
        //                 Err(err) => {
        //                     re_log::debug!("refreshing credentials failed: {err}");
        //                     // Credentials are bad, login again.
        //                     // fallthrough
        //                 }
        //             }
        //         }

        //         Ok(None) => {
        //             // No credentials yet, login as usual.
        //             // fallthrough
        //         }

        //         Err(err) => {
        //             re_log::debug!(
        //                 "validating credentials failed, logging user in again anyway. reason: {err}"
        //             );
        //             // fallthrough
        //         }
        //     }
        // }

        // Login process:

        println!("OauthLoginFlow::new starting server"); // TODO:

        // 1. Start web server listening for token
        let pkce = Pkce::new();
        let server = OauthCallbackServer::new(&pkce, None)?; // TODO: login_hint

        println!("OauthLoginFlow::new {}", server.get_login_url()); // TODO:

        let state = Arc::new(Mutex::new(OauthLoginFlowState::InProgress(
            OauthLoginInProgress {
                login_url: server.get_login_url().to_owned(),
                server,
                pkce,
            },
        )));

        // 2. Open authorization URL in browser

        // Once the user opens the link, they are redirected to the login UI.
        // If they were already logged in, it will immediately redirect them
        // to the login callback with an authorization code.
        // That code is then sent by our callback page back to the web server here.
        // if options.open_browser {
        //     p.println("Opening login page in your browser.");
        //     p.println("Once you've logged in, the process will continue here.");
        //     p.println(format!(
        //         "Alternatively, manually open this url: {login_url}"
        //     ));
        //     webbrowser::open(login_url).ok(); // Ok to ignore error here. The user can just open the above url themselves.
        // } else {
        //     p.println("Open the following page in your browser:");
        //     p.println(login_url);
        // }
        // p.inc(1);

        {
            let state = Arc::clone(&state);
            tokio::spawn(async move {
                Self::wait_for_credentials(state);
            });
        }

        Ok(Self { state })
    }

    async fn wait_for_credentials(state: Arc<Mutex<OauthLoginFlowState>>) {
        // 3. Wait for callback
        // p.set_message("Waiting for browser…");
        println!("OauthLoginFlow::wait_for_credentials"); // TODO:

        let code = loop {
            let response = {
                let state = state.lock();
                let OauthLoginFlowState::InProgress(in_progress) = &*state else {
                    re_log::error!("OAuth login flow ended unexpectedly.");
                    return;
                };
                in_progress.server.check_for_browser_response()
            };

            match response {
                Ok(Some(response)) => break response,
                Err(err) => {
                    re_log::error!("checking for browser response failed: {err:?}");
                    *state.lock() = OauthLoginFlowState::Finished(Err(err));
                    return;
                }
                Ok(None) => {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            };
        };

        println!("OauthLoginFlow::wait_for_credentials code: {code}"); // TODO:

        let mut state = state.lock();
        let OauthLoginFlowState::InProgress(in_progress) = &*state else {
            re_log::error!("OAuth login flow ended unexpectedly.");
            return;
        };
        let credentials = Self::exchange_code_for_credentials(&code, in_progress).await;
        *state = OauthLoginFlowState::Finished(credentials);
    }

    async fn exchange_code_for_credentials(
        code: &str,
        state: &OauthLoginInProgress,
    ) -> Result<Credentials, Error> {
        // 4. Exchange code for credentials
        let auth = send_async(AuthenticateWithCode::new(&code, &state.pkce))
            .await
            .map_err(|err| Error::Generic(err.into()))?;

        // 5. Store credentials
        let credentials = Credentials::from_auth_response(auth.into())?.ensure_stored()?;

        // p.finish_and_clear();

        println!(
            "Success! You are now logged in as {}",
            credentials.user().email
        );
        println!("Rerun will automatically use the credentials stored on your machine."); // TODO:

        Ok(credentials)
    }

    pub async fn get_credentials(&self) -> Option<Result<Credentials, Error>> {
        let mut state = self.state.lock();
        if matches!(&*state, OauthLoginFlowState::InProgress(_)) {
            return None;
        }
        match std::mem::take(&mut *state) {
            OauthLoginFlowState::Finished(result) => Some(result),
            _ => None,
        }

        // if let OauthLoginFlowState::Finished(_) = &*state {
        //     // let b = mem::replace(&mut *state, OauthLoginFlowState::Invalid);
        //     // match b {
        //     //     OauthLoginFlowState::Finished(result) => return Some(result),
        //     //     _ => unreachable!(),
        //     // }
        // }
        // None

        // // 3. Wait for callback
        // // p.set_message("Waiting for browser…");
        // println!("OauthLoginFlow::get_credentials"); // TODO:

        // let code = loop {
        //     match self.server.check_for_browser_response()? {
        //         None => {
        //             // p.inc(1);
        //             std::thread::sleep(Duration::from_millis(10));
        //         }
        //         Some(response) => break response,
        //     }
        // };

        // println!("OauthLoginFlow::get_credentials code: {code}"); // TODO:

        // // 4. Exchange code for credentials
        // let auth = send_async(AuthenticateWithCode::new(&code, &self.pkce))
        //     .await
        //     .map_err(|err| Error::Generic(err.into()))?;

        // // 5. Store credentials
        // let credentials = Credentials::from_auth_response(auth.into())?.ensure_stored()?;

        // // p.finish_and_clear();

        // println!(
        //     "Success! You are now logged in as {}",
        //     credentials.user().email
        // );
        // println!("Rerun will automatically use the credentials stored on your machine."); // TODO:

        // Ok(credentials)
    }
}
