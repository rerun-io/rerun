use re_build_info::CrateVersion;

use crate::{EncodingOptions, FileHeader, Serializer, decoder::DecodeError};

// ---

/// Read encoding options from the beginning of the stream.
pub fn read_options(
    reader: &mut impl std::io::Read,
) -> Result<(CrateVersion, EncodingOptions), DecodeError> {
    let mut data = [0_u8; FileHeader::SIZE];
    reader.read_exact(&mut data).map_err(DecodeError::Read)?;

    options_from_bytes(&data)
}

/// Read encoding options from the beginning of the stream asynchronously.
pub async fn read_options_async(
    reader: &mut (impl tokio::io::AsyncRead + Unpin),
) -> Result<(CrateVersion, EncodingOptions), DecodeError> {
    let mut data = [0_u8; FileHeader::SIZE];

    use tokio::io::AsyncReadExt as _;
    reader
        .read_exact(&mut data)
        .await
        .map_err(DecodeError::Read)?;

    options_from_bytes(&data)
}

pub fn options_from_bytes(bytes: &[u8]) -> Result<(CrateVersion, EncodingOptions), DecodeError> {
    let mut read = std::io::Cursor::new(bytes);

    let FileHeader {
        fourcc: _, // Checked in FileHeader::decode
        version,
        options,
    } = FileHeader::decode(&mut read)?;

    warn_on_version_mismatch(version)?;

    match options.serializer {
        Serializer::Protobuf => {}
    }

    Ok((CrateVersion::from_bytes(version), options))
}

fn warn_on_version_mismatch(encoded_version: [u8; 4]) -> Result<(), DecodeError> {
    // We used 0000 for all .rrd files up until 2023-02-27, post 0.2.0 release:
    let encoded_version = if encoded_version == [0, 0, 0, 0] {
        CrateVersion::new(0, 2, 0)
    } else {
        CrateVersion::from_bytes(encoded_version)
    };

    if encoded_version.major == 0 && encoded_version.minor < 23 {
        // We broke compatibility for 0.23 for (hopefully) the last time.
        Err(DecodeError::IncompatibleRerunVersion {
            file: Box::new(encoded_version),
            local: Box::new(CrateVersion::LOCAL),
        })
    } else if encoded_version <= CrateVersion::LOCAL {
        // Loading old files should be fine, and if it is not, the chunk migration in re_sorbet should already log a warning.
        Ok(())
    } else {
        re_log::warn_once!(
            "Found data stream with Rerun version {encoded_version} which is newer than the local Rerun version ({}). This file may contain data that is not compatible with this version of Rerun. Consider updating Rerun.",
            CrateVersion::LOCAL
        );
        Ok(())
    }
}
