//! Saving/loading [`LogMsg`]:es to/from a file.
use crate::LogMsg;

#[cfg(feature = "save")]
#[cfg(not(target_arch = "wasm32"))]
pub fn encode<'a>(
    messages: impl Iterator<Item = &'a LogMsg>,
    mut write: impl std::io::Write,
) -> anyhow::Result<()> {
    use anyhow::Context as _;
    use std::io::Write as _;

    write.write_all(b"RRF0").context("header")?;
    write.write_all(&[0, 0, 0, 0]).context("header")?; // reserved for future use

    let level = 3;
    let mut encoder = zstd::stream::Encoder::new(write, level).context("zstd start")?;

    let mut buffer = vec![];

    for message in messages {
        buffer.clear();
        rmp_serde::encode::write_named(&mut buffer, message).context("MessagePack encoding")?;
        encoder
            .write_all(&(buffer.len() as u64).to_le_bytes())
            .context("zstd write")?;
        encoder.write_all(&buffer).context("zstd write")?;
    }

    encoder.finish().context("zstd finish")?;

    Ok(())
}

// ----------------------------------------------------------------------------
// native

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
        use anyhow::Context as _;

        let mut header = [0_u8; 4];
        read.read_exact(&mut header).context("missing header")?;
        anyhow::ensure!(&header == b"RRF0", "Not a rerun file");
        read.read_exact(&mut header).context("missing header")?;
        anyhow::ensure!(header == [0, 0, 0, 0], "Incompatible rerun file format");

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
        use std::io::Read as _;

        let mut len = [0_u8; 8];
        self.zdecoder.read_exact(&mut len).ok()?;
        let len = u64::from_le_bytes(len) as usize;

        self.buffer.resize(len, 0);
        if let Err(err) = self.zdecoder.read_exact(&mut self.buffer) {
            return Some(Err(anyhow::anyhow!("zstd: {err}")));
        }

        match rmp_serde::from_read(&mut self.buffer.as_slice()) {
            Ok(msg) => Some(Ok(msg)),
            Err(err) => Some(Err(anyhow::anyhow!("MessagePack: {err}"))),
        }
    }
}

// ----------------------------------------------------------------------------
// wasm:

#[cfg(feature = "load")]
#[cfg(target_arch = "wasm32")]
pub struct Decoder<'r> {
    zdecoder: ruzstd::StreamingDecoder<'r>,
    buffer: Vec<u8>,
}

#[cfg(feature = "load")]
#[cfg(target_arch = "wasm32")]
impl<'r> Decoder<'r> {
    pub fn new(read: &'r mut dyn std::io::Read) -> anyhow::Result<Self> {
        use anyhow::Context as _;

        let mut header = [0_u8; 4];
        read.read_exact(&mut header).context("missing header")?;
        anyhow::ensure!(&header == b"RRF0", "Not a rerun file");
        read.read_exact(&mut header).context("missing header")?;
        anyhow::ensure!(header == [0, 0, 0, 0], "Incompatible rerun file format");

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
impl<'r> Iterator for Decoder<'r> {
    type Item = anyhow::Result<LogMsg>;
    fn next(&mut self) -> Option<Self::Item> {
        use std::io::Read as _;

        let mut len = [0_u8; 8];
        self.zdecoder.read_exact(&mut len).ok()?;
        let len = u64::from_le_bytes(len) as usize;

        self.buffer.resize(len, 0);
        if let Err(err) = self.zdecoder.read_exact(&mut self.buffer) {
            return Some(Err(anyhow::anyhow!("ruzstd: {err}")));
        }

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
    use crate::*;

    // Some fake log data
    let time_point = time_point([(
        "log_time",
        Time::from_ns_since_epoch(1_649_934_625_012_345_678).into(),
    )]);
    let messages = vec![
        LogMsg::from(data_msg(
            &time_point,
            ObjPathBuilder::from("foo") / "bar",
            "baz",
            42,
        )),
        LogMsg::from(data_msg(
            &time_point,
            ObjPathBuilder::from("badger") / "mushroom",
            "snake",
            1337.0,
        )),
        LogMsg::from(data_msg(
            &time_point,
            ObjPathBuilder::from("badger") / "mushroom",
            "pos",
            Data::Vec3([1.0, 2.5, 3.0]),
        )),
    ];

    let mut file = vec![];
    encode(messages.iter(), &mut file).unwrap();

    let decoded_messages = Decoder::new(&mut file.as_slice())
        .unwrap()
        .collect::<anyhow::Result<Vec<LogMsg>>>()
        .unwrap();

    assert_eq!(messages, decoded_messages);
}
