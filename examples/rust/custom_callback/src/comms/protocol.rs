use std::io::{self, ErrorKind};

use serde::{Deserialize, Serialize};

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
        bincode::serialize(self).map_err(|err| io::Error::new(ErrorKind::InvalidData, err))
    }

    pub fn encode_into(&self, buffer: &mut [u8]) -> io::Result<()> {
        bincode::serialize_into(buffer, self)
            .map_err(|err| io::Error::new(ErrorKind::InvalidData, err))
    }

    pub fn decode(data: &[u8]) -> io::Result<Self> {
        bincode::deserialize(data).map_err(|err| io::Error::new(ErrorKind::InvalidData, err))
    }
}
