use std::io::SeekFrom;

use futures::{AsyncReadExt as _, AsyncSeekExt as _};

use super::footer_reader::read_rrd_footer_payload;
use crate::rrd::{AsyncReadAt, CodecError};

/// A SHA-256 fingerprint of an RRD stream.
//
// NOTE: For an RRD with a footer, this hashes only the encoded footer payload.
// For a legacy RRD without a footer, this hashes the entire stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RrdFingerprint([u8; 32]);

impl RrdFingerprint {
    /// Returns the SHA-256 digest bytes.
    #[inline]
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Computes an RRD fingerprint without reading chunk payloads when possible.
    ///
    /// When a footer is present, the fingerprint is the SHA-256 of its encoded payload.
    /// Otherwise, the reader is reset and the entire stream is hashed incrementally.
    /// The reader position is unspecified on return.
    pub async fn compute_for_rrd<R: AsyncReadAt>(reader: &mut R) -> Result<Self, CodecError> {
        use sha2::Digest as _;

        if let Some(payload) = read_rrd_footer_payload(reader).await? {
            return Ok(Self(sha2::Sha256::digest(payload).into()));
        }

        reader.seek(SeekFrom::Start(0)).await?;

        let mut hasher = sha2::Sha256::new();
        let mut buffer = vec![0; 64 * 1024];
        loop {
            let read = reader.read(&mut buffer).await?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }

        Ok(Self(hasher.finalize().into()))
    }
}

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod tests {
    use std::fs::File;

    use sha2::Digest as _;

    use super::*;
    use crate::rrd::test_util::{encode_test_rrd, encode_test_rrd_to_file, make_test_chunks};
    use crate::{Decodable as _, StreamFooter};

    #[test]
    fn test_fingerprint_hashes_footer_payload() {
        let chunks = make_test_chunks(5);
        let (file, _store_id) = encode_test_rrd(&chunks);
        let bytes = std::fs::read(file.path()).unwrap();
        let stream_footer =
            StreamFooter::from_rrd_bytes(&bytes[bytes.len() - StreamFooter::ENCODED_SIZE_BYTES..])
                .unwrap();
        let span = stream_footer.entries[0].rrd_footer_byte_span_from_start_excluding_header;
        let start = usize::try_from(span.start).unwrap();
        let end = usize::try_from(span.start + span.len).unwrap();
        let expected: [u8; 32] = sha2::Sha256::digest(&bytes[start..end]).into();

        let mut file = futures::io::AllowStdIo::new(File::open(file.path()).unwrap());
        let fingerprint =
            futures::executor::block_on(RrdFingerprint::compute_for_rrd(&mut file)).unwrap();

        assert_eq!(fingerprint, RrdFingerprint(expected));
    }

    #[test]
    fn test_fingerprint_hashes_legacy_rrd() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let chunks = make_test_chunks(3);
        encode_test_rrd_to_file(file.path(), &chunks, false);
        let bytes = std::fs::read(file.path()).unwrap();
        let expected: [u8; 32] = sha2::Sha256::digest(&bytes).into();

        let mut file = futures::io::AllowStdIo::new(File::open(file.path()).unwrap());
        let fingerprint =
            futures::executor::block_on(RrdFingerprint::compute_for_rrd(&mut file)).unwrap();

        assert_eq!(fingerprint, RrdFingerprint(expected));
    }
}
