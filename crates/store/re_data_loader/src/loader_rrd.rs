#[cfg(not(target_arch = "wasm32"))]
use crossbeam::channel::Receiver;
use re_log_encoding::Decoder;
use re_log_types::ApplicationId;

use crate::{DataLoader as _, LoadedData};

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
        settings: &crate::DataLoaderSettings,
        filepath: std::path::PathBuf,
        tx: std::sync::mpsc::Sender<crate::LoadedData>,
    ) -> Result<(), crate::DataLoaderError> {
        use anyhow::Context as _;

        re_tracing::profile_function!(filepath.display().to_string());

        let mut extension = crate::extension(&filepath);
        if !matches!(extension.as_str(), "rbl" | "rrd") {
            if filepath.is_file() || filepath.is_dir() {
                // NOTE: blueprints and recordings have the same file format
                return Err(crate::DataLoaderError::Incompatible(filepath.clone()));
            } else {
                // NOTE(1): If this is some kind of virtual file (fifo, socket, pipe, etc), then we
                // always assume it's an RRD stream by default.
                //
                // NOTE(2): Because waiting for an end-of-stream marker on a pipe doesn't make sense,
                // we tag it as `rbl` instead of `rrd` (but really this just means: please don't block
                // indefinitely).
                extension = "rbl".to_owned();
            }
        }

        re_log::debug!(
            ?filepath,
            loader = self.name(),
            "Loading rrd data from filesystem…",
        );

        match extension.as_str() {
            "rbl" => {
                // We assume .rbl is not streamed and no retrying after seeing EOF is needed.
                // Otherwise we'd risk retrying to read .rbl file that has no end-of-stream header and
                // blocking the UI update thread indefinitely and making the viewer unresponsive (as .rbl
                // files are sometimes read on UI update).
                let file = std::fs::File::open(&filepath)
                    .with_context(|| format!("Failed to open file {filepath:?}"))?;
                let file = std::io::BufReader::new(file);

                let messages = Decoder::decode_eager(file)?;

                // NOTE: This is IO bound, it must run on a dedicated thread, not the shared rayon thread pool.
                std::thread::Builder::new()
                    .name(format!("decode_and_stream({filepath:?})"))
                    .spawn({
                        let filepath = filepath.clone();
                        let settings = settings.clone();
                        move || {
                            decode_and_stream(
                                &filepath,
                                &tx,
                                messages,
                                settings
                                    .opened_store_id
                                    .as_ref()
                                    .map(|store_id| store_id.application_id()),
                                // We never want to patch blueprints' store IDs, only their app IDs.
                                None,
                            );
                        }
                    })
                    .with_context(|| format!("Failed to spawn IO thread for {filepath:?}"))?;
            }

            "rrd" => {
                // For .rrd files we retry reading despite reaching EOF to support live (writer) streaming.
                // Decoder will give up when it sees end of file marker (i.e. end-of-stream message header)
                let retryable_reader = RetryableFileReader::new(&filepath).with_context(|| {
                    format!("failed to create retryable file reader for {filepath:?}")
                })?;
                let wait_for_eos = true;
                let messages = Decoder::decode_eager_with_opts(retryable_reader, wait_for_eos)?;

                // NOTE: This is IO bound, it must run on a dedicated thread, not the shared rayon thread pool.
                std::thread::Builder::new()
                    .name(format!("decode_and_stream({filepath:?})"))
                    .spawn({
                        let filepath = filepath.clone();
                        move || {
                            decode_and_stream(
                                &filepath, &tx, messages,
                                // Never use import semantics for .rrd files
                                None, None,
                            );
                        }
                    })
                    .with_context(|| format!("Failed to spawn IO thread for {filepath:?}"))?;
            }
            _ => unreachable!(),
        }

        Ok(())
    }

    fn load_from_file_contents(
        &self,
        settings: &crate::DataLoaderSettings,
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

        let contents = std::io::Cursor::new(contents);
        let messages = match Decoder::decode_eager(contents) {
            Ok(decoder) => decoder,
            Err(err) => match err {
                // simply not interested
                re_log_encoding::DecodeError::Codec(
                    re_log_encoding::rrd::CodecError::NotAnRrd(_)
                    | re_log_encoding::rrd::CodecError::InvalidOptions(_),
                ) => return Ok(()),
                _ => return Err(err.into()),
            },
        };

        // * We never want to patch blueprints' store IDs, only their app IDs.
        // * We neer use import semantics at all for .rrd files.
        let forced_application_id = if extension == "rbl" {
            settings
                .opened_store_id
                .as_ref()
                .map(|store_id| store_id.application_id())
        } else {
            None
        };
        let forced_recording_id = None;

        decode_and_stream(
            &filepath,
            &tx,
            messages,
            forced_application_id,
            forced_recording_id,
        );

        Ok(())
    }
}

