use std::time::Duration;

use crate::callback_server::{Error, OauthCallbackServer};
use crate::oauth;
use crate::oauth::Credentials;
use crate::oauth::api::{AuthenticateWithCode, Pkce, send_async};

use super::OAUTH_CLIENT_ID;
use super::api::{
    AuthenticateWithDeviceCode, AuthenticateWithDeviceCodeResponse, DeviceCodeFlowStatus,
    GetDeviceAuthUrl, RefreshResponse,
};

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
        let server = OauthCallbackServer::new(&pkce)?;

        Ok(OauthLoginFlowState::LoginFlowStarted(Self {
            server,
            login_hint,
            pkce,
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

#[expect(clippy::large_enum_variant)]
pub enum DeviceCodeFlowState {
    AlreadyLoggedIn(Credentials),
    LoginFlowStarted(DeviceCodeFlow),
}

pub struct DeviceCodeFlow {
    device_code: String,
    user_code: String,
    verification_uri: String,
    interval: Duration,
}

impl DeviceCodeFlow {
    pub async fn init(force_login: bool) -> Result<DeviceCodeFlowState, Error> {
        if !force_login {
            // NOTE: If the loading fails for whatever reason, we debug log the error
            // and have the user login again as if nothing happened.
            match oauth::load_credentials() {
                Ok(Some(credentials)) => {
                    match oauth::refresh_credentials(credentials).await {
                        Ok(credentials) => {
                            return Ok(DeviceCodeFlowState::AlreadyLoggedIn(credentials));
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

        let res = send_async(GetDeviceAuthUrl {
            client_id: &OAUTH_CLIENT_ID,
        })
        .await
        .map_err(|err| Error::Generic(err.into()))?;

        let interval = Duration::from_secs(res.interval_seconds as u64);

        Ok(DeviceCodeFlowState::LoginFlowStarted(Self {
            device_code: res.device_code,
            user_code: res.user_code,
            verification_uri: res.verification_uri_complete,
            interval,
        }))
    }

    pub fn get_login_url(&self) -> &str {
        &self.verification_uri
    }

    pub fn get_user_code(&self) -> &str {
        &self.user_code
    }

    pub async fn wait_for_user_confirmation(&mut self) -> Result<Credentials, Error> {
        loop {
            let res = send_async(AuthenticateWithDeviceCode::new(
                &OAUTH_CLIENT_ID,
                &self.device_code,
            ))
            .await
            .map_err(|err| Error::Generic(err.into()))?;

            match res {
                AuthenticateWithDeviceCodeResponse::Success {
                    user,
                    organization_id,
                    access_token,
                    refresh_token,
                } => {
                    return Ok(Credentials::from_auth_response(RefreshResponse {
                        user,
                        organization_id,
                        access_token,
                        refresh_token,
                    })?
                    .ensure_stored()?);
                }
                AuthenticateWithDeviceCodeResponse::Error {
                    error,
                    error_description,
                } => match error {
                    DeviceCodeFlowStatus::AuthorizationPending => { /*fallthrough*/ }
                    DeviceCodeFlowStatus::SlowDown => {
                        self.interval += Duration::from_secs(1);
                        /*fallthrough*/
                    }
                    DeviceCodeFlowStatus::AccessDenied
                    | DeviceCodeFlowStatus::ExpiredToken
                    | DeviceCodeFlowStatus::InvalidRequest
                    | DeviceCodeFlowStatus::InvalidClient
                    | DeviceCodeFlowStatus::InvalidGrant
                    | DeviceCodeFlowStatus::UnsupportedGrantType => {
                        return Err(Error::Generic(
                            DeviceCodeFlowError {
                                code: error,
                                reason: error_description,
                            }
                            .into(),
                        ));
                    }
                },
            }

            tokio::time::sleep(self.interval).await;
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("{code:?}: {reason}")]
pub struct DeviceCodeFlowError {
    code: DeviceCodeFlowStatus,
    reason: String,
}
