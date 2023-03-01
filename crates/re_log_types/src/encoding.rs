//! Encoding/decoding [`LogMsg`]:es as `.rrd` files.

use crate::LogMsg;

// ----------------------------------------------------------------------------
// native encode:

#[cfg(feature = "save")]
#[cfg(not(target_arch = "wasm32"))]
mod encoder {
    use anyhow::Context as _;
    use std::io::Write as _;

    use crate::LogMsg;

    /// Encode a stream of [`LogMsg`] into an `.rrd` file.
    pub struct Encoder<W: std::io::Write> {
        zstd_encoder: zstd::stream::Encoder<'static, W>,
        buffer: Vec<u8>,
    }

    impl<W: std::io::Write> Encoder<W> {
        pub fn new(mut write: W) -> anyhow::Result<Self> {
            let rerun_version = re_build_info::CrateVersion::parse(env!("CARGO_PKG_VERSION"));

            write.write_all(b"RRF0").context("header")?;
            write
                .write_all(&rerun_version.to_bytes())
                .context("header")?;

            let level = 3;
            let zstd_encoder = zstd::stream::Encoder::new(write, level).context("zstd start")?;

            Ok(Self {
                zstd_encoder,
                buffer: vec![],
            })
        }

        pub fn append(&mut self, message: &LogMsg) -> anyhow::Result<()> {
            let Self {
                zstd_encoder,
                buffer,
            } = self;

            buffer.clear();
            rmp_serde::encode::write_named(buffer, message).context("MessagePack encoding")?;

            zstd_encoder
                .write_all(&(buffer.len() as u64).to_le_bytes())
                .context("zstd write")?;
            zstd_encoder.write_all(buffer).context("zstd write")?;

            Ok(())
        }

        pub fn finish(self) -> anyhow::Result<()> {
            self.zstd_encoder.finish().context("zstd finish")?;
            Ok(())
        }
    }

    pub fn encode<'a>(
        messages: impl Iterator<Item = &'a LogMsg>,
        write: impl std::io::Write,
    ) -> anyhow::Result<()> {
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

    if !encoded_version.is_semver_compatible_with(local_version) {
        re_log::warn!("Found log stream with Rerun version {encoded_version}, which is incompatible with the local Rerun version {local_version}. Loading will try to continue, but might fail in subtle ways.");
    }
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
    pub fn new(mut read: R) -> anyhow::Result<Self> {
        crate::profile_function!();
        use anyhow::Context as _;

        let mut header = [0_u8; 4];
        read.read_exact(&mut header).context("missing header")?;
        anyhow::ensure!(&header == b"RRF0", "Not a rerun file");
        read.read_exact(&mut header).context("missing header")?;
        warn_on_version_mismatch(header);

        let zdecoder = zstd::stream::read::Decoder::new(read).context("zstd")?;
        Ok(Self {
            zdecoder,
            buffer: vec![],
        })
    }
}

#[cfg(feature = "load")]
#[cfg(not(target_arch = "wasm32"))]
impl<'r, R: std::io::BufRead> Iterator for Decoder<'r, R> {
    type Item = anyhow::Result<LogMsg>;

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
                return Some(Err(anyhow::anyhow!("zstd: {err}")));
            }
        }

        crate::profile_scope!("MsgPack deser");
        match rmp_serde::from_read(&mut self.buffer.as_slice()) {
            Ok(msg) => Some(Ok(msg)),
            Err(err) => Some(Err(anyhow::anyhow!("MessagePack: {err}"))),
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
    pub fn new(mut read: R) -> anyhow::Result<Self> {
        crate::profile_function!();
        use anyhow::Context as _;

        let mut header = [0_u8; 4];
        read.read_exact(&mut header).context("missing header")?;
        anyhow::ensure!(&header == b"RRF0", "Not a rerun file");
        read.read_exact(&mut header).context("missing header")?;
        warn_on_version_mismatch(header);

        let zdecoder =
            ruzstd::StreamingDecoder::new(read).map_err(|err| anyhow::anyhow!("ruzstd: {err}"))?;
        Ok(Self {
            zdecoder,
            buffer: vec![],
        })
    }
}

#[cfg(feature = "load")]
#[cfg(target_arch = "wasm32")]
impl<R: std::io::Read> Iterator for Decoder<R> {
    type Item = anyhow::Result<LogMsg>;

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
                return Some(Err(anyhow::anyhow!("ruzstd: {err}")));
            }
        }

        crate::profile_scope!("MsgPack deser");
        match rmp_serde::from_read(&mut self.buffer.as_slice()) {
            Ok(msg) => Some(Ok(msg)),
            Err(err) => Some(Err(anyhow::anyhow!("MessagePack: {err}"))),
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
                rust_version: env!("CARGO_PKG_RUST_VERSION").into(),
            },
        },
    })];

    let mut file = vec![];
    encode(messages.iter(), &mut file).unwrap();

    let decoded_messages = Decoder::new(&mut file.as_slice())
        .unwrap()
        .collect::<anyhow::Result<Vec<LogMsg>>>()
        .unwrap();

    assert_eq!(messages, decoded_messages);
}