fn decode_and_stream(
    filepath: &std::path::Path,
    tx: &std::sync::mpsc::Sender<crate::LoadedData>,
    msgs: impl Iterator<Item = Result<re_log_types::LogMsg, re_log_encoding::DecodeError>>,
    forced_application_id: Option<&ApplicationId>,
    forced_recording_id: Option<&String>,
) {
    re_tracing::profile_function!(filepath.display().to_string());

    for msg in msgs {
        let msg = match msg {
            Ok(msg) => msg,
            Err(err) => {
                re_log::warn_once!("Failed to decode message in {filepath:?}: {err}");
                continue;
            }
        };

        let msg = if forced_application_id.is_some() || forced_recording_id.is_some() {
            match msg {
                re_log_types::LogMsg::SetStoreInfo(set_store_info) => {
                    let mut store_id = set_store_info.info.store_id.clone();
                    if let Some(forced_application_id) = forced_application_id {
                        store_id = store_id.with_application_id(forced_application_id.clone());
                    }
                    if let Some(forced_recording_id) = forced_recording_id {
                        store_id = store_id.with_recording_id(forced_recording_id.clone());
                    }

                    re_log_types::LogMsg::SetStoreInfo(re_log_types::SetStoreInfo {
                        info: re_log_types::StoreInfo {
                            store_id,
                            ..set_store_info.info
                        },
                        ..set_store_info
                    })
                }

                re_log_types::LogMsg::ArrowMsg(mut store_id, arrow_msg) => {
                    if let Some(forced_application_id) = forced_application_id {
                        store_id = store_id.with_application_id(forced_application_id.clone());
                    }
                    if let Some(forced_recording_id) = forced_recording_id {
                        store_id = store_id.with_recording_id(forced_recording_id.clone());
                    }

                    re_log_types::LogMsg::ArrowMsg(store_id, arrow_msg)
                }

                re_log_types::LogMsg::BlueprintActivationCommand(blueprint_activation_command) => {
                    let mut blueprint_id = blueprint_activation_command.blueprint_id.clone();
                    if let Some(forced_application_id) = forced_application_id {
                        blueprint_id =
                            blueprint_id.with_application_id(forced_application_id.clone());
                    }
                    re_log_types::LogMsg::BlueprintActivationCommand(
                        re_log_types::BlueprintActivationCommand {
                            blueprint_id,
                            ..blueprint_activation_command
                        },
                    )
                }
            }
        } else {
            msg
        };

        let data = LoadedData::LogMsg(RrdLoader::name(&RrdLoader), msg);
        if tx.send(data).is_err() {
            break; // The other end has decided to hang up, not our problem.
        }
    }
}

// Retryable file reader that keeps retrying to read more data despite
// reading zero bytes or reaching EOF.
#[cfg(not(target_arch = "wasm32"))]
struct RetryableFileReader {
    reader: std::io::BufReader<std::fs::File>,
    rx_file_notifs: Receiver<notify::Result<notify::Event>>,
    rx_ticker: Receiver<std::time::Instant>,

    #[expect(dead_code)]
    watcher: notify::RecommendedWatcher,
}

