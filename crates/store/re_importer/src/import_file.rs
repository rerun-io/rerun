use std::borrow::Cow;

use ahash::{HashMap, HashMapExt as _};
use re_log_channel::LogSender;
use re_log_types::{ApplicationId, FileSource, LogMsg};

use crate::{ImportedData, Importer as _, ImporterError, RrdImporter};

// ---

/// Imports from the given `path` using all [`crate::Importer`]s available.
///
/// A single `path` might be handled by more than one importer.
///
/// Synchronously checks whether the file exists and can be loaded. Beyond that, all
/// errors are asynchronous and handled directly by the [`crate::Importer`]s themselves
/// (i.e. they're logged).
#[cfg(not(target_arch = "wasm32"))]
pub fn import_from_path(
    settings: &crate::ImporterSettings,
    file_source: FileSource,
    path: &std::path::Path,
    // NOTE: This channel must be unbounded since we serialize all operations when running on wasm.
    tx: &LogSender,
) -> Result<(), ImporterError> {
    re_tracing::profile_function!(path.to_string_lossy());

    if !path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("path does not exist: {path:?}"),
        )
        .into());
    }

    re_log::info!("Loading {path:?}…");

    // If no application ID was specified, we derive one from the filename.
    let application_id = settings.application_id.clone().or_else(|| {
        path.file_name()
            .map(|f| f.to_string_lossy().to_string())
            .map(ApplicationId::from)
    });
    let settings = crate::ImporterSettings {
        // When importing a LeRobot dataset, avoid sending a `SetStoreInfo` message since the LeRobot importer handles this automatically.
        force_store_info: !crate::lerobot::is_lerobot_dataset(path),
        application_id,
        ..settings.clone()
    };

    let rx = import(&settings, path, None)?;

    send(settings, file_source, rx, tx);

    Ok(())
}

/// Imports from the given `contents` using all [`crate::Importer`]s available.
///
/// A single file might be handled by more than one importer.
///
/// Synchronously checks that the file can be loaded. Beyond that, all errors are asynchronous
/// and handled directly by the [`crate::Importer`]s themselves (i.e. they're logged).
///
/// `path` is only used for informational purposes, no data is ever read from the filesystem.
pub fn import_from_file_contents(
    settings: &crate::ImporterSettings,
    file_source: FileSource,
    filepath: &std::path::Path,
    contents: std::borrow::Cow<'_, [u8]>,
    // NOTE: This channel must be unbounded since we serialize all operations when running on wasm.
    tx: &LogSender,
) -> Result<(), ImporterError> {
    re_tracing::profile_function!(filepath.to_string_lossy());

    re_log::info!("Loading {filepath:?}…");

    // If no application ID was specified, we derive one from the filename.
    let application_id = settings.application_id.clone().or_else(|| {
        filepath
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .map(ApplicationId::from)
    });

    let settings = crate::ImporterSettings {
        application_id,
        ..settings.clone()
    };

    let data = import(&settings, filepath, Some(contents))?;

    send(settings, file_source, data, tx);

    Ok(())
}

// ---

/// Prepares an adequate [`re_log_types::StoreInfo`] [`LogMsg`] given the input.
pub fn prepare_store_info(store_id: &re_log_types::StoreId, file_source: FileSource) -> LogMsg {
    re_tracing::profile_function!();

    use re_log_types::SetStoreInfo;

    let store_source = re_log_types::StoreSource::File { file_source };

    LogMsg::SetStoreInfo(SetStoreInfo {
        row_id: *re_chunk::RowId::new(),
        info: re_log_types::StoreInfo::new(store_id.clone(), store_source),
    })
}

