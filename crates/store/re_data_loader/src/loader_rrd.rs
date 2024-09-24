use std::{io::Read, sync::mpsc::Receiver};

use re_log_encoding::decoder::Decoder;

// ---

/// Loads data from any `rrd` file or in-memory contents.
pub struct RrdLoader;

impl crate::DataLoader for RrdLoader {
    #[inline]
    fn name(&self) -> String {
        "rerun.data_loaders.Rrd".into()
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn load_from_path(
        &self,
        // NOTE: The Store ID comes from the rrd file itself.
        _settings: &crate::DataLoaderSettings,
        filepath: std::path::PathBuf,
        tx: std::sync::mpsc::Sender<crate::LoadedData>,
    ) -> Result<(), crate::DataLoaderError> {
        use anyhow::Context as _;

        re_tracing::profile_function!(filepath.display().to_string());

        let extension = crate::extension(&filepath);
        if !matches!(extension.as_str(), "rbl" | "rrd") {
            // NOTE: blueprints and recordings has the same file format
            return Err(crate::DataLoaderError::Incompatible(filepath.clone()));
        }

        re_log::debug!(
            ?filepath,
            loader = self.name(),
            "Loading rrd data from filesystem…",
        );

        let version_policy = re_log_encoding::decoder::VersionPolicy::Warn;

        match extension.as_str() {
            "rbl" => {
                // We assume .rbl is not streamed and no retrying after seeing EOF is needed.
                // Otherwise we'd risk retrying to read .rbl file that has no end-of-stream header and
                // blocking the UI update thread indefinitely and making the viewer unresponsive (as .rbl
                // files are sometimes read on UI update).
                let file = std::fs::File::open(&filepath)
                    .with_context(|| format!("Failed to open file {filepath:?}"))?;
                let file = std::io::BufReader::new(file);

                let decoder = Decoder::new(version_policy, file)?;

                // NOTE: This is IO bound, it must run on a dedicated thread, not the shared rayon thread pool.
                std::thread::Builder::new()
                    .name(format!("decode_and_stream({filepath:?})"))
                    .spawn({
                        let filepath = filepath.clone();
                        move || {
                            decode_and_stream(&filepath, &tx, decoder);
                        }
                    })
                    .with_context(|| format!("Failed to open spawn IO thread for {filepath:?}"))?;
            }
            "rrd" => {
                // For .rrd files we retry reading despite reaching EOF to support live (writer) streaming.
                // Decoder will give up when it sees end of file marker (i.e. end-of-stream message header)
                let retryable_reader = RetryableFileReader::new(&filepath).with_context(|| {
                    format!("failed to create retryable file reader for {filepath:?}")
                })?;
                let decoder = Decoder::new(version_policy, retryable_reader)?;

                // NOTE: This is IO bound, it must run on a dedicated thread, not the shared rayon thread pool.
                std::thread::Builder::new()
                    .name(format!("decode_and_stream({filepath:?})"))
                    .spawn({
                        let filepath = filepath.clone();
                        move || {
                            decode_and_stream(&filepath, &tx, decoder);
                        }
                    })
                    .with_context(|| format!("Failed to open spawn IO thread for {filepath:?}"))?;
            }
            _ => unreachable!(),
        }

        Ok(())
    }

    fn load_from_file_contents(
        &self,
        // NOTE: The Store ID comes from the rrd file itself.
        _settings: &crate::DataLoaderSettings,
        filepath: std::path::PathBuf,
        contents: std::borrow::Cow<'_, [u8]>,
        tx: std::sync::mpsc::Sender<crate::LoadedData>,
    ) -> Result<(), crate::DataLoaderError> {
        re_tracing::profile_function!(filepath.display().to_string());

        let extension = crate::extension(&filepath);
        if !matches!(extension.as_str(), "rbl" | "rrd") {
            // NOTE: blueprints and recordings has the same file format
            return Err(crate::DataLoaderError::Incompatible(filepath));
        }

        let version_policy = re_log_encoding::decoder::VersionPolicy::Warn;
        let contents = std::io::Cursor::new(contents);
        let decoder = match re_log_encoding::decoder::Decoder::new(version_policy, contents) {
            Ok(decoder) => decoder,
            Err(err) => match err {
                // simply not interested
                re_log_encoding::decoder::DecodeError::NotAnRrd
                | re_log_encoding::decoder::DecodeError::Options(_) => return Ok(()),
                _ => return Err(err.into()),
            },
        };

        decode_and_stream(&filepath, &tx, decoder);

        Ok(())
    }
}

fn decode_and_stream<R: std::io::Read>(
    filepath: &std::path::Path,
    tx: &std::sync::mpsc::Sender<crate::LoadedData>,
    decoder: Decoder<R>,
) {
    re_tracing::profile_function!(filepath.display().to_string());

    for msg in decoder {
        let msg = match msg {
            Ok(msg) => msg,
            Err(err) => {
                re_log::warn_once!("Failed to decode message in {filepath:?}: {err}");
                continue;
            }
        };
        if tx.send(msg.into()).is_err() {
            break; // The other end has decided to hang up, not our problem.
        }
    }
}

// Retryable file reader that keeps retrying to read more data despite
// reading zero bytes or reaching EOF.
struct RetryableFileReader {
    reader: std::io::BufReader<std::fs::File>,
    rx: Receiver<notify::Result<notify::Event>>,
    #[allow(dead_code)]
    watcher: notify::RecommendedWatcher,
}

impl RetryableFileReader {
    #[cfg(not(target_arch = "wasm32"))]
    fn new(filepath: &std::path::Path) -> Result<Self, crate::DataLoaderError> {
        use anyhow::Context as _;
        use notify::{RecursiveMode, Watcher};

        let file = std::fs::File::open(filepath)
            .with_context(|| format!("Failed to open file {filepath:?}"))?;
        let reader = std::io::BufReader::new(file);

        let (tx, rx) = std::sync::mpsc::channel();
        let mut watcher = notify::recommended_watcher(tx)
            .with_context(|| format!("failed to create file watcher for {filepath:?}"))?;

        watcher
            .watch(filepath, RecursiveMode::NonRecursive)
            .with_context(|| format!("failed to watch file changes on {filepath:?}"))?;

        Ok(Self {
            reader,
            rx,
            watcher,
        })
    }
}

impl Read for RetryableFileReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        loop {
            match self.reader.read(buf) {
                Ok(0) => self.block_until_file_changes()?,
                Ok(n) => {
                    return Ok(n);
                }
                Err(err) => match err.kind() {
                    std::io::ErrorKind::Interrupted => continue,
                    _ => return Err(err),
                },
            };
        }
    }
}

