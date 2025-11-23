use crate::callback_server::Error;
use crate::callback_server::OauthCallbackServer;
use crate::oauth;
use crate::oauth::Credentials;
use crate::oauth::api::AuthenticateWithCode;
use crate::oauth::api::Pkce;
use crate::oauth::api::send_async;

pub enum OauthLoginFlowState {
    AlreadyLoggedIn(Credentials),
    LoginFlowStarted(OauthLoginFlow),
}

pub struct OauthLoginFlow {
    pub server: OauthCallbackServer,
    pub login_hint: Option<String>,
    pkce: Pkce,
}

impl OauthLoginFlow {
    /// Login to Rerun using Authorization Code flow.
    ///
    /// This first checks if valid credentials already exist locally,
    /// and doesn't perform the login flow if so, unless `force_login` is set to `true`.
    pub async fn init(force_login: bool) -> Result<OauthLoginFlowState, Error> {
        let mut login_hint = None;
        if !force_login {
            // NOTE: If the loading fails for whatever reason, we debug log the error
            // and have the user login again as if nothing happened.
            match oauth::load_credentials() {
                Ok(Some(credentials)) => {
                    login_hint = Some(credentials.user().email.clone());
                    match oauth::refresh_credentials(credentials).await {
                        Ok(credentials) => {
                            return Ok(OauthLoginFlowState::AlreadyLoggedIn(credentials));
                        }
                        Err(err) => {
                            // Credentials are bad, login again.
                            re_log::debug!("refreshing credentials failed: {err}");
                        }
                    }
                }

                Ok(None) => {
                    // No credentials yet, login as usual.
                }

                Err(err) => {
                    re_log::debug!(
                        "validating credentials failed, logging user in again anyway. reason: {err}"
                    );
                }
            }
        }

        // Start web server that listens for the authorization code received from the auth server.
        let pkce = Pkce::new();
        let server = OauthCallbackServer::new(&pkce, login_hint.as_deref())?;

        Ok(OauthLoginFlowState::LoginFlowStarted(Self {
            server,
            pkce,
            login_hint,
        }))
    }

    pub fn get_login_url(&self) -> &str {
        self.server.get_login_url()
    }

    /// Polls the web server for the authorization code received from the auth server.
    ///
    /// This will not block, and will return `None` if no authorization code has been received yet.
    pub async fn poll(&self) -> Result<Option<Credentials>, Error> {
        // Once the user opens the link, they are redirected to the login UI.
        // If they were already logged in, it will immediately redirect them
        // to the login callback with an authorization code.
        // That code is then sent by our callback page back to the web server here.
        let Some(code) = self.server.check_for_browser_response()? else {
            return Ok(None);
        };

        // Exchange code for credentials.
        let auth = send_async(AuthenticateWithCode::new(&code, &self.pkce))
            .await
            .map_err(|err| Error::Generic(err.into()))?;

        // Store and return credentials
        let credentials = Credentials::from_auth_response(auth.into())?.ensure_stored()?;
        Ok(Some(credentials))
    }
}
