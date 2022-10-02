use std::net::SocketAddr;

use re_log_types::{
    BeginRecordingMsg, LogMsg, LoggedData, MsgId, ObjPath, ObjTypePath, ObjectType, RecordingId,
    RecordingInfo, Time, TimePoint, TypeMsg,
};

pub struct Sdk {
    #[cfg(feature = "web")]
    tokio_rt: tokio::runtime::Runtime,

    sender: Sender,

    // TODO(emilk): just store `ObjTypePathHash`
    registered_types: nohash_hasher::IntMap<ObjTypePath, ObjectType>,

    recording_id: Option<RecordingId>,

    has_sent_begin_recording_msg: bool,
}

impl Sdk {
    fn new() -> Self {
        Self {
            #[cfg(feature = "web")]
            tokio_rt: tokio::runtime::Runtime::new().unwrap(),

            sender: Default::default(),
            registered_types: Default::default(),
            recording_id: None,
            has_sent_begin_recording_msg: false,
        }
    }
}

impl Sdk {
    /// Access the global [`Sdk`]. This is a singleton.
    pub fn global() -> std::sync::MutexGuard<'static, Self> {
        use once_cell::sync::OnceCell;
        use std::sync::Mutex;
        static INSTANCE: OnceCell<Mutex<Sdk>> = OnceCell::new();
        let mutex = INSTANCE.get_or_init(|| Mutex::new(Sdk::new()));
        mutex.lock().unwrap()
    }

    pub fn recording_id(&self) -> Option<RecordingId> {
        self.recording_id
    }

    pub fn set_recording_id(&mut self, recording_id: RecordingId) {
        if self.recording_id != Some(recording_id) {
            self.recording_id = Some(recording_id);
            self.has_sent_begin_recording_msg = false;
        }
    }

    /// Send log data to a remote server.
    pub fn connect(&mut self, addr: SocketAddr) {
        match &mut self.sender {
            Sender::Remote(remote) => {
                remote.set_addr(addr);
            }
            Sender::Buffered(messages) => {
                re_log::debug!("Connecting to remote…");
                let mut client = re_sdk_comms::Client::new(addr);
                for msg in messages.drain(..) {
                    client.send(msg);
                }
                self.sender = Sender::Remote(client);
            }
            #[cfg(feature = "web")]
            Sender::WebViewer(web_server, _) => {
                re_log::info!("Shutting down web server.");
                web_server.abort();
                self.sender = Sender::Remote(re_sdk_comms::Client::new(addr));
            }
        }
    }

    #[cfg(feature = "web")]
    pub fn serve(&mut self) {
        let (rerun_tx, rerun_rx) = std::sync::mpsc::channel();

        let web_server_join_handle = self.tokio_rt.spawn(async {
            // This is the server which the web viewer will talk to:
            let ws_server = re_ws_comms::Server::new(re_ws_comms::DEFAULT_WS_SERVER_PORT)
                .await
                .unwrap();
            let ws_server_handle = tokio::spawn(ws_server.listen(rerun_rx));

            // This is the server that serves the WASM+HTML:
            let web_port = 9090;
            let web_server = re_web_server::WebServer::new(web_port);
            let web_server_handle = tokio::spawn(async move {
                web_server.serve().await.unwrap();
            });

            let ws_server_url = re_ws_comms::default_server_url();
            let viewer_url = format!("http://127.0.0.1:{}?url={}", web_port, ws_server_url);
            let open = true;
            if open {
                webbrowser::open(&viewer_url).ok();
            } else {
                re_log::info!("Web server is running - view it at {}", viewer_url);
            }

            ws_server_handle.await.unwrap().unwrap();
            web_server_handle.await.unwrap();
        });

        self.sender = Sender::WebViewer(web_server_join_handle, rerun_tx);
    }

    #[cfg(feature = "re_viewer")]
    #[allow(unused)] // only used with "re_viewer" feature
    pub fn disconnect(&mut self) {
        if !matches!(&self.sender, &Sender::Buffered(_)) {
            re_log::debug!("Switching to buffered.");
            self.sender = Sender::Buffered(Default::default());
        }
    }

    pub fn is_connected(&self) -> bool {
        matches!(&self.sender, &Sender::Remote(_))
    }

    /// Wait until all logged data have been sent to the remove server (if any).
    pub fn flush(&mut self) {
        if let Sender::Remote(sender) = &mut self.sender {
            sender.flush();
        }
    }

    pub fn drain_log_messages_buffer(&mut self) -> Vec<LogMsg> {
        match &mut self.sender {
            Sender::Remote(_) => vec![],
            Sender::Buffered(log_messages) => std::mem::take(log_messages),
            #[cfg(feature = "web")]
            Sender::WebViewer(_, _) => vec![],
        }
    }

    pub fn register_type(&mut self, obj_type_path: &ObjTypePath, typ: ObjectType) {
        if let Some(prev_type) = self.registered_types.get(obj_type_path) {
            if *prev_type != typ {
                re_log::warn!("Registering different types to the same object type path: {obj_type_path:?}. First you used {prev_type:?}, then {typ:?}");
            }
        } else {
            self.registered_types.insert(obj_type_path.clone(), typ);

            self.send(LogMsg::TypeMsg(TypeMsg {
                msg_id: MsgId::random(),
                type_path: obj_type_path.clone(),
                obj_type: typ,
            }));
        }
    }

    pub fn send(&mut self, log_msg: LogMsg) {
        if !self.has_sent_begin_recording_msg {
            if let Some(recording_id) = self.recording_id {
                re_log::debug!("Beginning new recording with recording id {recording_id}");
                self.sender.send(
                    BeginRecordingMsg {
                        msg_id: MsgId::random(),
                        info: RecordingInfo {
                            recording_id,
                            started: Time::now(),
                            recording_source: re_log_types::RecordingSource::PythonSdk,
                        },
                    }
                    .into(),
                );
                self.has_sent_begin_recording_msg = true;
            }
        }

        self.sender.send(log_msg);
    }

    // convenience
    pub fn send_data(
        &mut self,
        time_point: &TimePoint,
        (obj_path, field_name): (&ObjPath, &str),
        data: LoggedData,
    ) {
        self.send(LogMsg::DataMsg(re_log_types::DataMsg {
            msg_id: MsgId::random(),
            time_point: time_point.clone(),
            data_path: re_log_types::DataPath::new(obj_path.clone(), field_name.into()),
            data,
        }));
    }
}

enum Sender {
    Remote(re_sdk_comms::Client),

    #[allow(unused)] // only used with `#[cfg(feature = "re_viewer")]`
    Buffered(Vec<LogMsg>),

    /// Send it to the web viewer over WebSockets
    #[cfg(feature = "web")]
    WebViewer(tokio::task::JoinHandle<()>, std::sync::mpsc::Sender<LogMsg>),
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

            #[cfg(feature = "web")]
            Self::WebViewer(_, sender) => {
                if let Err(err) = sender.send(msg) {
                    re_log::error!("Failed to send log message to web server: {err}");
                }
            }
        }
    }
}
