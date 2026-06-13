//! WebSocket client for remote (non-LCM) mode.
//!
//! When `dimos-viewer` is started with `--connect`, LCM multicast is unavailable
//! across machines. This module connects to a WebSocket server (typically the
//! Python `RerunWebSocketServer` module) and sends click, twist, and stop events
//! as JSON.
//!
//! Message format (JSON objects with a `"type"` discriminant):
//!
//! ```json
//! {"type":"click","x":1.0,"y":2.0,"z":3.0,"entity_path":"/world","timestamp_ms":1234567890}
//! {"type":"twist","linear_x":0.5,"linear_y":0.0,"linear_z":0.0,"angular_x":0.0,"angular_y":0.0,"angular_z":0.8}
//! {"type":"stop"}
//! ```

use std::{sync::Arc, time::Duration};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

/// Error returned when a WebSocket event cannot be sent.
#[derive(Debug)]
pub enum SendError {
    /// The send queue is full; the event was dropped.
    QueueFull,
    /// Failed to serialize the event to JSON.
    Serialize(String),
}

impl std::fmt::Display for SendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::QueueFull => write!(f, "send queue full, event dropped"),
            Self::Serialize(e) => write!(f, "serialization error: {e}"),
        }
    }
}

/// JSON message variants sent over the WebSocket.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsEvent {
    Click {
        x: f64,
        y: f64,
        z: f64,
        entity_path: String,
        timestamp_ms: u64,
    },
    Twist {
        linear_x: f64,
        linear_y: f64,
        linear_z: f64,
        angular_x: f64,
        angular_y: f64,
        angular_z: f64,
    },
    Stop,
}

/// JSON message variants received from the WebSocket server.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum WsCommand {
    /// Request that DimOS opens or updates a Web Page View panel.
    OpenWebPageView {
        /// Caller-owned stable identifier used to update the same panel later.
        panel_id: String,
        /// Human-readable panel title.
        title: String,
        /// Configured page URL.
        url: String,
        /// Whether browser-like controls should be visible.
        show_navigation_controls: bool,
    },
}

/// Error returned when an inbound WebSocket command is not safe to apply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WsCommandValidationError {
    /// URL text could not be parsed as an absolute URL.
    InvalidUrl,
    /// URL uses a scheme that Web Page View does not allow.
    UnsupportedUrlScheme(String),
}

impl std::fmt::Display for WsCommandValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidUrl => write!(f, "invalid URL"),
            Self::UnsupportedUrlScheme(scheme) => write!(f, "unsupported URL scheme: {scheme}"),
        }
    }
}

impl WsCommand {
    /// Validate command fields that affect viewer state.
    pub fn validate(&self) -> Result<(), WsCommandValidationError> {
        match self {
            Self::OpenWebPageView { url, .. } => validate_web_page_url(url),
        }
    }
}

fn validate_web_page_url(url: &str) -> Result<(), WsCommandValidationError> {
    let parsed = match url::Url::parse(url) {
        Ok(parsed) => parsed,
        Err(err) => {
            let _err = err;
            return Err(WsCommandValidationError::InvalidUrl);
        }
    };
    match parsed.scheme() {
        "http" | "https" => Ok(()),
        scheme => Err(WsCommandValidationError::UnsupportedUrlScheme(
            scheme.to_owned(),
        )),
    }
}

fn parse_command(text: &str) -> Result<WsCommand, serde_json::Error> {
    serde_json::from_str(text)
}

/// Sends `WsEvent`s (serialized to JSON) to a remote WebSocket server.
///
/// Maintains a persistent connection with automatic reconnection. The
/// internal sender is `Clone`, so you can hand copies to multiple producers
/// (keyboard handler, click handler, …).
#[derive(Clone)]
pub struct WsPublisher {
    tx: mpsc::Sender<String>,
    command_rx: Arc<Mutex<mpsc::Receiver<WsCommand>>>,
}