/// Imports data at `path` using all available [`crate::Importer`]s.
///
/// On success, returns a channel with all the [`ImportedData`]:
/// - On native, this is filled asynchronously from other threads.
/// - On wasm, this is pre-filled synchronously.
///
/// There is only one way this function can return an error: not a single [`crate::Importer`]
/// (whether it is builtin, custom or external) was capable of loading the data, in which case
/// [`ImporterError::Incompatible`] will be returned.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn import(
    settings: &crate::ImporterSettings,
    path: &std::path::Path,
    contents: Option<std::borrow::Cow<'_, [u8]>>,
) -> Result<crossbeam::channel::Receiver<ImportedData>, ImporterError> {
    re_tracing::profile_function!(path.display().to_string());

    // On native we run importers in parallel so this needs to become static.
    let contents: Option<std::sync::Arc<std::borrow::Cow<'static, [u8]>>> =
        contents.map(|contents| std::sync::Arc::new(Cow::Owned(contents.into_owned())));

    let rx_importer = {
        let (tx_importer, rx_importer) = crossbeam::channel::bounded(1024);

        let any_compatible_importer = {
            #[derive(Debug, PartialEq, Eq)]
            struct CompatibleImporterFound;
            let (tx_feedback, rx_feedback) =
                crossbeam::channel::bounded::<CompatibleImporterFound>(128);

            // When loading a file type with native support (.rrd, .mcap, .png, …)
            // then we don't need the overhead and noise of external importers:
            // See <https://github.com/rerun-io/rerun/issues/6530>.
            let importers = {
                use rayon::iter::Either;

                use crate::Importer as _;

                let extension = crate::extension(path);
                if crate::is_supported_file_extension(&extension) {
                    Either::Left(
                        crate::iter_importers()
                            .filter(|importer| importer.name() != crate::ExternalImporter.name()),
                    )
                } else {
                    // We need to use an external importer
                    Either::Right(crate::iter_importers())
                }
            };

            for importer in importers {
                let importer = std::sync::Arc::clone(&importer);

                let settings = settings.clone();
                let path = path.to_owned();
                let contents = contents.clone(); // arc

                let tx_importer = tx_importer.clone();
                let tx_feedback = tx_feedback.clone();

                rayon::spawn(move || {
                    re_tracing::profile_scope!("inner", importer.name());

                    if let Some(contents) = contents.as_deref() {
                        let contents = Cow::Borrowed(contents.as_ref());

                        if let Err(err) = importer.import_from_file_contents(
                            &settings,
                            path.clone(),
                            contents,
                            tx_importer,
                        ) {
                            if err.is_incompatible() {
                                return;
                            }
                            re_log::error!(?path, importer = importer.name(), %err, "Failed to import data");
                        }
                    } else if let Err(err) =
                        importer.import_from_path(&settings, path.clone(), tx_importer)
                    {
                        if err.is_incompatible() {
                            return;
                        }
                        re_log::error!(?path, importer = importer.name(), %err, "Failed to import data from file");
                    }

                    re_log::debug!(
                        importer = importer.name(),
                        ?path,
                        "compatible importer found"
                    );
                    re_quota_channel::send_crossbeam(&tx_feedback, CompatibleImporterFound).ok();
                });
            }

            re_tracing::profile_wait!("compatible_importer");

            drop(tx_feedback);

            rx_feedback.recv() == Ok(CompatibleImporterFound)
        };

        // Implicitly closing `tx_importer`!

        any_compatible_importer.then_some(rx_importer)
    };

    if let Some(rx_importer) = rx_importer {
        Ok(rx_importer)
    } else {
        Err(ImporterError::Incompatible(path.to_owned()))
    }
}

