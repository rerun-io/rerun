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
        let reader = std::io::BufReader::new(file);
        let data_source = re_log_channel::LogSource::File {
            path: path.into(),
            follow: false,
        };

        Ok(StoreBundle::from_rrd(reader, &data_source)?)
    }

    match load_file_path_impl(path) {
        Ok(rrd) => Some(rrd),
        Err(err) => {
            let msg = format!("Failed loading {path:?}: {err}");

            // Silently ignore
            re_log::debug!("{msg}");

            None
        }
    }
}
