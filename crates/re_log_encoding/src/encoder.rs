//! Encoding of [`LogMsg`]es as a binary stream, e.g. to store in an `.rrd` file, or send over network.

use std::io::Write as _;

use re_log_types::LogMsg;

use crate::{Compression, EncodingOptions};

// ----------------------------------------------------------------------------

/// On failure to encode or serialize a [`LogMsg`].
#[derive(thiserror::Error, Debug)]
pub enum EncodeError {
    #[error("Failed to write: {0}")]
    Write(std::io::Error),

    #[error("lz4 error: {0}")]
    Lz4Write(std::io::Error),

    #[error("lz4 error: {0}")]
    Lz4Finish(lz4_flex::frame::Error),

    #[error("MsgPack error: {0}")]
    MsgPack(#[from] rmp_serde::encode::Error),

    #[error("Called append on already finished encoder")]
    AlreadyFinished,
}

// ----------------------------------------------------------------------------

pub fn encode_to_bytes<'a>(
    options: EncodingOptions,
    msgs: impl IntoIterator<Item = &'a LogMsg>,
) -> Result<Vec<u8>, EncodeError> {
    let mut bytes: Vec<u8> = vec![];
    {
        let mut encoder = Encoder::new(options, std::io::Cursor::new(&mut bytes))?;
        for msg in msgs {
            encoder.append(msg)?;
        }
        encoder.finish()?;
    }
    Ok(bytes)
}

// ----------------------------------------------------------------------------

struct Lz4Compressor<W: std::io::Write> {
    /// `None` if finished.
    lz4_encoder: Option<lz4_flex::frame::FrameEncoder<W>>,
}

impl<W: std::io::Write> Lz4Compressor<W> {
    pub fn new(write: W) -> Self {
        Self {
            lz4_encoder: Some(lz4_flex::frame::FrameEncoder::new(write)),
        }
    }

    pub fn write(&mut self, bytes: &[u8]) -> Result<(), EncodeError> {
        if let Some(lz4_encoder) = &mut self.lz4_encoder {
            lz4_encoder
                .write_all(bytes)
                .map_err(EncodeError::Lz4Write)?;

            Ok(())
        } else {
            Err(EncodeError::AlreadyFinished)
        }
    }

    pub fn finish(&mut self) -> Result<(), EncodeError> {
        if let Some(lz4_encoder) = self.lz4_encoder.take() {
            lz4_encoder.finish().map_err(EncodeError::Lz4Finish)?;
            Ok(())
        } else {
            re_log::warn!("Encoder::finish called twice");
            Ok(())
        }
    }
}

impl<W: std::io::Write> Drop for Lz4Compressor<W> {
    fn drop(&mut self) {
        if self.lz4_encoder.is_some() {
            re_log::warn!("Encoder dropped without calling finish()!");
            if let Err(err) = self.finish() {
                re_log::error!("Failed to finish encoding: {err}");
            }
        }
    }
}

#[allow(clippy::large_enum_variant)]
enum Compressor<W: std::io::Write> {
    Off(W),
    Lz4(Lz4Compressor<W>),
}

impl<W: std::io::Write> Compressor<W> {
    pub fn new(compression: Compression, write: W) -> Self {
        match compression {
            Compression::Off => Self::Off(write),
            Compression::LZ4 => Self::Lz4(Lz4Compressor::new(write)),
        }
    }

    pub fn write(&mut self, bytes: &[u8]) -> Result<(), EncodeError> {
        let len = (bytes.len() as u64).to_le_bytes();

        match self {
            Compressor::Off(write) => {
                write.write_all(&len).map_err(EncodeError::Write)?;
                write.write_all(bytes).map_err(EncodeError::Write)
            }
            Compressor::Lz4(lz4) => {
                lz4.write(&len)?;
                lz4.write(bytes)
            }
        }
    }

    pub fn finish(&mut self) -> Result<(), EncodeError> {
        match self {
            Compressor::Off(_) => Ok(()),
            Compressor::Lz4(lz4) => lz4.finish(),
        }
    }
}

// ----------------------------------------------------------------------------

/// Encode a stream of [`LogMsg`] into an `.rrd` file.
pub struct Encoder<W: std::io::Write> {
    compressor: Compressor<W>,
    buffer: Vec<u8>,
}

impl<W: std::io::Write> Encoder<W> {
    pub fn new(options: EncodingOptions, mut write: W) -> Result<Self, EncodeError> {
        let rerun_version = re_build_info::CrateVersion::parse(env!("CARGO_PKG_VERSION"));

        write
            .write_all(crate::RRD_HEADER)
            .map_err(EncodeError::Write)?;
        write
            .write_all(&rerun_version.to_bytes())
            .map_err(EncodeError::Write)?;
        write
            .write_all(&options.to_bytes())
            .map_err(EncodeError::Write)?;

        match options.serializer {
            crate::Serializer::MsgPack => {}
        }

        Ok(Self {
            compressor: Compressor::new(options.compression, write),
            buffer: vec![],
        })
    }

    pub fn append(&mut self, message: &LogMsg) -> Result<(), EncodeError> {
        let Self { compressor, buffer } = self;

        buffer.clear();
        rmp_serde::encode::write_named(buffer, message)?;

        compressor.write(buffer)
    }

    pub fn finish(&mut self) -> Result<(), EncodeError> {
        self.compressor.finish()
    }
}

pub fn encode<'a>(
    options: EncodingOptions,
    messages: impl Iterator<Item = &'a LogMsg>,
    write: &mut impl std::io::Write,
) -> Result<(), EncodeError> {
    let mut encoder = Encoder::new(options, write)?;
    for message in messages {
        encoder.append(message)?;
    }
    encoder.finish()
}

pub fn encode_owned(
    options: EncodingOptions,
    messages: impl Iterator<Item = LogMsg>,
    write: impl std::io::Write,
) -> Result<(), EncodeError> {
    let mut encoder = Encoder::new(options, write)?;
    for message in messages {
        encoder.append(&message)?;
    }
    encoder.finish()
}
