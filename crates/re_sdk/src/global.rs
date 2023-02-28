use crate::session::Session;

/// Access a global [`Session`] singleton for convenient logging.
///
/// By default, logging is enabled. To disable logging, call `set_enabled(false)` on the global `Session`, or
/// set the `RERUN` environment variable to `false`.
pub fn global_session() -> parking_lot::MutexGuard<'static, Session> {
    let default_enabled = true;
    global_session_with_default_enabled(default_enabled)
}

/// Access a global [`Session`] singleton for convenient logging.
///
/// The given variable controls if Rerun is enabled by default.
/// It can be overridden with the `RERUN` environment variable.
pub fn global_session_with_default_enabled(
    default_enabled: bool,
) -> parking_lot::MutexGuard<'static, Session> {
    use once_cell::sync::OnceCell;
    use parking_lot::Mutex;
    static INSTANCE: OnceCell<Mutex<Session>> = OnceCell::new();

    let mutex = INSTANCE.get_or_init(|| Mutex::new(Session::with_default_enabled(default_enabled)));
    mutex.lock()
}
