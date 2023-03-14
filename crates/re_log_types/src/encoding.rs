//! Encoding/decoding [`LogMsg`]:es as `.rrd` files.

use crate::LogMsg;

// ----------------------------------------------------------------------------
// native encode:

#[cfg(feature = "save")]
#[cfg(not(target_arch = "wasm32"))]
mod encoder {
    use std::io::Write as _;

    use crate::LogMsg;

    /// On failure to encode or serialize a [`LogMsg`].
    #[derive(thiserror::Error, Debug)]
    pub enum EncodeError {
        #[error("Failed to write: {0}")]
        Write(std::io::Error),

        #[error("Zstd error: {0}")]
        Zstd(std::io::Error),

        #[error("MsgPack error: {0}")]
        MsgPack(#[from] rmp_serde::encode::Error),
    }

    /// Encode a stream of [`LogMsg`] into an `.rrd` file.
    pub struct Encoder<W: std::io::Write> {
        zstd_encoder: zstd::stream::Encoder<'static, W>,
        buffer: Vec<u8>,
    }

    impl<W: std::io::Write> Encoder<W> {
        pub fn new(mut write: W) -> Result<Self, EncodeError> {
            let rerun_version = re_build_info::CrateVersion::parse(env!("CARGO_PKG_VERSION"));

            write.write_all(b"RRF0").map_err(EncodeError::Write)?;
            write
                .write_all(&rerun_version.to_bytes())
                .map_err(EncodeError::Write)?;

            let level = 3;
            let zstd_encoder =
                zstd::stream::Encoder::new(write, level).map_err(EncodeError::Zstd)?;

            Ok(Self {
                zstd_encoder,
                buffer: vec![],
            })
        }

        pub fn append(&mut self, message: &LogMsg) -> Result<(), EncodeError> {
            let Self {
                zstd_encoder,
                buffer,
            } = self;

            buffer.clear();
            rmp_serde::encode::write_named(buffer, message)?;

            zstd_encoder
                .write_all(&(buffer.len() as u64).to_le_bytes())
                .map_err(EncodeError::Zstd)?;
            zstd_encoder.write_all(buffer).map_err(EncodeError::Zstd)?;

            Ok(())
        }

        pub fn finish(self) -> Result<(), EncodeError> {
            self.zstd_encoder.finish().map_err(EncodeError::Zstd)?;
            Ok(())
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
}

#[cfg(feature = "save")]
#[cfg(not(target_arch = "wasm32"))]
pub use encoder::*;

// ----------------------------------------------------------------------------

fn warn_on_version_mismatch(encoded_version: [u8; 4]) {
    use re_build_info::CrateVersion;

    // We used 0000 for all .rrd files up until 2023-02-27, post 0.2.0 release:
    let encoded_version = if encoded_version == [0, 0, 0, 0] {
        CrateVersion::new(0, 2, 0)
    } else {
        CrateVersion::from_bytes(encoded_version)
    };

    let local_version = CrateVersion::parse(env!("CARGO_PKG_VERSION"));

    if !encoded_version.is_compatible_with(local_version) {
        re_log::warn!("Found log stream with Rerun version {encoded_version}, which is incompatible with the local Rerun version {local_version}. Loading will try to continue, but might fail in subtle ways.");
    }
}

// ----------------------------------------------------------------------------

/// On failure to encode or serialize a [`LogMsg`].
#[cfg(feature = "load")]
#[derive(thiserror::Error, Debug)]
pub enum DecodeError {
    #[error("Not an .rrd file")]
    NotAnRrd,

    #[error("Failed to read: {0}")]
    Read(std::io::Error),

    #[cfg(not(target_arch = "wasm32"))]
    #[error("Zstd error: {0}")]
    Zstd(std::io::Error),

    #[cfg(target_arch = "wasm32")]
    #[error("Zstd error: {0}")]
    RuzstdInit(ruzstd::frame_decoder::FrameDecoderError),

    #[cfg(target_arch = "wasm32")]
    #[error("Zstd read error: {0}")]
    RuzstdRead(std::io::Error),