impl WsPublisher {
    /// Spawn the WebSocket client task and return a publisher.
    ///
    /// The client connects to `url` (e.g. `ws://127.0.0.1:3030/ws`) and
    /// reconnects automatically whenever the connection drops.
    ///
    /// This spawns a dedicated background thread with its own tokio runtime,
    /// so it works even when called from a non-async context (like the eframe UI).
    pub fn connect(url: String) -> Self {
        let (tx, rx) = mpsc::channel::<String>(256);
        let (command_tx, command_rx) = mpsc::channel::<WsCommand>(256);

        // Spawn a dedicated thread with its own tokio runtime.
        // This allows WsPublisher to work from the eframe UI thread which
        // doesn't have a tokio runtime.
        std::thread::Builder::new()
            .name("ws-publisher".to_owned())
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("failed to create tokio runtime for WsPublisher");
                rt.block_on(run_client(url, rx, command_tx));
            })
            .expect("failed to spawn WsPublisher thread");

        Self {
            tx,
            command_rx: Arc::new(Mutex::new(command_rx)),
        }
    }

    /// Try to receive one inbound command without blocking the UI thread.
    pub fn try_recv_command(&self) -> Option<WsCommand> {
        self.command_rx.lock().try_recv().ok()
    }

    /// Publish a click event.
    pub fn send_click(
        &self,
        x: f64,
        y: f64,
        z: f64,
        entity_path: &str,
        timestamp_ms: u64,
    ) -> Result<(), SendError> {
        let event = WsEvent::Click {
            x,
            y,
            z,
            entity_path: entity_path.to_owned(),
            timestamp_ms,
        };
        self.broadcast(&event)
    }

    /// Publish a twist (velocity) command.
    pub fn send_twist(
        &self,
        linear_x: f64,
        linear_y: f64,
        linear_z: f64,
        angular_x: f64,
        angular_y: f64,
        angular_z: f64,
    ) -> Result<(), SendError> {
        let event = WsEvent::Twist {
            linear_x,
            linear_y,
            linear_z,
            angular_x,
            angular_y,
            angular_z,
        };
        self.broadcast(&event)
    }

    /// Publish a stop command.
    pub fn send_stop(&self) -> Result<(), SendError> {
        self.broadcast(&WsEvent::Stop)
    }

    fn broadcast(&self, event: &WsEvent) -> Result<(), SendError> {
        let json =
            serde_json::to_string(&event).map_err(|e| SendError::Serialize(e.to_string()))?;
        // Non-blocking: error if the channel is full rather than block the UI thread.
        self.tx.try_send(json).map_err(|err| {
            let _err = err;
            SendError::QueueFull
        })
    }
}

/// Returns true if `DIMOS_DEBUG` is set to `1`.
fn is_debug() -> bool {
    std::env::var("DIMOS_DEBUG").is_ok_and(|v| v == "1")
}

