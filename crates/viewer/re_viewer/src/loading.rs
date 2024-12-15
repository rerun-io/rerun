use re_entity_db::{StoreBundle, StoreLoadError};

#[derive(thiserror::Error, Debug)]
enum BlueprintLoadError {
    #[error("Failed to open file: {0}")]
    FileOpen(#[from] std::io::Error),

    #[error(transparent)]
    StoreLoad(#[from] StoreLoadError),
}

/// Try to load the given `.blueprint` file.
///
/// The file must be of a matching version of rerun.
#[must_use]
pub fn load_blueprint_file(path: &std::path::Path) -> Option<StoreBundle> {
    fn load_file_path_impl(path: &std::path::Path) -> Result<StoreBundle, BlueprintLoadError> {
        re_tracing::profile_function!();

        let file = std::fs::File::open(path)?;
        let file = std::io::BufReader::new(file);

        // Blueprint files change often. Be strict about the version, and then ignore any errors.
        // See https://github.com/rerun-io/rerun/issues/2830
        let version_policy = re_log_encoding::VersionPolicy::Error;

        Ok(StoreBundle::from_rrd(version_policy, file)?)
    }

    match load_file_path_impl(path) {
        Ok(mut rrd) => {
            for entity_db in rrd.entity_dbs_mut() {
                entity_db.data_source =
                    Some(re_smart_channel::SmartChannelSource::File(path.into()));
            }
            Some(rrd)
        }
        Err(err) => {
            let msg = format!("Failed loading {path:?}: {err}");

            // Silently ignore
            re_log::debug!("{msg}");

            None
        }
    }
}
