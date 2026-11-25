//! [`Login`]: the `catalog_login` tool, a login link for the user to open.

use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

use futures::future::BoxFuture;
use parking_lot::Mutex;
use serde::Serialize;

use re_async::AsyncRuntimeHandle;
use re_auth::OauthLoginFlow;
use re_auth::callback_server::Error;
use re_auth::oauth::login_flow::OauthLoginFlowState;

use crate::CatalogToolError;

/// How long a call waits for the user to finish in the browser before it returns.
///
/// Long enough for most logins to finish within one call. A client that gives up sooner loses
/// only that call: the login keeps going, and the next call picks it up.
const WAIT: Duration = Duration::from_mins(2);

/// How often the login is checked: often enough that the browser gets its answer, and the agent
/// its result, without a lag the user notices.
const POLL_INTERVAL: Duration = Duration::from_millis(250);

/// How long a login link stays usable.
///
/// A pending login holds one of the few callback ports that `rerun auth login` and the viewer
/// also need, so an abandoned one must not hold it forever.
const PENDING_LIFETIME: Duration = Duration::from_mins(10);

/// What a `catalog_login` call returns to the agent.
#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum LoginStatus {
    LoggedIn {
        next_step: &'static str,
    },
    WaitingForUser {
        login_url: String,
        next_step: &'static str,
    },
}

impl LoginStatus {
    fn logged_in() -> Self {
        Self::LoggedIn {
            next_step: "Call `catalog_connect` again.",
        }
    }

    fn waiting(login_url: String) -> Self {
        Self::WaitingForUser {
            login_url,
            next_step: "Show the user this link and ask them to open it. Then call \
                        `catalog_login` again to wait until they are done, and \
                        `catalog_connect` again.",
        }
    }

    fn into_json(self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_else(|err| {
            re_log::error_once!("Failed to encode the login status: {err}");
            serde_json::Value::Null
        })
    }
}

/// A login the user finishes in their browser.
trait Flow: Send + Sync + 'static {
    fn login_url(&self) -> &str;

    /// `Ok(true)` once the user has logged in and the credentials are stored.
    fn poll(&self) -> BoxFuture<'_, Result<bool, Error>>;
}

impl Flow for OauthLoginFlow {
    fn login_url(&self) -> &str {
        self.get_login_url()
    }

    fn poll(&self) -> BoxFuture<'_, Result<bool, Error>> {
        Box::pin(async move { Ok(Self::poll(self).await?.is_some()) })
    }
}

/// Where the login stands, shared by the calls and the task that polls the login.
#[derive(Default)]
struct Shared {
    /// Counts the logins started, so that each finishes only itself.
    num_started: u64,

    /// The login the user may be finishing, by number, and its link.
    pending: Option<(u64, String)>,

    /// The last login that finished, by number, and how.
    finished: Option<(u64, Result<(), Arc<Error>>)>,
}

/// How far one login got.
enum Progress {
    Pending,
    LoggedIn,
    Failed(Arc<Error>),

    /// Its link is no longer usable: the user needs a new one.
    Expired,
}

/// The `catalog_login` tool, which remembers the login it started between calls.
pub struct Login {
    runtime: AsyncRuntimeHandle,
    shared: Arc<Mutex<Shared>>,
}

impl Login {
    pub fn new(runtime: AsyncRuntimeHandle) -> Self {
        Self {
            runtime,
            shared: Arc::default(),
        }
    }

    /// Start a login and return its link, or wait for the one started earlier.
    ///
    /// Returns `logged_in` at once if the user is already logged in, also when they logged in
    /// some other way after an earlier call started a login.
    pub async fn run(&self) -> Result<serde_json::Value, CatalogToolError> {
        let pending = self.shared.lock().pending.clone();
        let status = match pending {
            Some((id, login_url)) => match re_auth::oauth::load_and_refresh_credentials().await {
                Ok(Some(_)) => {
                    self.end(id, Ok(()));
                    LoginStatus::logged_in()
                }
                Ok(None) => self.wait(id, login_url, WAIT).await?,
                Err(err) => {
                    re_log::debug!("Not logged in yet: {err}");
                    self.wait(id, login_url, WAIT).await?
                }
            },
            None => self.start().await?,
        };
        Ok(status.into_json())
    }

