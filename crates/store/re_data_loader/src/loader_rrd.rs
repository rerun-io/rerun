use std::{
    io::Read,
    path::Path,
    sync::mpsc::{channel, Receiver},
};

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use re_log_encoding::decoder::Decoder;

use crate::DataLoaderError;

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
    rx: Receiver<notify::Result<Event>>,
    #[allow(dead_code)]
    watcher: RecommendedWatcher,
}

impl RetryableFileReader {
    fn new(filepath: &Path) -> Result<Self, DataLoaderError> {
        use anyhow::Context as _;

        let file = std::fs::File::open(filepath)
            .with_context(|| format!("Failed to open file {filepath:?}"))?;
        let reader = std::io::BufReader::new(file);

        let (tx, rx) = channel();
        let mut watcher = notify::recommended_watcher(tx)
            .with_context(|| format!("failed to create file watcher for {filepath:?}"))?;

        watcher
            .watch(filepath, RecursiveMode::NonRecursive)
            .with_context(|| format!("failed to to watch file changes on {filepath:?}"))?;

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
                Ok(0) => match self.rx.recv() {
                    Ok(Ok(event)) => match event.kind {
                        EventKind::Remove(_) => {
                            return Err(std::io::Error::new(
                                std::io::ErrorKind::NotFound,
                                "file removed",
                            ))
                        }
                        _ => continue,
                    },
                    Ok(Err(e)) => {
                        return Err(std::io::Error::new(std::io::ErrorKind::Other, e));
                    }
                    Err(e) => {
                        return Err(std::io::Error::new(std::io::ErrorKind::Other, e));
                    }
                },
                Ok(n) => {
                    return Ok(n);
                }
                Err(err) => match err.kind() {
                    std::io::ErrorKind::Interrupted => continue,
                    _ => return Err(err),
                },
            }
        }
    }
}