/// Background task: connect → send → reconnect loop.
async fn run_client(
    url: String,
    mut rx: mpsc::Receiver<String>,
    command_tx: mpsc::Sender<WsCommand>,
) {
    use futures_util::{SinkExt as _, StreamExt as _};
    use tokio_tungstenite::{connect_async, tungstenite::Message};

    let debug = is_debug();

    loop {
        if debug {
            eprintln!("[DIMOS_DEBUG] WsPublisher: connecting to {url}");
        }

        match connect_async(&url).await {
            Ok((ws_stream, _)) => {
                if debug {
                    eprintln!("[DIMOS_DEBUG] WsPublisher: connected to {url}");
                }

                let (mut writer, mut reader) = ws_stream.split();

                // Read task: consume incoming frames (ping → auto pong) so the
                // server's keepalive pings get answered and the connection stays
                // alive. Exits when the server closes or an error occurs.
                let debug_read = debug;
                let command_tx_read = command_tx.clone();
                let mut read_handle = tokio::spawn(async move {
                    while let Some(frame) = reader.next().await {
                        match frame {
                            Ok(Message::Text(text)) => match parse_command(&text) {
                                Ok(command) => {
                                    if let Err(err) = command_tx_read.try_send(command)
                                        && debug_read
                                    {
                                        eprintln!(
                                            "[DIMOS_DEBUG] WsPublisher: inbound command dropped: {err}"
                                        );
                                    }
                                }
                                Err(err) => {
                                    if debug_read {
                                        eprintln!(
                                            "[DIMOS_DEBUG] WsPublisher: ignoring inbound message: {err}"
                                        );
                                    }
                                }
                            },
                            Ok(Message::Close(_)) => {
                                if debug_read {
                                    eprintln!("[DIMOS_DEBUG] WsPublisher: server sent close frame");
                                }
                                break;
                            }
                            Err(err) => {
                                if debug_read {
                                    eprintln!("[DIMOS_DEBUG] WsPublisher: read error: {err}");
                                }
                                break;
                            }
                            _ => {} // Ping/Pong handled by tungstenite internally
                        }
                    }
                });

                // Write loop: drain the channel into the WebSocket.
                let disconnected = loop {
                    tokio::select! {
                        msg = rx.recv() => {
                            match msg {
                                Some(text) => {
                                    if let Err(err) = writer.send(Message::text(text)).await {
                                        if debug {
                                            eprintln!("[DIMOS_DEBUG] WsPublisher: send error: {err} — reconnecting");
                                        }
                                        break false;
                                    }
                                }
                                None => break true, // rx closed → task is done
                            }
                        }
                        _ = &mut read_handle => {
                            // Reader exited → server closed the connection.
                            if debug {
                                eprintln!("[DIMOS_DEBUG] WsPublisher: server closed connection — reconnecting");
                            }
                            break false;
                        }
                    }
                };

                if disconnected {
                    if debug {
                        eprintln!("[DIMOS_DEBUG] WsPublisher: channel closed, shutting down");
                    }
                    break;
                }
            }
            Err(err) => {
                if debug {
                    eprintln!(
                        "[DIMOS_DEBUG] WsPublisher: connection failed: {err} — retrying in 1s"
                    );
                }
            }
        }

        // Drain any stale commands queued during the disconnect — sending
        // outdated velocity commands on reconnect would be dangerous.
        while rx.try_recv().is_ok() {}

        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_publisher_with_command_rx(command_rx: mpsc::Receiver<WsCommand>) -> WsPublisher {
        let (tx, _rx) = mpsc::channel::<String>(1);
        WsPublisher {
            tx,
            command_rx: Arc::new(Mutex::new(command_rx)),
        }
    }

    #[test]
    fn parses_open_web_page_view_command() {
        let command: WsCommand = serde_json::from_str(
            r#"{
                "type": "open_web_page_view",
                "panel_id": "viser",
                "title": "Viser",
                "url": "http://127.0.0.1:8095/",
                "show_navigation_controls": true
            }"#,
        )
        .expect("valid open_web_page_view command should parse");

        assert_eq!(
            command,
            WsCommand::OpenWebPageView {
                panel_id: "viser".to_owned(),
                title: "Viser".to_owned(),
                url: "http://127.0.0.1:8095/".to_owned(),
                show_navigation_controls: true,
            }
        );
    }

    #[test]
    fn outbound_events_keep_existing_wire_format() {
        let click = serde_json::to_value(WsEvent::Click {
            x: 1.0,
            y: 2.0,
            z: 3.0,
            entity_path: "/world".to_owned(),
            timestamp_ms: 1234567890,
        })
        .expect("click event should serialize");
        assert_eq!(
            click,
            serde_json::json!({
                "type": "click",
                "x": 1.0,
                "y": 2.0,
                "z": 3.0,
                "entity_path": "/world",
                "timestamp_ms": 1234567890_u64,
            })
        );

        let twist = serde_json::to_value(WsEvent::Twist {
            linear_x: 0.5,
            linear_y: 0.0,
            linear_z: 0.0,
            angular_x: 0.0,
            angular_y: 0.0,
            angular_z: 0.8,
        })
        .expect("twist event should serialize");
        assert_eq!(
            twist,
            serde_json::json!({
                "type": "twist",
                "linear_x": 0.5,
                "linear_y": 0.0,
                "linear_z": 0.0,
                "angular_x": 0.0,
                "angular_y": 0.0,
                "angular_z": 0.8,
            })
        );

        let stop = serde_json::to_value(WsEvent::Stop).expect("stop event should serialize");
        assert_eq!(stop, serde_json::json!({ "type": "stop" }));
    }

    #[test]
    fn try_recv_command_drains_inbound_queue_without_blocking() {
        let (command_tx, command_rx) = mpsc::channel::<WsCommand>(1);
        command_tx
            .try_send(WsCommand::OpenWebPageView {
                panel_id: "viser".to_owned(),
                title: "Viser".to_owned(),
                url: "http://127.0.0.1:8095/".to_owned(),
                show_navigation_controls: true,
            })
            .expect("test command queue should have capacity");

        let publisher = test_publisher_with_command_rx(command_rx);

        assert_eq!(
            publisher.try_recv_command(),
            Some(WsCommand::OpenWebPageView {
                panel_id: "viser".to_owned(),
                title: "Viser".to_owned(),
                url: "http://127.0.0.1:8095/".to_owned(),
                show_navigation_controls: true,
            })
        );
        assert_eq!(publisher.try_recv_command(), None);
    }

    #[test]
    fn unknown_or_malformed_inbound_messages_are_rejected_by_parser() {
        assert!(parse_command(r#"{"type":"unknown"}"#).is_err());
        assert!(parse_command(r#"{"type":"open_web_page_view","panel_id":"viser"}"#).is_err());
        assert!(parse_command("not json").is_err());
    }

    #[test]
    fn layout_placement_fields_are_rejected_by_parser() {
        assert!(
            parse_command(
                r#"{
                    "type": "open_web_page_view",
                    "panel_id": "viser",
                    "title": "Viser",
                    "url": "http://127.0.0.1:8095/",
                    "show_navigation_controls": true,
                    "split_direction": "right"
                }"#,
            )
            .is_err()
        );
    }

    #[test]
    fn web_page_command_url_policy_matches_web_page_view() {
        assert_eq!(validate_web_page_url("https://rerun.io"), Ok(()));
        assert_eq!(validate_web_page_url("http://127.0.0.1:8095/"), Ok(()));
        assert_eq!(
            validate_web_page_url("file:///tmp/report.html"),
            Err(WsCommandValidationError::UnsupportedUrlScheme(
                "file".to_owned()
            ))
        );
        assert_eq!(
            validate_web_page_url("javascript:alert(1)"),
            Err(WsCommandValidationError::UnsupportedUrlScheme(
                "javascript".to_owned()
            ))
        );
        assert_eq!(
            validate_web_page_url("not a url"),
            Err(WsCommandValidationError::InvalidUrl)
        );
    }
}