    async fn start(&self) -> Result<LoginStatus, CatalogToolError> {
        let state =
            OauthLoginFlow::init(false)
                .await
                .map_err(|source| CatalogToolError::Login {
                    source: Arc::new(source),
                })?;
        Ok(match state {
            OauthLoginFlowState::AlreadyLoggedIn(_credentials) => LoginStatus::logged_in(),
            OauthLoginFlowState::LoginFlowStarted(flow) => {
                self.start_flow(Arc::new(*flow), PENDING_LIFETIME)
            }
        })
    }

    /// Show `flow`'s link, and poll it in the background until it ends or `lifetime` passes.
    ///
    /// Keeps the link of a login a concurrent call started meanwhile, since that one may have
    /// been shown already.
    fn start_flow(&self, flow: Arc<dyn Flow>, lifetime: Duration) -> LoginStatus {
        let mut shared = self.shared.lock();
        if let Some((_, login_url)) = &shared.pending {
            return LoginStatus::waiting(login_url.clone());
        }
        shared.num_started += 1;
        let id = shared.num_started;
        let login_url = flow.login_url().to_owned();
        shared.pending = Some((id, login_url.clone()));
        drop(shared);

        self.runtime.spawn_future(poll_until_done(
            Arc::downgrade(&self.shared),
            id,
            flow,
            lifetime,
        ));
        LoginStatus::waiting(login_url)
    }

    /// Wait up to `wait` for the login `id` to end.
    async fn wait(
        &self,
        id: u64,
        login_url: String,
        wait: Duration,
    ) -> Result<LoginStatus, CatalogToolError> {
        let start = Instant::now();
        loop {
            match self.progress(id) {
                Progress::Pending => {}
                Progress::LoggedIn => return Ok(LoginStatus::logged_in()),
                Progress::Failed(source) => return Err(CatalogToolError::Login { source }),
                Progress::Expired => return self.start().await,
            }
            if wait <= start.elapsed() {
                return Ok(LoginStatus::waiting(login_url));
            }
            re_async::sleep(POLL_INTERVAL).await;
        }
    }

    fn progress(&self, id: u64) -> Progress {
        let shared = self.shared.lock();
        if let Some((finished, outcome)) = &shared.finished
            && *finished == id
        {
            return match outcome {
                Ok(()) => Progress::LoggedIn,
                Err(err) => Progress::Failed(Arc::clone(err)),
            };
        }
        if shared
            .pending
            .as_ref()
            .is_some_and(|(pending, _)| *pending == id)
        {
            Progress::Pending
        } else {
            Progress::Expired
        }
    }

    /// End the login `id`, unless a newer one replaced it.
    fn end(&self, id: u64, outcome: Result<(), Arc<Error>>) {
        end(&mut self.shared.lock(), id, outcome);
    }
}

fn end(shared: &mut Shared, id: u64, outcome: Result<(), Arc<Error>>) {
    if shared
        .pending
        .as_ref()
        .is_some_and(|(pending, _)| *pending == id)
    {
        shared.pending = None;
    }
    shared.finished = Some((id, outcome));
}

/// Poll `flow` until the user finishes, it fails, `lifetime` passes, or the login `id` is no
/// longer pending, e.g. because the user logged in some other way.
///
/// Polling answers the browser, which waits on the callback until then.
async fn poll_until_done(
    shared: Weak<Mutex<Shared>>,
    id: u64,
    flow: Arc<dyn Flow>,
    lifetime: Duration,
) {
    let start = Instant::now();
    loop {
        let outcome = flow.poll().await;
        let Some(shared) = shared.upgrade() else {
            return;
        };
        if !keeps_going(&mut shared.lock(), id, outcome, lifetime <= start.elapsed()) {
            return;
        }
        drop(shared);
        re_async::sleep(POLL_INTERVAL).await;
    }
}

