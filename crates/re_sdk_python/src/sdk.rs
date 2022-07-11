use re_log_types::LogMsg;

#[derive(Default)]
pub struct Sdk {
    // TODO(emilk): also support sending over `mpsc::Sender`.
    sender: re_sdk_comms::Client,
}

impl Sdk {
    /// Access the global [`Sdk`]. This is a singleton.
    pub fn global() -> std::sync::MutexGuard<'static, Self> {
        use once_cell::sync::OnceCell;
        use std::sync::Mutex;
        static INSTANCE: OnceCell<Mutex<Sdk>> = OnceCell::new();
        let mutex = INSTANCE.get_or_init(Default::default);
        mutex.lock().unwrap()
    }
}

impl Sdk {
    pub fn send(&mut self, log_msg: &LogMsg) {
        self.sender.send(log_msg);
    }
}
