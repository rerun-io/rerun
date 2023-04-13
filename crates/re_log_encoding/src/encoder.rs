//! Encoding of [`LogMsg`]es as a binary stream, e.g. to store in an `.rrd` file, or send over network.

use std::io::Write as _;

use re_log_types::LogMsg;

/// On failure to encode or serialize a [`LogMsg`].
#[derive(thiserror::Error, Debug)]
pub enum EncodeError {
    #[error("Failed to write: {0}")]
    Write(std::io::Error),

    #[error("Zstd error: {0}")]
    Zstd(std::io::Error),

    #[error("MsgPack error: {0}")]
    MsgPack(#[from] rmp_serde::encode::Error),

    #[error("Called append on already finished encoder")]
    AlreadyFinished,
}

/// Encode a stream of [`LogMsg`] into an `.rrd` file.
pub struct Encoder<W: std::io::Write> {
    /// Set to None when finished.
    zstd_encoder: Option<zstd::stream::Encoder<'static, W>>,
    buffer: Vec<u8>,
}

impl<W: std::io::Write> Drop for Encoder<W> {
    fn drop(&mut self) {
        if self.zstd_encoder.is_some() {
            re_log::warn!("Encoder dropped without calling finish()!");
            if let Err(err) = self.finish() {
                re_log::error!("Failed to finish encoding: {err}");
            }
        }
    }
}

impl<W: std::io::Write> Encoder<W> {
    pub fn new(mut write: W) -> Result<Self, EncodeError> {
        let rerun_version = re_build_info::CrateVersion::parse(env!("CARGO_PKG_VERSION"));

        write.write_all(b"RRF0").map_err(EncodeError::Write)?;
        write
            .write_all(&rerun_version.to_bytes())
            .map_err(EncodeError::Write)?;

        let level = 3;
        let zstd_encoder = zstd::stream::Encoder::new(write, level).map_err(EncodeError::Zstd)?;

        Ok(Self {
            zstd_encoder: Some(zstd_encoder),
            buffer: vec![],
        })
    }

    pub fn append(&mut self, message: &LogMsg) -> Result<(), EncodeError> {
        let Self {
            zstd_encoder,
            buffer,
        } = self;

        if let Some(zstd_encoder) = zstd_encoder {
            buffer.clear();
            rmp_serde::encode::write_named(buffer, message)?;

            zstd_encoder
                .write_all(&(buffer.len() as u64).to_le_bytes())
                .map_err(EncodeError::Zstd)?;
            zstd_encoder.write_all(buffer).map_err(EncodeError::Zstd)?;

            Ok(())
        } else {
            Err(EncodeError::AlreadyFinished)
        }
    }

    pub fn finish(&mut self) -> Result<(), EncodeError> {
        if let Some(zstd_encoder) = self.zstd_encoder.take() {
            zstd_encoder.finish().map_err(EncodeError::Zstd)?;
            Ok(())
        } else {
            re_log::warn!("Encoder::finish called twice");
            Ok(())
        }
    }
}

pub fn encode<'a>(
    messages: impl Iterator<Item = &'a LogMsg>,
    write: impl std::io::Write,
) -> Result<(), EncodeError> {
    let mut encoder = Encoder::new(write)?;
    for message in messages {
        encoder.append(message)?;
    }
    encoder.finish()
}

pub fn encode_owned(
    messages: impl Iterator<Item = LogMsg>,
    write: impl std::io::Write,
) -> Result<(), EncodeError> {
    let mut encoder = Encoder::new(write)?;
    for message in messages {
        encoder.append(&message)?;
    }
    encoder.finish()
}
