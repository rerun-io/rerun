use std::{error::Error, result::Result, sync::Arc};

use parking_lot::RwLock;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf},
    net::{TcpListener, TcpSocket, TcpStream},
    sync::{
        Mutex,
        mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel},
    },
};

use rerun::external::{re_error, re_log};

use super::protocol::Message;

type HandlerFn = Box<dyn Fn(&Message) + Send + Sync + 'static>;

pub struct ControlApp {
    listener: TcpListener,
    handlers: RwLock<Vec<HandlerFn>>,
    clients: Arc<Mutex<Vec<UnboundedSender<Message>>>>,
}

impl ControlApp {
    pub async fn bind(addr: &str) -> tokio::io::Result<ControlApp> {
        let socket = TcpSocket::new_v4()?;
        socket.set_reuseaddr(true)?;
        socket.bind(addr.parse().unwrap())?;

        let listener = socket.listen(1024)?;
        Ok(Self {
            listener,
            handlers: RwLock::new(Vec::new()),
            clients: Arc::new(Mutex::new(Vec::new())),
        })
    }

    pub fn add_handler(&self, handler: HandlerFn) -> Result<(), Box<dyn Error>> {
        let mut handlers = self.handlers.write();
        handlers.push(handler);
        Ok(())
    }

    pub async fn broadcast(&self, message: Message) -> tokio::io::Result<()> {
        let clients = self.clients.lock().await;
        clients.iter().for_each(|client| {
            client.send(message.clone()).ok();
        });

        Ok(())
    }

    pub fn run(self) -> ControlAppHandle {
        re_log::info!(
            "Server running on {:?}",
            self.listener.local_addr().unwrap()
        );

        let app = Arc::new(self);
        let handle = app.clone();

        tokio::spawn(async move {
            loop {
                re_log::info!("Waiting for connection...");
                let app = app.clone();
                match app.listener.accept().await {
                    Ok((socket, addr)) => {
                        re_log::info!("Accepted connection from {:?}", addr);

                        tokio::spawn(async move {
                            app.handle_connection(socket).await;
                        });
                    }
                    Err(err) => {
                        re_log::error!(
                            "Error accepting connection: {}",
                            re_error::format_ref(&err)
                        );
                    }
                }
            }
        });

        ControlAppHandle { app: handle }
    }

    async fn handle_connection(&self, socket: TcpStream) {
        let (read_half, write_half) = tokio::io::split(socket);
        let (tx, rx) = unbounded_channel();

        // Add the client to the list
        {
            self.clients.lock().await.push(tx.clone());
        }

        // Spawn reader and writer tasks
        let reader_task = self.handle_reader(read_half);
        let writer_task = self.handle_writer(write_half, rx);

        let _ = tokio::join!(reader_task, writer_task);

        // Remove the client when the connection ends
        {
            let mut clients = self.clients.lock().await;
            if let Some(pos) = clients.iter().position(|x| x.same_channel(&tx)) {
                clients.remove(pos);
            }
        }
    }

    async fn handle_reader(&self, mut read_half: ReadHalf<TcpStream>) {
        let mut buf = [0; 1024];
        loop {
            match read_half.read(&mut buf).await {
                Ok(0) => {
                    re_log::info!("Connection closed by client");
                    break;
                }
                Ok(n) => match Message::decode(&buf[..n]) {
                    Ok(message) => {
                        re_log::info!("Received message: {:?}", message);
                        let handlers = &self.handlers.read();
                        for handler in handlers.iter() {
                            handler(&message);
                        }
                    }
                    Err(err) => {
                        re_log::error!("Failed to decode message: {}", re_error::format_ref(&err));
                    }
                },
                Err(err) => {
                    re_log::error!(
                        "Error reading from socket: {:?}",
                        re_error::format_ref(&err),
                    );
                    break;
                }
            }
        }
    }

    async fn handle_writer(
        &self,
        mut write_half: WriteHalf<TcpStream>,
        mut rx: UnboundedReceiver<Message>,
    ) {
        while let Some(message) = rx.recv().await {
            if matches!(message, Message::Disconnect) {
                re_log::info!("Received disconnect message, closing connection");
                break;
            }

            // Encode and send response
            if let Ok(data) = message.encode() {
                if write_half.write_all(&data).await.is_err() {
                    re_log::info!("Failed to send response to client");
                    break;
                }
            }
        }
    }
}

#[derive(Clone)]
pub struct ControlAppHandle {
    app: Arc<ControlApp>,
}

impl ControlAppHandle {
    pub fn add_handler<H>(
        &mut self,
        handler: H,
    ) -> std::result::Result<(), Box<dyn std::error::Error>>
    where
        H: Fn(&Message) + Send + Sync + 'static,
    {
        self.app.add_handler(Box::new(handler))
    }

    pub async fn broadcast(&self, message: Message) {
        let clients = self.app.clients.lock().await;

        clients.iter().for_each(|client| {
            client.send(message.clone()).ok();
        });
    }
}
