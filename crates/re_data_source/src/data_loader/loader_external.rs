/// To register a new external data loader, simply add an executable in your $PATH whose name
/// starts with this prefix.
pub const EXTERNAL_DATA_LOADER_PREFIX: &str = "rerun-loader";

// TODO: this is the stdio integration
// TODO: binary plugin example and indicate how to not care about ext
pub struct ExternalDataLoader;

impl crate::DataLoader for ExternalDataLoader {
    #[inline]
    fn name(&self) -> String {
        "rerun.data_loaders.External".into()
    }

    fn load_from_file(
        &self,
        store_id: re_log_types::StoreId,
        filepath: std::path::PathBuf,
        tx: std::sync::mpsc::Sender<crate::LoadedData>,
    ) -> Result<(), crate::DataLoaderError> {
        use std::io::Cursor;
        use std::process::{Command, Stdio};

        re_tracing::profile_function!(filepath.display().to_string());

        for exe in crate::EXTERNAL_LOADERS.iter() {
            let store_id = store_id.clone();
            let filepath = filepath.clone();
            let tx = tx.clone();

            // NOTE: spawn is fine, the entire loader is native-only.
            rayon::spawn(move || {
                let child = Command::new(exe)
                    .arg(filepath.clone())
                    // TODO: effectively a public API now...!
                    .args(["--recording-id".to_owned(), store_id.to_string()])
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn();

                let child = match child {
                    Ok(child) => child,
                    Err(err) => {
                        re_log::error!(?filepath, loader = ?exe, %err, "Failed to execute external loader");
                        return;
                    }
                };

                // TODO: is this streaming at all? clearly it's not, but it should be.
                let output = match child.wait_with_output() {
                    Ok(output) => output,
                    Err(err) => {
                        re_log::error!(?filepath, loader = ?exe, %err, "Failed to execute external loader");
                        return;
                    }
                };

                // TODO: test this manually
                if !output.status.success() {
                    let reason = String::from_utf8_lossy(&output.stderr);
                    re_log::error!(?filepath, loader = ?exe, %reason, "Failed to execute external loader");
                    return;
                }

                if output.stdout.is_empty() {
                    return;
                }

                re_log::debug!(?filepath, loader = ?exe, "Loading data from filesystem using external loader…",);

                let version_policy = re_log_encoding::decoder::VersionPolicy::Warn;
                let cursor = Cursor::new(output.stdout);
                let decoder = match re_log_encoding::decoder::Decoder::new(version_policy, cursor) {
                    Ok(decoder) => decoder,
                    Err(err) => {
                        re_log::error!(?filepath, loader = ?exe, %err, "Failed to decode external loader's output");
                        return;
                    }
                };

                decode_and_stream(&filepath, &tx, decoder);
            });
        }

        Ok(())
    }

    #[inline]
    fn load_from_file_contents(
        &self,
        _store_id: re_log_types::StoreId,
        _path: std::path::PathBuf,
        _contents: std::borrow::Cow<'_, [u8]>,
        _tx: std::sync::mpsc::Sender<crate::LoadedData>,
    ) -> Result<(), crate::DataLoaderError> {
        // TODO: support stdin on plugins too uuuuuuuh
        // TODO: prob gotta explain
        Ok(()) // simply not interested
    }
}

fn decode_and_stream<R: std::io::Read>(
    filepath: &std::path::Path,
    tx: &std::sync::mpsc::Sender<crate::LoadedData>,
    decoder: re_log_encoding::decoder::Decoder<R>,
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
