/// Errors produced by the mp4 reader.
#[derive(Debug, thiserror::Error)]
pub enum Mp4Error {
    #[error("I/O: {0}")]
    Io(#[from] std::io::Error),

    #[error("Chunk construction: {0}")]
    Chunk(#[from] re_chunk::ChunkError),

    #[error(
        "MP4 file is too large ({0} bytes). \
         Maximum supported blob size is ~2 GiB due to Arrow i32 offset limits."
    )]
    AssetTooLarge(usize),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_too_large_display_is_stable() {
        let err = Mp4Error::AssetTooLarge(3_000_000_000);
        let s = err.to_string();
        assert!(
            s.contains("3000000000 bytes") && s.contains("Maximum supported blob size is ~2 GiB"),
            "unexpected display: {s}"
        );
    }
}
