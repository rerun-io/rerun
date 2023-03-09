use crate::session::Session;

/// Access a global [`Session`] singleton for convenient logging.
///
/// The default [`Session`] is a disabled dummy-session that ignore all log calls,
/// so you need to explicitly set the global session for it to be useful
///
/// Example usage:
///
/// ```
/// use re_sdk::{global_session, SessionBuilder, default_server_addr};
///
/// *global_session() = SessionBuilder::new("my_app").connect(default_server_addr());
///
/// // Call code that logs using `global_session`.
/// ```
pub fn global_session() -> parking_lot::MutexGuard<'static, Session> {
    use once_cell::sync::OnceCell;
    use parking_lot::Mutex;
    static INSTANCE: OnceCell<Mutex<Session>> = OnceCell::new();
    let mutex = INSTANCE.get_or_init(|| Mutex::new(Session::disabled()));
    mutex.lock()
}
