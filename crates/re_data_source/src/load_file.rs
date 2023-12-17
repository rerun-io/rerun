use std::borrow::Cow;

use re_log_types::{FileSource, LogMsg};
use re_smart_channel::Sender;

use crate::{DataLoaderError, LoadedData};

// ---

/// Loads the given `path` using all [`crate::DataLoader`]s available.
///
/// A single `path` might be handled by more than one loader.
///
/// Synchronously checks whether the file exists and can be loaded. Beyond that, all
/// errors are asynchronous and handled directly by the [`crate::DataLoader`]s themselves
/// (i.e. they're logged).
#[cfg(not(target_arch = "wasm32"))]
pub fn load_from_path(
    store_id: &re_log_types::StoreId,
    file_source: FileSource,
    path: &std::path::Path,
    // NOTE: This channel must be unbounded since we serialize all operations when running on wasm.
    tx: &Sender<LogMsg>,
) -> Result<(), DataLoaderError> {
    re_tracing::profile_function!(path.to_string_lossy());

    if !path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "path does not exist: {path:?}",
        )
        .into());
    }

    re_log::info!("Loading {path:?}…");

    let store_info = prepare_store_info(store_id, file_source, path, path.is_dir());
    if let Some(store_info) = store_info {
        if tx.send(store_info).is_err() {
            return Ok(()); // other end has hung up.
        }
    }

    let data = load(store_id, path, None)?;
    send(store_id, data, tx);

    Ok(())
}

/// Loads the given `contents` using all [`crate::DataLoader`]s available.
///
/// A single file might be handled by more than one loader.
///
/// Synchronously checks that the file can be loaded. Beyond that, all errors are asynchronous
/// and handled directly by the [`crate::DataLoader`]s themselves (i.e. they're logged).
///
/// `path` is only used for informational purposes, no data is ever read from the filesystem.
pub fn load_from_file_contents(
    store_id: &re_log_types::StoreId,
    file_source: FileSource,
    filepath: &std::path::Path,
    contents: std::borrow::Cow<'_, [u8]>,
    // NOTE: This channel must be unbounded since we serialize all operations when running on wasm.
    tx: &Sender<LogMsg>,
) -> Result<(), DataLoaderError> {
    re_tracing::profile_function!(filepath.to_string_lossy());

    re_log::info!("Loading {filepath:?}…");

    let store_info = prepare_store_info(store_id, file_source, filepath, false);
    if let Some(store_info) = store_info {
        if tx.send(store_info).is_err() {
            return Ok(()); // other end has hung up.
        }
    }

    let data = load(store_id, filepath, Some(contents))?;
    send(store_id, data, tx);

    Ok(())
}

// ---

/// Empty string if no extension.
#[inline]
pub fn extension(path: &std::path::Path) -> String {
    path.extension()
        .unwrap_or_default()
        .to_ascii_lowercase()
        .to_string_lossy()
        .to_string()
}

/// Returns whether the given path is supported by builtin [`crate::DataLoader`]s.
///
/// This does _not_ access the filesystem.
#[inline]
pub fn is_associated_with_builtin_loader(path: &std::path::Path, is_dir: bool) -> bool {
    is_dir || crate::is_supported_file_extension(&extension(path))
}

/// Prepares an adequate [`re_log_types::StoreInfo`] [`LogMsg`] given the input.
pub(crate) fn prepare_store_info(
    store_id: &re_log_types::StoreId,
    file_source: FileSource,
    path: &std::path::Path,
    is_dir: bool,
) -> Option<LogMsg> {
    re_tracing::profile_function!(path.display().to_string());

    use re_log_types::SetStoreInfo;

    let app_id = re_log_types::ApplicationId(path.display().to_string());
    let store_source = re_log_types::StoreSource::File { file_source };

    let is_builtin = is_associated_with_builtin_loader(path, is_dir);
    let is_rrd = crate::SUPPORTED_RERUN_EXTENSIONS.contains(&extension(path).as_str());

    (!is_rrd && is_builtin).then(|| {
        LogMsg::SetStoreInfo(SetStoreInfo {
            row_id: re_log_types::RowId::new(),
            info: re_log_types::StoreInfo {
                application_id: app_id.clone(),
                store_id: store_id.clone(),
                is_official_example: false,
                started: re_log_types::Time::now(),
                store_source,
                store_kind: re_log_types::StoreKind::Recording,
            },
        })
    })
}

