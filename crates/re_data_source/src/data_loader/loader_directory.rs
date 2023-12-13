// TODO: later -> HIVE partitiong, timestamp regexes, zip files, that kinda thing
pub struct DirectoryLoader;

impl crate::DataLoader for DirectoryLoader {
    #[inline]
    fn name(&self) -> String {
        "rerun.data_loaders.Directory".into()
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn load_from_file(
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
                    let data = match super::load(&store_id, &filepath, false, None) {
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
        // TODO: zip file supports
        Ok(()) // simply not interested
    }
}