    #[error("MsgPack error: {0}")]
    MsgPack(#[from] rmp_serde::decode::Error),
}

// ----------------------------------------------------------------------------
// native decode:

#[cfg(feature = "load")]
#[cfg(not(target_arch = "wasm32"))]
pub struct Decoder<'r, R: std::io::BufRead> {
    zdecoder: zstd::stream::Decoder<'r, R>,
    buffer: Vec<u8>,
}

#[cfg(feature = "load")]
#[cfg(not(target_arch = "wasm32"))]
impl<'r, R: std::io::Read> Decoder<'r, std::io::BufReader<R>> {
    pub fn new(mut read: R) -> Result<Self, DecodeError> {
        crate::profile_function!();

        let mut header = [0_u8; 4];
        read.read_exact(&mut header).map_err(DecodeError::Read)?;
        if &header != b"RRF0" {
            return Err(DecodeError::NotAnRrd);
        }
        read.read_exact(&mut header).map_err(DecodeError::Read)?;
        warn_on_version_mismatch(header);

        let zdecoder = zstd::stream::read::Decoder::new(read).map_err(DecodeError::Zstd)?;
        Ok(Self {
            zdecoder,
            buffer: vec![],
        })
    }
}

#[cfg(feature = "load")]
#[cfg(not(target_arch = "wasm32"))]
impl<'r, R: std::io::BufRead> Iterator for Decoder<'r, R> {
    type Item = Result<LogMsg, DecodeError>;

    fn next(&mut self) -> Option<Self::Item> {
        crate::profile_function!();
        use std::io::Read as _;

        let mut len = [0_u8; 8];
        self.zdecoder.read_exact(&mut len).ok()?;
        let len = u64::from_le_bytes(len) as usize;

        self.buffer.resize(len, 0);

        {
            crate::profile_scope!("zstd");
            if let Err(err) = self.zdecoder.read_exact(&mut self.buffer) {
                return Some(Err(DecodeError::Zstd(err)));
            }
        }

        crate::profile_scope!("MsgPack deser");
        match rmp_serde::from_read(&mut self.buffer.as_slice()) {
            Ok(msg) => Some(Ok(msg)),
            Err(err) => Some(Err(err.into())),
        }
    }
}

// ----------------------------------------------------------------------------
// wasm decode:

#[cfg(feature = "load")]
#[cfg(target_arch = "wasm32")]
pub struct Decoder<R: std::io::Read> {
    zdecoder: ruzstd::StreamingDecoder<R>,
    buffer: Vec<u8>,
}

#[cfg(feature = "load")]
#[cfg(target_arch = "wasm32")]
impl<R: std::io::Read> Decoder<R> {
    pub fn new(mut read: R) -> Result<Self, DecodeError> {
        crate::profile_function!();

        let mut header = [0_u8; 4];
        read.read_exact(&mut header).map_err(DecodeError::Read)?;
        if &header != b"RRF0" {
            return Err(DecodeError::NotAnRrd);
        }
        read.read_exact(&mut header).map_err(DecodeError::Read)?;
        warn_on_version_mismatch(header);

        let zdecoder = ruzstd::StreamingDecoder::new(read).map_err(DecodeError::RuzstdInit)?;
        Ok(Self {
            zdecoder,
            buffer: vec![],
        })
    }
}

#[cfg(feature = "load")]
#[cfg(target_arch = "wasm32")]
impl<R: std::io::Read> Iterator for Decoder<R> {
    type Item = Result<LogMsg, DecodeError>;

    fn next(&mut self) -> Option<Self::Item> {
        crate::profile_function!();
        use std::io::Read as _;

        let mut len = [0_u8; 8];
        self.zdecoder.read_exact(&mut len).ok()?;
        let len = u64::from_le_bytes(len) as usize;

        self.buffer.resize(len, 0);

        {
            crate::profile_scope!("ruzstd");
            if let Err(err) = self.zdecoder.read_exact(&mut self.buffer) {
                return Some(Err(DecodeError::RuzstdRead(err)));
            }
        }

        crate::profile_scope!("MsgPack deser");
        match rmp_serde::from_read(&mut self.buffer.as_slice()) {
            Ok(msg) => Some(Ok(msg)),
            Err(err) => Some(Err(err.into())),
        }
    }
}

// ----------------------------------------------------------------------------

#[cfg(all(feature = "load", feature = "save"))]
#[test]
fn test_encode_decode() {
    use crate::{BeginRecordingMsg, LogMsg, MsgId, Time};

    let messages = vec![LogMsg::BeginRecordingMsg(BeginRecordingMsg {
        msg_id: MsgId::random(),
        info: crate::RecordingInfo {
            application_id: crate::ApplicationId("test".to_owned()),
            recording_id: crate::RecordingId::random(),
            is_official_example: true,
            started: Time::now(),
            recording_source: crate::RecordingSource::RustSdk {
                rustc_version: String::new(),
                llvm_version: String::new(),
            },
        },
    })];

    let mut file = vec![];
    encode(messages.iter(), &mut file).unwrap();

    let decoded_messages = Decoder::new(&mut file.as_slice())
        .unwrap()
        .collect::<Result<Vec<LogMsg>, DecodeError>>()
        .unwrap();

    assert_eq!(messages, decoded_messages);
}
