use re_log_types::{LogId, LogMsg, ObjTypePath, ObjectType, TypeMsg};

#[derive(Default)]
pub struct Sdk {
    // TODO(emilk): also support sending over `mpsc::Sender`.
    sender: Sender,

    // TODO(emilk): just store `ObjTypePathHash`
    registered_types: nohash_hasher::IntMap<ObjTypePath, ObjectType>,
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
    pub fn configure_remote(&mut self) {
        if !matches!(&self.sender, &Sender::Remote(_)) {
            self.sender = Sender::Remote(Default::default());
        }
    }

    #[cfg(feature = "re_viewer")]
    #[allow(unused)] // only used with "re_viewer" feature
    pub fn configure_buffered(&mut self) {
        if !matches!(&self.sender, &Sender::Buffered(_)) {
            self.sender = Sender::Buffered(Default::default());
        }
    }

    #[cfg(feature = "re_viewer")]
    pub fn is_buffered(&self) -> bool {
        matches!(&self.sender, &Sender::Buffered(_))
    }

    /// Wait until all logged data have been sent to the remove server (if any).
    pub fn flush(&mut self) {
        if let Sender::Remote(remote) = &mut self.sender {
            remote.flush();
        }
    }

    #[cfg(feature = "re_viewer")]
    pub fn drain_log_messages(&mut self) -> Vec<LogMsg> {
        match &mut self.sender {
            Sender::Remote(_) => vec![],
            Sender::Buffered(log_messages) => std::mem::take(log_messages),
        }
    }

    pub fn register_type(&mut self, obj_type_path: &ObjTypePath, typ: ObjectType) {
        if let Some(prev_type) = self.registered_types.get(obj_type_path) {
            if *prev_type != typ {
                tracing::warn!("Registering different types to the same object type path: {}. First you uses {:?}, then {:?}",
                               obj_type_path, prev_type, typ);
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
}

enum Sender {
    Remote(re_sdk_comms::Client),

    #[allow(unused)] // only used with `#[cfg(feature = "re_viewer")]`
    Buffered(Vec<LogMsg>),
}

impl Default for Sender {
    fn default() -> Self {
        Sender::Remote(Default::default())
    }
}

impl Sender {
    pub fn send(&mut self, msg: LogMsg) {
        match self {
            Self::Remote(client) => client.send(&msg),
            Self::Buffered(buffer) => buffer.push(msg),
        }
    }
}
