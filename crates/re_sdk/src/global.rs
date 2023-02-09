use crate::session::Session;

/// Access a global [`Session`] singleton for convenient logging.
pub fn global_session() -> std::sync::MutexGuard<'static, Session> {
    use once_cell::sync::OnceCell;
    use std::sync::Mutex;
    static INSTANCE: OnceCell<Mutex<Session>> = OnceCell::new();
    let mutex = INSTANCE.get_or_init(|| Mutex::new(Session::new()));
    mutex.lock().unwrap()
}