/// Imports data at `path` using all available [`crate::Importer`]s.
///
/// On success, returns a channel (pre-filled synchronously) with all the [`ImportedData`].
///
/// There is only one way this function can return an error: not a single [`crate::Importer`]
/// (whether it is builtin, custom or external) was capable of loading the data, in which case
/// [`ImporterError::Incompatible`] will be returned.
#[cfg(target_arch = "wasm32")]
#[expect(clippy::needless_pass_by_value)]
pub(crate) fn import(
    settings: &crate::ImporterSettings,
    path: &std::path::Path,
    contents: Option<std::borrow::Cow<'_, [u8]>>,
) -> Result<crossbeam::channel::Receiver<ImportedData>, ImporterError> {
    re_tracing::profile_function!(path.display().to_string());

    let rx_importer = {
        let (tx_importer, rx_importer) = crossbeam::channel::unbounded();

        let any_compatible_importer = crate::iter_importers().any(|importer| {
            if let Some(contents) = contents.as_deref() {
                let settings = settings.clone();
                let tx_importer = tx_importer.clone();
                let path = path.to_owned();
                let contents = Cow::Borrowed(contents);

                if let Err(err) = importer.import_from_file_contents(&settings, path.clone(), contents, tx_importer) {
                    if err.is_incompatible() {
                        return false;
                    }
                    re_log::error!(?path, importer = importer.name(), %err, "Failed to import data from file");
                }

                true
            } else {
                false
            }
        });

        // Implicitly closing `tx_importer`!

        any_compatible_importer.then_some(rx_importer)
    };

    if let Some(rx_importer) = rx_importer {
        Ok(rx_importer)
    } else {
        Err(ImporterError::Incompatible(path.to_owned()))
    }
}

/// Forwards the data in `rx_importer` to `tx`, taking care of necessary conversions, if any.
///
/// Runs asynchronously from another thread on native, synchronously on wasm.
pub(crate) fn send(
    settings: crate::ImporterSettings,
    file_source: FileSource,
    rx_importer: crossbeam::channel::Receiver<ImportedData>,
    tx: &LogSender,
) {
    spawn({
        re_tracing::profile_function!();

        #[derive(Default, Debug)]
        struct Tracked {
            is_rrd_or_rbl: bool,
            already_has_store_info: bool,
        }

        let mut store_info_tracker: HashMap<re_log_types::StoreId, Tracked> = HashMap::new();

        let tx = tx.clone();
        move || {
            // ## Ignoring channel errors
            //
            // Not our problem whether or not the other end has hung up, but we still want to
            // poll the channel in any case so as to make sure that the data producer
            // doesn't get stuck.
            for data in rx_importer {
                let importer_name = data.importer_name().clone();
                let msg = match data.into_log_msg() {
                    Ok(msg) => {
                        let store_info = match &msg {
                            LogMsg::SetStoreInfo(set_store_info) => {
                                Some((set_store_info.info.store_id.clone(), true))
                            }
                            LogMsg::ArrowMsg(store_id, _arrow_msg) => {
                                Some((store_id.clone(), false))
                            }
                            LogMsg::BlueprintActivationCommand(_) => None,
                        };

                        if let Some((store_id, store_info_created)) = store_info {
                            let tracked = store_info_tracker.entry(store_id).or_default();
                            tracked.is_rrd_or_rbl =
                                *importer_name == RrdImporter::name(&RrdImporter);
                            tracked.already_has_store_info |= store_info_created;
                        }

                        msg
                    }
                    Err(err) => {
                        re_log::error!(%err, "Couldn't serialize component data");
                        continue;
                    }
                };
                tx.send(msg.into()).ok();
            }

            for (store_id, tracked) in store_info_tracker {
                let is_a_preexisting_recording =
                    Some(&store_id) == settings.opened_store_id.as_ref();

                // Never try to send custom store info for RRDs and RBLs, they always have their own, and
                // it's always right.
                let should_force_store_info = settings.force_store_info && !tracked.is_rrd_or_rbl;

                let should_send_new_store_info = should_force_store_info
                    || (!tracked.already_has_store_info && !is_a_preexisting_recording);

                if should_send_new_store_info {
                    let store_info = prepare_store_info(&store_id, file_source.clone());
                    tx.send(store_info.into()).ok();
                }
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
    if 1 < rayon::current_num_threads() {
        rayon::spawn(f);
    } else {
        // Avoids a deadlock when send-channel gets full.
        // We usually only use `-j1` for profiling the main application; not data loading.
        std::thread::Builder::new()
            .name("importer".to_owned())
            .spawn(f)
            .expect("Failed to spawn a thread");
    }
}

#[cfg(target_arch = "wasm32")]
fn spawn<F>(f: F)
where
    F: FnOnce(),
{
    f();
}
