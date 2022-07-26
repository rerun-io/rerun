use re_log_types::{LogId, LogMsg, ObjTypePath, ObjectType, TypeMsg};

#[derive(Default)]
pub struct Sdk {
    // TODO(emilk): also support sending over `mpsc::Sender`.
    sender: re_sdk_comms::Client,

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

    pub fn register_type(&mut self, obj_type_path: &ObjTypePath, typ: ObjectType) {
        if let Some(prev_type) = self.registered_types.get(obj_type_path) {
            if *prev_type != typ {
                tracing::warn!("Registering different types to the same object type path: {}. First you uses {:?}, then {:?}",
                               obj_type_path, prev_type, typ);
            }
        } else {
            self.registered_types.insert(obj_type_path.clone(), typ);

            self.send(&LogMsg::TypeMsg(TypeMsg {
                id: LogId::random(),
                type_path: obj_type_path.clone(),
                object_type: typ,
            }));
        }
    }
}

impl Sdk {
    pub fn send(&mut self, log_msg: &LogMsg) {
        self.sender.send(log_msg);
    }
}
