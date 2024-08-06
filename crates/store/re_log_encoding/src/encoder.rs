//! Encoding of [`LogMsg`]es as a binary stream, e.g. to store in an `.rrd` file, or send over network.

use re_build_info::CrateVersion;
use re_chunk::{ChunkError, ChunkResult};
use re_log_types::LogMsg;

use crate::FileHeader;
use crate::MessageHeader;
use crate::{Compression, EncodingOptions};

// ----------------------------------------------------------------------------

/// On failure to encode or serialize a [`LogMsg`].
#[derive(thiserror::Error, Debug)]
pub enum EncodeError {
    #[error("Failed to write: {0}")]
    Write(std::io::Error),

    #[error("lz4 error: {0}")]
    Lz4(lz4_flex::block::CompressError),

    #[error("MsgPack error: {0}")]
    MsgPack(#[from] rmp_serde::encode::Error),

    #[error("Chunk error: {0}")]
    Chunk(#[from] ChunkError),

    #[error("Called append on already finished encoder")]
    AlreadyFinished,
}

// ----------------------------------------------------------------------------

pub fn encode_to_bytes<'a>(
    version: CrateVersion,
    options: EncodingOptions,
    msgs: impl IntoIterator<Item = &'a LogMsg>,
) -> Result<Vec<u8>, EncodeError> {
    let mut bytes: Vec<u8> = vec![];
    {
        let mut encoder = Encoder::new(version, options, std::io::Cursor::new(&mut bytes))?;
        for msg in msgs {
            encoder.append(msg)?;
        }
    }
    Ok(bytes)
}

// ----------------------------------------------------------------------------

/// Encode a stream of [`LogMsg`] into an `.rrd` file.
pub struct Encoder<W: std::io::Write> {
    compression: Compression,
    write: W,
    uncompressed: Vec<u8>,
    compressed: Vec<u8>,
}

impl<W: std::io::Write> Encoder<W> {
    pub fn new(
        version: CrateVersion,
        options: EncodingOptions,
        mut write: W,
    ) -> Result<Self, EncodeError> {
        FileHeader {
            magic: *crate::RRD_HEADER,
            version: version.to_bytes(),
            options,
        }
        .encode(&mut write)?;

        match options.serializer {
            crate::Serializer::MsgPack => {}
        }

        Ok(Self {
            compression: options.compression,
            write,
            uncompressed: vec![],
            compressed: vec![],
        })
    }

    pub fn append(&mut self, message: &LogMsg) -> Result<(), EncodeError> {
        re_tracing::profile_function!();

        self.uncompressed.clear();
        rmp_serde::encode::write_named(&mut self.uncompressed, message)?;

        match self.compression {
            Compression::Off => {
                MessageHeader {
                    uncompressed_len: self.uncompressed.len() as u32,
                    compressed_len: self.uncompressed.len() as u32,
                }
                .encode(&mut self.write)?;
                self.write
                    .write_all(&self.uncompressed)
                    .map_err(EncodeError::Write)?;
            }
            Compression::LZ4 => {
                let max_len = lz4_flex::block::get_maximum_output_size(self.uncompressed.len());
                self.compressed.resize(max_len, 0);
                let compressed_len =
                    lz4_flex::block::compress_into(&self.uncompressed, &mut self.compressed)
                        .map_err(EncodeError::Lz4)?;
                MessageHeader {
                    uncompressed_len: self.uncompressed.len() as u32,
                    compressed_len: compressed_len as u32,
                }
                .encode(&mut self.write)?;
                self.write
                    .write_all(&self.compressed[..compressed_len])
                    .map_err(EncodeError::Write)?;
            }
        }

        Ok(())
    }

    pub fn flush_blocking(&mut self) -> std::io::Result<()> {
        self.write.flush()
    }

    pub fn into_inner(self) -> W {
        self.write
    }
}

pub fn encode(
    version: CrateVersion,
    options: EncodingOptions,
    messages: impl Iterator<Item = ChunkResult<LogMsg>>,
    write: &mut impl std::io::Write,
) -> Result<(), EncodeError> {
    re_tracing::profile_function!();
    let mut encoder = Encoder::new(version, options, write)?;
    for message in messages {
        encoder.append(&message?)?;
    }
    Ok(())
}

pub fn encode_ref<'a>(
    version: CrateVersion,
    options: EncodingOptions,
    messages: impl Iterator<Item = ChunkResult<&'a LogMsg>>,
    write: &mut impl std::io::Write,
) -> Result<(), EncodeError> {
    re_tracing::profile_function!();
    let mut encoder = Encoder::new(version, options, write)?;
    for message in messages {
        encoder.append(message?)?;
    }
    Ok(())
}

pub fn encode_as_bytes(
    version: CrateVersion,
    options: EncodingOptions,
    messages: impl Iterator<Item = ChunkResult<LogMsg>>,
) -> Result<Vec<u8>, EncodeError> {
    re_tracing::profile_function!();
    let mut bytes: Vec<u8> = vec![];
    let mut encoder = Encoder::new(version, options, &mut bytes)?;
    for message in messages {
        encoder.append(&message?)?;
    }
    Ok(bytes)
}

#[inline]
pub fn local_encoder() -> Result<Encoder<Vec<u8>>, EncodeError> {
    Encoder::new(CrateVersion::LOCAL, EncodingOptions::COMPRESSED, Vec::new())
}

#[inline]
pub fn encode_as_bytes_local(
    messages: impl Iterator<Item = ChunkResult<LogMsg>>,
) -> Result<Vec<u8>, EncodeError> {
    let mut encoder = local_encoder()?;
    for message in messages {
        encoder.append(&message?)?;
    }
    Ok(encoder.into_inner())
}

#[inline]
pub fn encode_ref_as_bytes_local<'a>(
    messages: impl Iterator<Item = ChunkResult<&'a LogMsg>>,
) -> Result<Vec<u8>, EncodeError> {
    let mut encoder = local_encoder()?;
    for message in messages {
        encoder.append(message?)?;
    }
    Ok(encoder.into_inner())
}
