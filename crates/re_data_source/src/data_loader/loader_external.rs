use std::io::Read;

use once_cell::sync::Lazy;

// ---

/// To register a new external data loader, simply add an executable in your $PATH whose name
/// starts with this prefix.
pub const EXTERNAL_DATA_LOADER_PREFIX: &str = "rerun-loader-";

/// When an external [`crate::DataLoader`] is asked to load some data that it doesn't know
/// how to load, it should exit with this exit code.
// NOTE: Always keep in sync with other languages.
pub const EXTERNAL_DATA_LOADER_INCOMPATIBLE_EXIT_CODE: i32 = 66;

/// Keeps track of the paths all external executable [`crate::DataLoader`]s.
///
/// Lazy initialized the first time a file is opened by running a full scan of the `$PATH`.
///
/// External loaders are _not_ registered on a per-extension basis: we want users to be able to
/// filter data on a much more fine-grained basis that just file extensions (e.g. checking the file
/// itself for magic bytes).
pub static EXTERNAL_LOADER_PATHS: Lazy<Vec<std::path::PathBuf>> = Lazy::new(|| {
    re_tracing::profile_function!();

    use walkdir::WalkDir;

    let dirpaths = std::env::var("PATH")
        .ok()
        .into_iter()
        .flat_map(|paths| paths.split(':').map(ToOwned::to_owned).collect::<Vec<_>>())
        .map(std::path::PathBuf::from);

    let executables: ahash::HashSet<_> = dirpaths
        .into_iter()
        .flat_map(|dirpath| {
            WalkDir::new(dirpath).into_iter().filter_map(|entry| {
                let Ok(entry) = entry else {
                    return None;
                };
                let filepath = entry.path();
                let is_rerun_loader = filepath.file_name().map_or(false, |filename| {
                    filename
                        .to_string_lossy()
                        .starts_with(EXTERNAL_DATA_LOADER_PREFIX)
                });
                (filepath.is_file() && is_rerun_loader).then(|| filepath.to_owned())
            })
        })
        .collect();

    // NOTE: We call all available loaders and do so in parallel: order is irrelevant here.
    executables.into_iter().collect()
});

/// Iterator over all registered external [`crate::DataLoader`]s.
#[inline]
pub fn iter_external_loaders() -> impl ExactSizeIterator<Item = std::path::PathBuf> {
    EXTERNAL_LOADER_PATHS.iter().cloned()
}

// ---

/// A [`crate::DataLoader`] that forwards the path to load to all executables present in
/// the user's `PATH` with a name that starts with [`EXTERNAL_DATA_LOADER_PREFIX`].
///
/// The external loaders are expected to log rrd data to their standard output.
///
/// Refer to our `external_data_loader` example for more information.
///
/// Checkout our [guide](https://www.rerun.io/docs/howto/open-any-file?speculative-link) on
/// how to implement external loaders.
pub struct ExternalLoader;

impl crate::DataLoader for ExternalLoader {
    #[inline]
    fn name(&self) -> String {
        "rerun.data_loaders.External".into()
    }