impl RetryableFileReader {
    fn block_until_file_changes(&self) -> std::io::Result<usize> {
        #[allow(clippy::disallowed_methods)]
        match self.rx.recv() {
            Ok(Ok(event)) => match event.kind {
                notify::EventKind::Remove(_) => Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "file removed",
                )),
                _ => Ok(0),
            },
            Ok(Err(err)) => Err(std::io::Error::new(std::io::ErrorKind::Other, err)),
            Err(err) => Err(std::io::Error::new(std::io::ErrorKind::Other, err)),
        }
    }
}

#[cfg(test)]
mod tests {
    use re_build_info::CrateVersion;
    use re_chunk::RowId;
    use re_log_encoding::{decoder, encoder::Encoder};
    use re_log_types::{
        ApplicationId, LogMsg, SetStoreInfo, StoreId, StoreInfo, StoreKind, StoreSource, Time,
    };
    use tempfile::NamedTempFile;

    use super::*;

    #[test]
    fn test_loading_with_retryable_reader() {
        let rrd_file = NamedTempFile::new().unwrap();
        let rrd_file_path = rrd_file.path().to_owned();

        let mut encoder = Encoder::new(
            re_build_info::CrateVersion::LOCAL,
            re_log_encoding::EncodingOptions::UNCOMPRESSED,
            rrd_file,
        )
        .unwrap();

        fn new_message() -> LogMsg {
            LogMsg::SetStoreInfo(SetStoreInfo {
                row_id: *RowId::new(),
                info: StoreInfo {
                    application_id: ApplicationId("test".to_owned()),
                    store_id: StoreId::random(StoreKind::Recording),
                    cloned_from: None,
                    is_official_example: true,
                    started: Time::now(),
                    store_source: StoreSource::RustSdk {
                        rustc_version: String::new(),
                        llvm_version: String::new(),
                    },
                    store_version: Some(CrateVersion::LOCAL),
                },
            })
        }

        let messages = (0..5).map(|_| new_message()).collect::<Vec<_>>();

        for m in &messages {
            encoder.append(m).expect("failed to append message");
        }

        let reader = RetryableFileReader::new(&rrd_file_path).unwrap();
        let mut decoder = Decoder::new(decoder::VersionPolicy::Warn, reader).unwrap();

        // we should be able to read 5 messages that we wrote
        let decoded_messages = (0..5)
            .map(|_| decoder.next().unwrap().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(messages, decoded_messages);

        // as we're using retryable reader, we should be able to read more messages that we're now going to append
        let decoder_handle = std::thread::Builder::new()
            .name("background decoder".into())
            .spawn(move || {
                let mut remaining = Vec::new();
                for msg in decoder {
                    let msg = msg.unwrap();
                    remaining.push(msg);
                }

                remaining
            })
            .unwrap();

        // append more messages to the file
        let more_messages = (0..100).map(|_| new_message()).collect::<Vec<_>>();
        for m in &more_messages {
            encoder.append(m).unwrap();
        }
        // close the stream to stop the decoder reading, otherwise with retryable reader we'd be waiting indefinitely
        encoder.finish().unwrap();

        let remaining_messages = decoder_handle.join().unwrap();

        assert_eq!(more_messages, remaining_messages);
    }
}
