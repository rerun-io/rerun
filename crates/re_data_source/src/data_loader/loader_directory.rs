/// Recursively oads entire directories, using the appropriate [`crate::DataLoader`]:s for each
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
        store_id: re_log_types::StoreId,
        dirpath: std::path::PathBuf,
        tx: std::sync::mpsc::Sender<crate::LoadedData>,
    ) -> Result<(), crate::DataLoaderError> {
        if dirpath.is_file() {
            return Ok(()); // simply not interested
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
                let store_id = store_id.clone();
                let filepath = filepath.to_owned();
                let tx = tx.clone();

                // NOTE: spawn is fine, this whole function is native-only.
                rayon::spawn(move || {
                    let data = match crate::load_file::load(&store_id, &filepath, false, None) {
                        Ok(data) => data,
                        Err(err) => {
                            re_log::error!(?filepath, %err, "Failed to load directory entry");
                            return;
                        }
                    };

                    for datum in data {
                        if tx.send(datum).is_err() {
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
        _store_id: re_log_types::StoreId,
        _path: std::path::PathBuf,
        _contents: std::borrow::Cow<'_, [u8]>,
        _tx: std::sync::mpsc::Sender<crate::LoadedData>,
    ) -> Result<(), crate::DataLoaderError> {
        // TODO(cmc): This could make sense to implement for e.g. archive formats (zip, tar, …)
        Ok(()) // simply not interested
    }
}