    fn load_from_path(
        &self,
        store_id: re_log_types::StoreId,
        filepath: std::path::PathBuf,
        tx: std::sync::mpsc::Sender<crate::LoadedData>,
    ) -> Result<(), crate::DataLoaderError> {
        use std::process::{Command, Stdio};

        re_tracing::profile_function!(filepath.display().to_string());

        #[derive(PartialEq, Eq)]
        struct CompatibleLoaderFound;
        let (tx_feedback, rx_feedback) = std::sync::mpsc::channel::<CompatibleLoaderFound>();

        for exe in EXTERNAL_LOADER_PATHS.iter() {
            let store_id = store_id.clone();
            let filepath = filepath.clone();
            let tx = tx.clone();
            let tx_feedback = tx_feedback.clone();

            rayon::spawn(move || {
                re_tracing::profile_function!(exe.to_string_lossy());

                let child = Command::new(exe)
                    .arg(filepath.clone())
                    .args(["--recording-id".to_owned(), store_id.to_string()])
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn();

                let mut child = match child {
                    Ok(child) => child,
                    Err(err) => {
                        re_log::error!(?filepath, loader = ?exe, %err, "Failed to execute external loader");
                        return;
                    }
                };

                let Some(stdout) = child.stdout.take() else {
                    let reason = "stdout unreachable";
                    re_log::error!(?filepath, loader = ?exe, %reason, "Failed to execute external loader");
                    return;
                };
                let Some(stderr) = child.stderr.take() else {
                    let reason = "stderr unreachable";
                    re_log::error!(?filepath, loader = ?exe, %reason, "Failed to execute external loader");
                    return;
                };

                re_log::debug!(?filepath, loader = ?exe, "Loading data from filesystem using external loader…",);

                let version_policy = re_log_encoding::decoder::VersionPolicy::Warn;
                let stdout = std::io::BufReader::new(stdout);
                match re_log_encoding::decoder::Decoder::new(version_policy, stdout) {
                    Ok(decoder) => {
                        let filepath = filepath.clone();
                        let tx = tx.clone();
                        // NOTE: This is completely IO bound, it must run on a dedicated thread, not the shared
                        // rayon thread pool.
                        if let Err(err) = std::thread::Builder::new()
                            .name(format!("decode_and_stream({filepath:?})"))
                            .spawn({
                                let filepath = filepath.clone();
                                move || {
                                    decode_and_stream(&filepath, &tx, decoder);
                                }
                            })
                        {
                            re_log::error!(?filepath, loader = ?exe, %err, "Failed to open spawn IO thread");
                            return;
                        }
                    }
                    Err(re_log_encoding::decoder::DecodeError::Read(_)) => {
                        // The child was not interested in that file and left without logging
                        // anything.
                        // That's fine, we just need to make sure to check its exit status further
                        // down, still.
                    }
                    Err(err) => {
                        re_log::error!(?filepath, loader = ?exe, %err, "Failed to decode external loader's output");
                        return;
                    }
                };

                let status = match child.wait() {
                    Ok(output) => output,
                    Err(err) => {
                        re_log::error!(?filepath, loader = ?exe, %err, "Failed to execute external loader");
                        return;
                    }
                };

                // NOTE: We assume that plugins are compatible until proven otherwise.
                let is_compatible =
                    status.code() != Some(crate::EXTERNAL_DATA_LOADER_INCOMPATIBLE_EXIT_CODE);

                if is_compatible && !status.success() {
                    let mut stderr = std::io::BufReader::new(stderr);
                    let mut reason = String::new();
                    stderr.read_to_string(&mut reason).ok();
                    re_log::error!(?filepath, loader = ?exe, %reason, "Failed to execute external loader");
                }

                if is_compatible {
                    re_log::debug!(loader = ?exe, ?filepath, "compatible external loader found");
                    tx_feedback.send(CompatibleLoaderFound).ok();
                }
            });
        }

        re_tracing::profile_wait!("compatible_loader");

        drop(tx_feedback);

        let any_compatible_loader = rx_feedback.recv() == Ok(CompatibleLoaderFound);
        if !any_compatible_loader {
            // NOTE: The only way to get here is if all loaders closed then sending end of the
            // channel without sending anything, i.e. none of them are compatible.
            return Err(crate::DataLoaderError::Incompatible(filepath.clone()));
        }

        Ok(())
    }

    #[inline]
    fn load_from_file_contents(
        &self,
        _store_id: re_log_types::StoreId,
        path: std::path::PathBuf,
        _contents: std::borrow::Cow<'_, [u8]>,
        _tx: std::sync::mpsc::Sender<crate::LoadedData>,
    ) -> Result<(), crate::DataLoaderError> {
        // TODO(cmc): You could imagine a world where plugins can be streamed rrd data via their
        // standard input… but today is not world.
        Err(crate::DataLoaderError::Incompatible(path))
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
