/// Recursively loads entire directories, using the appropriate [`crate::DataLoader`]:s for each
/// files within.
//
// TODO(cmc): There are a lot more things than can be done be done when it comes to the semantics
// of a folder, e.g.: HIVE-like partitioning, similarly named files with different indices and/or
// timestamps (e.g. a folder of video frames), etc.
// We could support some of those at some point, or at least add examples to show users how.
pub struct DirectoryLoader;

impl crate::DataLoader for DirectoryLoader {
    #[inline]
    fn name(&self) -> String {
        "rerun.data_loaders.Directory".into()
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn load_from_path(
        &self,
        settings: &crate::DataLoaderSettings,
        dirpath: std::path::PathBuf,
        tx: crossbeam::channel::Sender<crate::LoadedData>,
    ) -> Result<(), crate::DataLoaderError> {
        // NOTE: Checking whether this is a file is _not_ enough. It could also be a fifo, a
        // socket, a named pipe, a symlink to any of these things, etc.
        // So make sure to check whether it's a directory, and nothing else.
        if !dirpath.is_dir() {
            return Err(crate::DataLoaderError::Incompatible(dirpath.clone()));
        }

        if crate::lerobot::is_lerobot_dataset(&dirpath) {
            // LeRobot dataset is loaded by LeRobotDatasetLoader
            return Err(crate::DataLoaderError::Incompatible(dirpath.clone()));
        }

        re_tracing::profile_function!(dirpath.display().to_string());

        re_log::debug!(?dirpath, loader = self.name(), "Loading directory…",);

        for entry in walkdir::WalkDir::new(&dirpath) {
            let entry = match entry {
                Ok(entry) => entry,
                Err(err) => {
                    re_log::error!(loader = self.name(), ?dirpath, %err, "Failed to open filesystem entry");
                    continue;
                }
            };

            let filepath = entry.path();
            if filepath.is_file() {
                let settings = settings.clone();
                let filepath = filepath.to_owned();
                let tx = tx.clone();

                // NOTE(1): `spawn` is fine, this whole function is native-only.
                // NOTE(2): this must spawned on a dedicated thread to avoid a deadlock!
                // `load` will spawn a bunch of loaders on the common rayon thread pool and wait for
                // their response via channels: we cannot be waiting for these responses on the
                // common rayon thread pool.
                _ = std::thread::Builder::new()
                    .name(format!("load_dir_entry({filepath:?})"))
                    .spawn(move || {
                        let data = match crate::load_file::load(&settings, &filepath, None) {
                            Ok(data) => data,
                            Err(err) => {
                                re_log::error!(?filepath, %err, "Failed to load directory entry");
                                return;
                            }
                        };

                        for datum in data {
                            if re_quota_channel::send_crossbeam(&tx, datum).is_err() {
                                break;
                            }
                        }
                    });
            }
        }

        Ok(())
    }

    #[inline]
    fn load_from_file_contents(
        &self,
        _settings: &crate::DataLoaderSettings,
        path: std::path::PathBuf,
        _contents: std::borrow::Cow<'_, [u8]>,
        _tx: crossbeam::channel::Sender<crate::LoadedData>,
    ) -> Result<(), crate::DataLoaderError> {
        // TODO(cmc): This could make sense to implement for e.g. archive formats (zip, tar, …)
        Err(crate::DataLoaderError::Incompatible(path))
    }
}
