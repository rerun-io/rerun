//! Decoding [`LogMsg`]:es from `.rrd` files/streams.

use re_log_types::LogMsg;

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
#[derive(thiserror::Error, Debug)]
pub enum DecodeError {
    #[error("Not an .rrd file")]
    NotAnRrd,

    #[error("Failed to read: {0}")]
    Read(std::io::Error),

    #[error("lz4 error: {0}")]
    Lz4(std::io::Error),

    #[error("MsgPack error: {0}")]
    MsgPack(#[from] rmp_serde::decode::Error),
}

// ----------------------------------------------------------------------------

pub fn decode_bytes(bytes: &[u8]) -> Result<Vec<LogMsg>, DecodeError> {
    let decoder = Decoder::new(std::io::Cursor::new(bytes))?;
    let mut msgs = vec![];
    for msg in decoder {
        msgs.push(msg?);
    }
    Ok(msgs)
}

// ----------------------------------------------------------------------------

pub struct Decoder<R: std::io::Read> {
    lz4_decoder: lz4_flex::frame::FrameDecoder<R>,
    buffer: Vec<u8>,
}

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

        let lz4_decoder = lz4_flex::frame::FrameDecoder::new(read);
        Ok(Self {
            lz4_decoder,
            buffer: vec![],
        })
    }
}

impl<R: std::io::Read> Iterator for Decoder<R> {
    type Item = Result<LogMsg, DecodeError>;

    fn next(&mut self) -> Option<Self::Item> {
        crate::profile_function!();
        use std::io::Read as _;

        let mut len = [0_u8; 8];
        self.lz4_decoder.read_exact(&mut len).ok()?;
        let len = u64::from_le_bytes(len) as usize;

        self.buffer.resize(len, 0);

        {
            crate::profile_scope!("lz4");
            if let Err(err) = self.lz4_decoder.read_exact(&mut self.buffer) {
                return Some(Err(DecodeError::Lz4(err)));
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

#[cfg(all(feature = "decoder", feature = "encoder"))]
#[test]
fn test_encode_decode() {
    use re_log_types::{
        ApplicationId, LogMsg, RecordingId, RecordingInfo, RecordingSource, RecordingType, RowId,
        SetRecordingInfo, Time,
    };

    let messages = vec![LogMsg::SetRecordingInfo(SetRecordingInfo {
        row_id: RowId::random(),
        info: RecordingInfo {
            application_id: ApplicationId("test".to_owned()),
            recording_id: RecordingId::random(RecordingType::Data),
            is_official_example: true,
            started: Time::now(),
            recording_source: RecordingSource::RustSdk {
                rustc_version: String::new(),
                llvm_version: String::new(),
            },
            recording_type: re_log_types::RecordingType::Data,
        },
    })];

    let mut file = vec![];
    crate::encoder::encode(messages.iter(), &mut file).unwrap();

    let decoded_messages = Decoder::new(&mut file.as_slice())
        .unwrap()
        .collect::<Result<Vec<LogMsg>, DecodeError>>()
        .unwrap();

    assert_eq!(messages, decoded_messages);
}