/// Loads the data at `path` using all available [`crate::DataLoader`]s.
///
/// On success, returns a channel with all the [`LoadedData`]:
/// - On native, this is filled asynchronously from other threads.
/// - On wasm, this is pre-filled synchronously.
///
/// There is only one way this function can return an error: not a single [`crate::DataLoader`]
/// (whether it is builtin, custom or external) was capable of loading the data, in which case
/// `DataLoaderError::NotSupported` will be returned.
#[cfg_attr(target_arch = "wasm32", allow(clippy::needless_pass_by_value))]
pub(crate) fn load(
    store_id: &re_log_types::StoreId,
    path: &std::path::Path,
    contents: Option<std::borrow::Cow<'_, [u8]>>,
) -> Result<std::sync::mpsc::Receiver<LoadedData>, DataLoaderError> {
    re_tracing::profile_function!(path.display().to_string());

    // On native we run loaders in parallel so this needs to become static.
    #[cfg(not(target_arch = "wasm32"))]
    let contents: Option<std::sync::Arc<std::borrow::Cow<'static, [u8]>>> =
        contents.map(|contents| std::sync::Arc::new(Cow::Owned(contents.into_owned())));

    let rx_loader = {
        let (tx_loader, rx_loader) = std::sync::mpsc::channel();

        #[cfg(target_arch = "wasm32")]
        let any_compatible_loader = crate::iter_loaders().map(|loader| {
            if let Some(contents) = contents.as_deref() {
                let store_id = store_id.clone();
                let tx_loader = tx_loader.clone();
                let path = path.to_owned();
                let contents = Cow::Borrowed(contents);

                if let Err(err) = loader.load_from_file_contents(store_id, path.clone(), contents, tx_loader) {
                    if err.is_not_supported() {
                        return false;
                    }
                    re_log::error!(?path, loader = loader.name(), %err, "Failed to load data from file");
                }

                true
            } else {
                false
            }
        })
            .reduce(|any_compatible, is_compatible| any_compatible | is_compatible)
            .unwrap_or(false);

        #[cfg(not(target_arch = "wasm32"))]
        let any_compatible_loader = {
            let (tx_feedback, rx_feedback) = std::sync::mpsc::channel();

            for loader in crate::iter_loaders() {
                let loader = std::sync::Arc::clone(&loader);

                let store_id = store_id.clone();
                let path = path.to_owned();
                let contents = contents.clone(); // arc

                let tx_loader = tx_loader.clone();
                let tx_feedback = tx_feedback.clone();

                rayon::spawn(move || {
                    re_tracing::profile_scope!("inner", loader.name());

                    if let Some(contents) = contents.as_deref() {
                        let contents = Cow::Borrowed(contents.as_ref());

                        if let Err(err) = loader.load_from_file_contents(
                            store_id,
                            path.clone(),
                            contents,
                            tx_loader,
                        ) {
                            if err.is_not_supported() {
                                return;
                            }
                            re_log::error!(?path, loader = loader.name(), %err, "Failed to load data");
                        }
                    } else if let Err(err) =
                        loader.load_from_path(store_id, path.clone(), tx_loader)
                    {
                        if err.is_not_supported() {
                            return;
                        }
                        re_log::error!(?path, loader = loader.name(), %err, "Failed to load data from file");
                    }

                    re_log::debug!(loader = loader.name(), ?path, "compatible loader found");
                    tx_feedback.send(()).ok();
                });
            }

            drop(tx_feedback);

            rx_feedback.recv().is_ok() // at least one compatible loader was found
        };

        // Implicitly closing `tx_loader`!

        any_compatible_loader.then_some(rx_loader)
    };

    if let Some(rx_loader) = rx_loader {
        Ok(rx_loader)
    } else {
        Err(DataLoaderError::NotSupported(path.to_owned()))
    }
}

/// Forwards the data in `rx_loader` to `tx`, taking care of necessary conversions, if any.
///
/// Runs asynchronously from another thread on native, synchronously on wasm.
pub(crate) fn send(
    store_id: &re_log_types::StoreId,
    rx_loader: std::sync::mpsc::Receiver<LoadedData>,
    tx: &Sender<LogMsg>,
) {
    spawn({
        re_tracing::profile_function!();

        let tx = tx.clone();
        let store_id = store_id.clone();
        move || {
            // ## Ignoring channel errors
            //
            // Not our problem whether or not the other end has hung up, but we still want to
            // poll the channel in any case so as to make sure that the data producer
            // doesn't get stuck.
            for data in rx_loader {
                let msg = match data.into_log_msg(&store_id) {
                    Ok(msg) => msg,
                    Err(err) => {
                        re_log::error!(%err, %store_id, "Couldn't serialize component data");
                        continue;
                    }
                };
                tx.send(msg).ok();
            }

            tx.quit(None).ok();
        }
    });
}

// NOTE:
// - On native, we parallelize using `rayon`.
// - On wasm, we serialize everything, which works because the data-loading channels are unbounded.

#[cfg(not(target_arch = "wasm32"))]
fn spawn<F>(f: F)
where
    F: FnOnce() + Send + 'static,
{
    rayon::spawn(f);
}

#[cfg(target_arch = "wasm32")]
fn spawn<F>(f: F)
where
    F: FnOnce(),
{
    f();
}