/// Record the poll `outcome` of the login `id`, and whether to poll it again.
fn keeps_going(shared: &mut Shared, id: u64, outcome: Result<bool, Error>, expired: bool) -> bool {
    if shared
        .pending
        .as_ref()
        .is_none_or(|(pending, _)| *pending != id)
    {
        return false;
    }
    match outcome {
        Ok(true) => end(shared, id, Ok(())),
        Ok(false) if expired => shared.pending = None,
        Ok(false) => return true,
        Err(err) => end(shared, id, Err(Arc::new(err))),
    }
    false
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU8, Ordering};

    use super::*;

    /// A flow that the test finishes or fails by setting its state.
    #[derive(Default)]
    struct FakeFlow {
        /// 0: waiting for the user, 1: logged in, 2: failed.
        state: AtomicU8,
    }

    impl FakeFlow {
        const WAITING: u8 = 0;
        const LOGGED_IN: u8 = 1;
        const FAILED: u8 = 2;

        fn set(&self, state: u8) {
            self.state.store(state, Ordering::Relaxed);
        }
    }

    impl Flow for FakeFlow {
        fn login_url(&self) -> &'static str {
            "http://localhost:17340/login"
        }

        fn poll(&self) -> BoxFuture<'_, Result<bool, Error>> {
            let state = self.state.load(Ordering::Relaxed);
            Box::pin(async move {
                match state {
                    Self::WAITING => Ok(false),
                    Self::LOGGED_IN => Ok(true),
                    _ => Err(Error::Generic("the user cancelled".into())),
                }
            })
        }
    }

    fn login() -> Login {
        Login::new(
            AsyncRuntimeHandle::from_current_tokio_runtime_or_wasmbindgen().expect("runtime"),
        )
    }

    fn pending_id(login: &Login) -> Option<u64> {
        login.shared.lock().pending.as_ref().map(|(id, _)| *id)
    }

    #[tokio::test]
    async fn a_finished_login_ends_the_wait() {
        let login = login();
        let flow = Arc::new(FakeFlow::default());
        login.start_flow(flow.clone(), PENDING_LIFETIME);
        flow.set(FakeFlow::LOGGED_IN);
        let status = login
            .wait(1, String::new(), Duration::from_secs(5))
            .await
            .expect("logged in");
        assert!(matches!(status, LoginStatus::LoggedIn { .. }));
        assert_eq!(pending_id(&login), None);
    }

    #[tokio::test]
    async fn a_failed_login_is_an_error_and_ends_it() {
        let login = login();
        let flow = Arc::new(FakeFlow::default());
        login.start_flow(flow.clone(), PENDING_LIFETIME);
        flow.set(FakeFlow::FAILED);
        let result = login.wait(1, String::new(), Duration::from_secs(5)).await;
        assert!(matches!(result, Err(CatalogToolError::Login { .. })));
        assert_eq!(pending_id(&login), None);
    }

    #[tokio::test]
    async fn the_wait_returns_the_link_while_the_user_is_not_done() {
        let login = login();
        login.start_flow(Arc::new(FakeFlow::default()), PENDING_LIFETIME);
        let status = login
            .wait(1, "link".to_owned(), POLL_INTERVAL)
            .await
            .expect("waiting");
        assert!(
            matches!(status, LoginStatus::WaitingForUser { login_url, .. } if login_url == "link")
        );
        assert_eq!(pending_id(&login), Some(1));
    }

    #[tokio::test]
    async fn a_second_start_keeps_the_first_link() {
        let login = login();
        login.start_flow(Arc::new(FakeFlow::default()), PENDING_LIFETIME);
        login.start_flow(Arc::new(FakeFlow::default()), PENDING_LIFETIME);
        assert_eq!(pending_id(&login), Some(1));
    }

    #[tokio::test]
    async fn an_old_login_does_not_end_a_newer_one() {
        let login = login();
        login.start_flow(Arc::new(FakeFlow::default()), PENDING_LIFETIME);
        login.shared.lock().pending = None;
        login.start_flow(Arc::new(FakeFlow::default()), PENDING_LIFETIME);
        login.end(1, Err(Arc::new(Error::Generic("old".into()))));
        assert_eq!(pending_id(&login), Some(2));
    }

    #[tokio::test]
    async fn an_expired_login_frees_its_link() {
        let login = login();
        login.start_flow(Arc::new(FakeFlow::default()), Duration::ZERO);
        re_async::sleep(4 * POLL_INTERVAL).await;
        assert_eq!(pending_id(&login), None);
    }
}
