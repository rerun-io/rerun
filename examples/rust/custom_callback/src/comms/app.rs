use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::State;
use axum::extract::ws::{self, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use axum::routing::get;
use parking_lot::RwLock;
use rerun::external::{re_error, re_log};
use tokio::net::TcpListener;

use super::protocol::Message;
use super::web;

type HandlerFn = Box<dyn Fn(&Message) + Send + Sync + 'static>;

/// An HTTP server that accepts control panels on `/ws` and also hosts the browser control panel.
///
/// Both the native viewer and the browser panel connect to `/ws`.
/// Every message from every connection runs all handlers registered with [`ControlAppHandle::add_handler`].
pub struct ControlApp {
    listener: TcpListener,
    addr: SocketAddr,
}

impl ControlApp {
    pub async fn bind(addr: &str) -> std::io::Result<Self> {
        let listener = TcpListener::bind(addr).await?;
        let addr = listener.local_addr()?;
        Ok(Self { listener, addr })
    }

    pub fn run(self) -> ControlAppHandle {
        let handle = ControlAppHandle::default();

        let router = web::routes()
            .route("/ws", get(ws_upgrade))
            .with_state(handle.clone());

        re_log::info!("Control server running on http://{}", self.addr);

        tokio::spawn(async move {
            if let Err(err) = axum::serve(self.listener, router).await {
                re_log::error!("Control server stopped: {}", re_error::format_ref(&err));
            }
        });

        handle
    }
}

#[derive(Clone, Default)]
pub struct ControlAppHandle {
    handlers: Arc<RwLock<Vec<HandlerFn>>>,
}

impl ControlAppHandle {
    pub fn add_handler(&self, handler: impl Fn(&Message) + Send + Sync + 'static) {
        self.handlers.write().push(Box::new(handler));
    }

    fn dispatch(&self, message: &Message) {
        re_log::info!("Received message: {message:?}");
        for handler in self.handlers.read().iter() {
            handler(message);
        }
    }
}

async fn ws_upgrade(ws: WebSocketUpgrade, State(app): State<ControlAppHandle>) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, app))
}

async fn handle_socket(mut socket: WebSocket, app: ControlAppHandle) {
    re_log::info!("Control panel connected");

    while let Some(frame) = socket.recv().await {
        match frame {
            Ok(ws::Message::Text(text)) => match serde_json::from_str::<Message>(&text) {
                Ok(message) => app.dispatch(&message),
                Err(err) => re_log::error!(
                    "Failed to decode message: {}\nPayload: {text}",
                    re_error::format_ref(&err)
                ),
            },
            Ok(ws::Message::Close(_)) => break,
            Ok(_) => {}
            Err(err) => {
                re_log::error!(
                    "Error reading from WebSocket: {}",
                    re_error::format_ref(&err)
                );
                break;
            }
        }
    }

    re_log::info!("Control panel disconnected");
}
