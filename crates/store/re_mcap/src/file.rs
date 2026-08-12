//! Access to an MCAP file's bytes and file-level information.

use std::sync::Arc;

use parking_lot::Mutex;

use crate::{Error, McapInfo, McapSummarySource, Summary};

struct SummaryWithSource {
    summary: Arc<Summary>,
    source: McapSummarySource,
}

/// An immutable view of an MCAP file's bytes with lazily cached file-level information.
///
/// The underlying bytes must not change for the lifetime of this object.
pub struct McapFile<BytesSource> {
    bytes: BytesSource,

    /// Whether automatic recovery of an incomplete or missing summary is enabled.
    recover: bool,

    /// The low-level [`Summary`] index used by decoding, assembled lazily.
    /// The summary is reference-counted so callers and worker threads can retain it without copying.
    summary: Mutex<Option<SummaryWithSource>>,

    /// File-level [`McapInfo`] for inspection APIs and user interfaces, assembled lazily.
    /// The value is reference-counted so callers can retain the same allocation independently.
    info: Mutex<Option<Arc<McapInfo>>>,
}

impl<BytesSource> McapFile<BytesSource>
where
    BytesSource: AsRef<[u8]>,
{
    /// Create an immutable view over an MCAP byte source.
    ///
    /// When `recover` is true, a missing or invalid embedded summary is reconstructed from the
    /// readable portion of the file.
    pub fn new(bytes: BytesSource, recover: bool) -> Self {
        Self {
            bytes,
            recover,
            summary: Mutex::new(None),
            info: Mutex::new(None),
        }
    }

    /// Return the complete immutable MCAP byte source.
    pub fn bytes(&self) -> &[u8] {
        self.bytes.as_ref()
    }

    /// Whether automatic recovery of an incomplete or missing summary is enabled.
    pub fn recover(&self) -> bool {
        self.recover
    }

    /// Return the cached summary without initializing it.
    pub fn cached_summary(&self) -> Option<Arc<Summary>> {
        self.summary
            .lock()
            .as_ref()
            .map(|cached| Arc::clone(&cached.summary))
    }

    /// Return the derived [`Summary`], parsing or reconstructing it on first use.
    pub fn summary(&self) -> Result<Arc<Summary>, Error> {
        self.summary_with_source().map(|(summary, _source)| summary)
    }

    /// Return the summary and its source, caching them after successful initialization.
    fn summary_with_source(&self) -> Result<(Arc<Summary>, McapSummarySource), Error> {
        let mut cached = self.summary.lock();
        if let Some(cached) = cached.as_ref() {
            return Ok((Arc::clone(&cached.summary), cached.source));
        }

        let (summary, source) =
            crate::recover::read_or_reconstruct_summary_with_source(self.bytes(), self.recover)?;
        let summary = Arc::new(summary);
        *cached = Some(SummaryWithSource {
            summary: Arc::clone(&summary),
            source,
        });
        Ok((summary, source))
    }

    /// Return [`McapInfo`], assembling and caching it on first use.
    pub fn info(&self) -> Result<Arc<McapInfo>, Error> {
        let mut cached = self.info.lock();
        if let Some(info) = cached.as_ref() {
            return Ok(Arc::clone(info));
        }

        let (summary, summary_source) = self.summary_with_source()?;
        let header = crate::info::read_header(self.bytes())?;
        let info = Arc::new(McapInfo::from_summary(
            &header,
            &summary,
            self.bytes(),
            summary_source,
        ));
        *cached = Some(Arc::clone(&info));
        Ok(info)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    /// Tests that concurrent callers share the same cached `Summary` and `McapInfo` allocations.
    #[test]
    fn summary_and_info_are_initialized_once_across_threads() {
        let mut writer = mcap::Writer::new(Cursor::new(Vec::new())).expect("create writer");
        writer.finish().expect("finish writer");
        let bytes = writer.into_inner().into_inner();
        let file = Arc::new(McapFile::new(bytes, false));

        let handles = (0..8)
            .map(|index| {
                let file = Arc::clone(&file);
                std::thread::Builder::new()
                    .name(format!("test-mcap-file-cache-{index}"))
                    .spawn(move || (file.summary().unwrap(), file.info().unwrap()))
                    .expect("spawn cache test thread")
            })
            .collect::<Vec<_>>();
        let results = handles
            .into_iter()
            .map(|handle| handle.join().expect("thread completed"))
            .collect::<Vec<_>>();

        for (summary, info) in &results[1..] {
            assert!(Arc::ptr_eq(&results[0].0, summary));
            assert!(Arc::ptr_eq(&results[0].1, info));
        }
    }
}
