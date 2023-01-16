use std::net::SocketAddr;

use lazy_static::lazy_static;

use re_log_types::{
    ApplicationId, BeginRecordingMsg, LogMsg, LoggedData, MsgId, ObjPath, ObjPathComp, ObjTypePath,
    ObjectType, PathOp, RecordingId, RecordingInfo, Time, TimePoint, TypeMsg,
};
use re_string_interner::InternedString;

#[derive(Debug)]
enum StoreSelect {
    Arrow,
    Classic,
    Mixed,
}

// TODO(#707): Make this the default for Debug-builds, and eventually release builds
//#[cfg(debug_assertions)]
//const DEFAULT_STORE: StoreSelect = StoreSelect::Arrow;

//#[cfg(not(debug_assertions))]
const DEFAULT_STORE: StoreSelect = StoreSelect::Classic;

lazy_static! {
    static ref ARROW_PREFIX: InternedString = "arrow".into();
    static ref CLASSIC_PREFIX: InternedString = "classic".into();
}

pub struct Session {
    #[cfg(feature = "web")]
    tokio_rt: tokio::runtime::Runtime,

    sender: Sender,

    // TODO(emilk): just store `ObjTypePathHash`
    registered_types: nohash_hasher::IntMap<ObjTypePath, ObjectType>,

    application_id: Option<ApplicationId>,
    recording_id: Option<RecordingId>,

    has_sent_begin_recording_msg: bool,

    store_select: StoreSelect,
}

impl Session {
    pub fn new() -> Self {
        let store_select = match std::env::var("RERUN_STORE").as_deref() {
            Ok("arrow") => StoreSelect::Arrow,
            Ok("classic") => StoreSelect::Classic,
            Ok("mixed") => StoreSelect::Mixed,
            Ok("") | Err(_) => DEFAULT_STORE,
            Ok(_) => {
                re_log::error!(
                    "Unexpected value of RERUN_STORE. Please set to: arrow, classic, or mixed"
                );
                DEFAULT_STORE
            }
        };

        Self {
            #[cfg(feature = "web")]
            tokio_rt: tokio::runtime::Runtime::new().unwrap(),

            sender: Default::default(),
            registered_types: Default::default(),
            application_id: None,
            recording_id: None,
            has_sent_begin_recording_msg: false,
            store_select,
        }
    }

    pub fn set_application_id(&mut self, application_id: ApplicationId) {
        if self.application_id.as_ref() != Some(&application_id) {
            self.application_id = Some(application_id);
            self.has_sent_begin_recording_msg = false;
        }
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
        let (rerun_tx, rerun_rx) =
            re_smart_channel::smart_channel(re_smart_channel::Source::Network);

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
                let application_id = self
                    .application_id
                    .clone()
                    .unwrap_or_else(ApplicationId::unknown);

                re_log::debug!(
                    "Beginning new recording with application_id {:?} and recording id {}",
                    application_id.0,
                    recording_id
                );

                self.sender.send(
                    BeginRecordingMsg {
                        msg_id: MsgId::random(),
                        info: RecordingInfo {
                            application_id,
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

    // convenience
    pub fn send_path_op(&mut self, time_point: &TimePoint, path_op: PathOp) {
        self.send(LogMsg::PathOpMsg(re_log_types::PathOpMsg {
            msg_id: MsgId::random(),
            time_point: time_point.clone(),
            path_op,
        }));
    }

    pub fn arrow_log_gate(&self) -> bool {
        matches!(self.store_select, StoreSelect::Mixed | StoreSelect::Arrow)
    }

    pub fn classic_log_gate(&self) -> bool {
        matches!(self.store_select, StoreSelect::Mixed | StoreSelect::Classic)
    }

    pub fn arrow_prefix_obj_path(&self, obj_path: ObjPath) -> ObjPath {
        match self.store_select {
            StoreSelect::Arrow => obj_path,
            StoreSelect::Classic => {
                re_log::error_once!(
                    "Tried to log to arrow store when in mode {:?}. Path: {:?}",
                    self.store_select,
                    obj_path
                );
                let mut components = obj_path.to_components();
                components.insert(0, ObjPathComp::Name(*CLASSIC_PREFIX));
                components.into()
            }
            StoreSelect::Mixed => {
                let mut components = obj_path.to_components();
                components.insert(0, ObjPathComp::Name(*ARROW_PREFIX));
                components.into()
            }
        }
    }

    pub fn classic_prefix_obj_path(&self, obj_path: ObjPath) -> ObjPath {
        match self.store_select {
            StoreSelect::Arrow => {
                re_log::error_once!(
                    "Tried to log to classic store when in mode {:?}. Path: {:?}",
                    self.store_select,
                    obj_path
                );
                let mut components = obj_path.to_components();
                components.insert(0, ObjPathComp::Name(*ARROW_PREFIX));
                components.into()
            }
            StoreSelect::Classic => obj_path,
            StoreSelect::Mixed => {
                let mut components = obj_path.to_components();
                components.insert(0, ObjPathComp::Name(*CLASSIC_PREFIX));
                components.into()
            }
        }
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

enum Sender {
    Remote(re_sdk_comms::Client),

    #[allow(unused)] // only used with `#[cfg(feature = "re_viewer")]`
    Buffered(Vec<LogMsg>),

    /// Send it to the web viewer over WebSockets
    #[cfg(feature = "web")]
    WebViewer(
        tokio::task::JoinHandle<()>,
        re_smart_channel::Sender<LogMsg>,
    ),
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