#[cfg(not(target_arch = "wasm32"))]
impl RetryableFileReader {
    fn new(filepath: &std::path::Path) -> Result<Self, crate::DataLoaderError> {
        use anyhow::Context as _;
        use notify::{RecursiveMode, Watcher as _};

        let file = std::fs::File::open(filepath)
            .with_context(|| format!("Failed to open file {filepath:?}"))?;
        let reader = std::io::BufReader::new(file);

        #[cfg(not(any(target_os = "windows", target_arch = "wasm32")))]
        re_crash_handler::sigint::track_sigint();

        // 50ms is just a nice tradeoff: we just need the delay to not be perceptible by a human
        // while not needlessly hammering the CPU.
        let rx_ticker = crossbeam::channel::tick(std::time::Duration::from_millis(50));

        let (tx_file_notifs, rx_file_notifs) = crossbeam::channel::bounded(32 * 1024);
        let mut watcher = notify::recommended_watcher(tx_file_notifs)
            .with_context(|| format!("failed to create file watcher for {filepath:?}"))?;

        watcher
            .watch(filepath, RecursiveMode::NonRecursive)
            .with_context(|| format!("failed to watch file changes on {filepath:?}"))?;

        Ok(Self {
            reader,
            rx_file_notifs,
            rx_ticker,
            watcher,
        })
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl std::io::Read for RetryableFileReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        loop {
            match self.reader.read(buf) {
                Ok(0) => {
                    self.block_until_file_changes()?;
                }
                Ok(n) => {
                    return Ok(n);
                }
                Err(err) => {
                    if err.kind() == std::io::ErrorKind::Interrupted {
                        return Err(err);
                    }
                }
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl std::io::BufRead for RetryableFileReader {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        self.reader.fill_buf()
    }

    fn consume(&mut self, amount: usize) {
        self.reader.consume(amount);
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl RetryableFileReader {
    fn block_until_file_changes(&self) -> std::io::Result<usize> {
        loop {
            crossbeam::select! {
                // Periodically check for SIGINT.
                recv(self.rx_ticker) -> _ => {
                    if re_crash_handler::sigint::was_sigint_ever_caught() {
                        return Err(std::io::Error::new(std::io::ErrorKind::Interrupted, "SIGINT"));
                    }
                }

                // Otherwise check for file notifications.
                recv(self.rx_file_notifs) -> res => {
                    return match res {
                        Ok(Ok(event)) => match event.kind {
                            notify::EventKind::Remove(_) => Err(std::io::Error::new(
                                std::io::ErrorKind::NotFound,
                                "file removed",
                            )),
                            _ => Ok(0),
                        },
                        Ok(Err(err)) => Err(std::io::Error::other(err)),
                        Err(err) => Err(std::io::Error::other(err)),
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use re_chunk::RowId;
    use re_log_encoding::Encoder;
    use re_log_types::{LogMsg, SetStoreInfo, StoreId, StoreInfo, StoreKind, StoreSource};

    use super::*;

    struct DeleteOnDrop {
        path: std::path::PathBuf,
    }

    impl Drop for DeleteOnDrop {
        fn drop(&mut self) {
            std::fs::remove_file(&self.path).ok();
        }
    }

    #[test]
    fn test_loading_with_retryable_reader() {
        // We can't use `tempfile` here since it deletes the file on drop and we want to keep it around for a bit longer.
        let rrd_file_path = std::path::PathBuf::from("testfile.rrd");
        let rrd_file_delete_guard = DeleteOnDrop {
            path: rrd_file_path.clone(),
        };
        std::fs::remove_file(&rrd_file_path).ok(); // Remove the file just in case a previous test crashes hard.
        let rrd_file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(rrd_file_path.to_str().unwrap())
            .unwrap();

        let mut encoder = Encoder::new_eager(
            re_build_info::CrateVersion::LOCAL,
            re_log_encoding::rrd::EncodingOptions::PROTOBUF_UNCOMPRESSED,
            rrd_file,
        )
        .unwrap();

        fn new_message() -> LogMsg {
            LogMsg::SetStoreInfo(SetStoreInfo {
                row_id: *RowId::new(),
                info: StoreInfo::new(
                    StoreId::random(StoreKind::Recording, "test_app"),
                    StoreSource::RustSdk {
                        rustc_version: String::new(),
                        llvm_version: String::new(),
                    },
                ),
            })
        }

        let messages = (0..5).map(|_| new_message()).collect::<Vec<_>>();

        for m in &messages {
            encoder.append(m).expect("failed to append message");
        }
        encoder.flush_blocking().expect("failed to flush messages");

        let reader = RetryableFileReader::new(&rrd_file_path).unwrap();
        let wait_for_eos = true;
        let mut decoder = Decoder::decode_eager_with_opts(reader, wait_for_eos).unwrap();

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
        // Close the encoder and thus the file to make sure that file is actually written out.
        // Otherwise we can't we be sure that the filewatcher will ever see those changes.
        // A simple flush works sometimes, but is not as reliably as closing the file since the OS may still cache the data.
        // (in fact we can't be sure that close is enough either, but it's the best we can do)
        // Note that this test is not entirely representative of the real usecase of having reader and writer on
        // different processes, since file read/write visibility across processes may behave differently.
        encoder.finish().expect("failed to finish encoder");
        drop(encoder);

        let remaining_messages = decoder_handle.join().unwrap();
        assert_eq!(more_messages, remaining_messages);

        // Drop explicitly to make sure that rustc doesn't drop it earlier.
        drop(rrd_file_delete_guard);
    }
}
