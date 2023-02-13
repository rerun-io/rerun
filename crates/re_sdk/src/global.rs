use crate::session::Session;

/// Access a global [`Session`] singleton for convenient logging.
///
/// By default, logging is enabled. To disable logging, call `set_enabled(false)` on the global `Session`, or
/// set the `RERUN` environment variable to `false`.
pub fn global_session() -> std::sync::MutexGuard<'static, Session> {
    use once_cell::sync::OnceCell;
    use std::sync::Mutex;
    static INSTANCE: OnceCell<Mutex<Session>> = OnceCell::new();

    let default_enabled = true;
    let mutex = INSTANCE.get_or_init(|| Mutex::new(Session::new(default_enabled)));
    mutex.lock().unwrap()
}
