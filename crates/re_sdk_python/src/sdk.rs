use std::net::SocketAddr;

use re_log_types::{
    LogId, LogMsg, ObjTypePath, ObjectType, Time, TimePoint, TimeSource, TimeType, TimeValue,
    TypeMsg,
};

#[derive(Default)]
pub struct Sdk {
    // TODO(emilk): also support sending over `mpsc::Sender`.
    sender: Sender,

    // TODO(emilk): just store `ObjTypePathHash`
    registered_types: nohash_hasher::IntMap<ObjTypePath, ObjectType>,

    /// The current time, which can be set by users.
    time_point: TimePoint,
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

    /// Send log data to a remote server.
    pub fn connect(&mut self, addr: SocketAddr) {
        match &mut self.sender {
            Sender::Remote(remote) => {
                remote.set_addr(addr);
            }
            Sender::Buffered(messages) => {
                tracing::debug!("Connecting to remote…");
                let mut client = re_sdk_comms::Client::new(addr);
                for msg in messages.drain(..) {
                    client.send(msg);
                }
                self.sender = Sender::Remote(client);
            }
        }
    }

    #[cfg(feature = "re_viewer")]
    #[allow(unused)] // only used with "re_viewer" feature
    pub fn disconnect(&mut self) {
        if !matches!(&self.sender, &Sender::Buffered(_)) {
            tracing::debug!("Switching to buffered.");
            self.sender = Sender::Buffered(Default::default());
        }
    }

    #[cfg(feature = "re_viewer")]
    pub fn is_connected(&self) -> bool {
        matches!(&self.sender, &Sender::Remote(_))
    }

    /// Wait until all logged data have been sent to the remove server (if any).
    pub fn flush(&mut self) {
        if let Sender::Remote(sender) = &mut self.sender {
            sender.flush();
        }
    }

    #[cfg(feature = "re_viewer")]
    pub fn drain_log_messages_buffer(&mut self) -> Vec<LogMsg> {
        match &mut self.sender {
            Sender::Remote(_) => vec![],
            Sender::Buffered(log_messages) => std::mem::take(log_messages),
        }
    }

    pub fn register_type(&mut self, obj_type_path: &ObjTypePath, typ: ObjectType) {
        if let Some(prev_type) = self.registered_types.get(obj_type_path) {
            if *prev_type != typ {
                tracing::warn!("Registering different types to the same object type path: {obj_type_path:?}. First you used {prev_type:?}, then {typ:?}");
            }
        } else {
            self.registered_types.insert(obj_type_path.clone(), typ);

            self.send(LogMsg::TypeMsg(TypeMsg {
                id: LogId::random(),
                type_path: obj_type_path.clone(),
                object_type: typ,
            }));
        }
    }

    pub fn send(&mut self, log_msg: LogMsg) {
        self.sender.send(log_msg);
    }

    pub fn now(&self) -> TimePoint {
        let mut time_point = self.time_point.clone();
        time_point.0.insert(
            TimeSource::new("log_time", TimeType::Time),
            Time::now().into(),
        );
        time_point
    }

    pub fn set_time(&mut self, time_source: TimeSource, time_value: Option<TimeValue>) {
        if let Some(time_value) = time_value {
            self.time_point.0.insert(time_source, time_value);
        } else {
            self.time_point.0.remove(&time_source);
        }
    }
}

enum Sender {
    Remote(re_sdk_comms::Client),

    #[allow(unused)] // only used with `#[cfg(feature = "re_viewer")]`
    Buffered(Vec<LogMsg>),
}

impl Default for Sender {
    fn default() -> Self {
        Sender::Buffered(vec![])
    }
}

impl Sender {
    pub fn send(&mut self, msg: LogMsg) {
        match self {
            Self::Remote(client) => client.send(msg),
            Self::Buffered(buffer) => buffer.push(msg),
        }
    }
}
