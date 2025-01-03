use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf};
use tokio::net::TcpStream;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::sync::{Mutex, Notify};

use super::protocol::Message;

#[derive(Debug)]
pub struct ControlViewer {
    address: String,
    tx: UnboundedSender<Message>,
    rx: Arc<Mutex<UnboundedReceiver<Message>>>,
    message_queue: Arc<Mutex<VecDeque<Message>>>,
    notify: Arc<Notify>,
}

#[derive(Clone)]
pub struct ControlViewerHandle {
    tx: UnboundedSender<Message>,
}

impl ControlViewerHandle {
    pub fn send(&self, msg: Message) -> Result<(), tokio::sync::mpsc::error::SendError<Message>> {
        self.tx.send(msg)
    }
}

impl ControlViewer {
    pub async fn connect(address: String) -> tokio::io::Result<Self> {
        let (tx, rx) = unbounded_channel();
        Ok(Self {
            address,
            tx,
            rx: Arc::new(Mutex::new(rx)),
            message_queue: Arc::new(Mutex::new(VecDeque::new())),
            notify: Arc::new(Notify::new()),
        })
    }

    pub fn handle(&self) -> ControlViewerHandle {
        ControlViewerHandle {
            tx: self.tx.clone(),
        }
    }

    pub async fn run(self) {
        re_log::info!("Starting client");

        // Spawn a background task to handle messages from the global channel.
        {
            let rx = Arc::clone(&self.rx);
            let message_queue = Arc::clone(&self.message_queue);
            let notify = Arc::clone(&self.notify);
            tokio::spawn(async move {
                Self::global_message_handler(rx, message_queue, notify).await;
            });
        }

        loop {
            match TcpStream::connect(self.address.clone()).await {
                Ok(socket) => {
                    if let Err(e) = socket.set_linger(Some(Duration::from_secs(2))) {
                        re_log::error!("Failed to set socket linger: {:?}", e);
                    }

                    re_log::info!("Connected to {}", self.address);
                    let (read_half, write_half) = tokio::io::split(socket);

                    // Spawn tasks to handle read and write
                    let reader_task = tokio::spawn(Self::handle_read(read_half));
                    let writer_task = {
                        let message_queue = Arc::clone(&self.message_queue);
                        let notify = Arc::clone(&self.notify);
                        tokio::spawn(async move {
                            Self::handle_write(write_half, message_queue, notify).await;
                        })
                    };

                    // Wait for tasks to complete
                    tokio::select! {
                        result = reader_task => {
                            if let Err(e) = result {
                                re_log::error!("Reader task ended with error: {:?}", e);
                            }
                        }
                        result = writer_task => {
                            if let Err(e) = result {
                                re_log::error!("Writer task ended with error: {:?}", e);
                            }
                        }
                    }

                    re_log::info!("Connection lost. Attempting to reconnect...");
                }
                Err(e) => {
                    re_log::error!("Failed to connect to {}: {:?}", self.address, e);
                }
            }

            // Wait some time before attempting to reconnect
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    }

    async fn global_message_handler(
        rx: Arc<Mutex<UnboundedReceiver<Message>>>,
        message_queue: Arc<Mutex<VecDeque<Message>>>,
        notify: Arc<Notify>,
    ) {
        let mut rx_guard = rx.lock().await;
        while let Some(message) = rx_guard.recv().await {
            // Store the message in the queue and notify the writer task
            let mut queue_guard = message_queue.lock().await;
            queue_guard.push_back(message);
            drop(queue_guard);
            notify.notify_one();
        }
        re_log::info!("Global message channel closed");
    }

    async fn handle_read(mut read: ReadHalf<TcpStream>) {
        let mut buf = [0; 1024];
        loop {
            match read.read(&mut buf).await {
                Ok(0) => {
                    re_log::info!("Server closed connection");
                    break;
                }
                Ok(n) => match Message::decode(&buf[..n]) {
                    Ok(message) => {
                        // we received a message from the server, we can process it here if needed
                        re_log::info!("Received message from server: {:?}", message);
                    }
                    Err(e) => {
                        re_log::error!("Failed to decode message: {:?}", e);
                    }
                },
                Err(e) => {
                    re_log::error!("Error reading from server: {:?}", e);
                    break;
                }
            }
        }
    }

    async fn handle_write(
        mut write: WriteHalf<TcpStream>,
        message_queue: Arc<Mutex<VecDeque<Message>>>,
        notify: Arc<Notify>,
    ) {
        loop {
            let message_option;
            {
                let mut queue_guard = message_queue.lock().await;
                message_option = queue_guard.pop_front();
            }

            match message_option {
                Some(message) => match message {
                    Message::Disconnect => {
                        re_log::info!("Disconnecting...");
                        break;
                    }
                    _ => {
                        if let Ok(data) = message.encode() {
                            if let Err(e) = write.write_all(&data).await {
                                re_log::info!(
                                    "Failed to send message: {:?}, error: {:?}",
                                    message,
                                    e
                                );
                                break;
                            }
                        }
                    }
                },
                None => {
                    // If no messages are available, wait for a new one to arrive
                    notify.notified().await;
                }
            }
        }
    }
}
