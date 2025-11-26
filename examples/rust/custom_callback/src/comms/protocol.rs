use serde::{Deserialize, Serialize};
use std::io::{self, ErrorKind};

/// Messages that can be sent between the client and server.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Message {
    Point3d {
        path: String,
        position: (f32, f32, f32),
        radius: f32,
    },
    Box3d {
        path: String,
        half_size: (f32, f32, f32),
        position: (f32, f32, f32),
    },
    DynamicPosition {
        radius: f32,
        offset: f32,
    },
    Disconnect,
}

impl Message {
    pub fn encode(&self) -> io::Result<Vec<u8>> {
        bincode::serde::encode_to_vec(self, config::standard())
            .map_err(|err| io::Error::new(ErrorKind::InvalidData, err))
    }

    pub fn encode_into(&self, buffer: &mut [u8]) -> io::Result<()> {
        bincode::serde::encode_into_slice(self, buffer, config::standard())
            .map(|_bytes_written| ()) // Discard the usize return value
            .map_err(|err| io::Error::new(ErrorKind::InvalidData, err))
    }

    pub fn decode(data: &[u8]) -> io::Result<Self> {
        bincode::serde::decode_from_slice(data, config::standard())
            .map(|(message, _bytes_read)| message)
            .map_err(|err| io::Error::new(ErrorKind::InvalidData, err))
    }
}
