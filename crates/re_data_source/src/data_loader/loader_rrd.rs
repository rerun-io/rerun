use re_log_encoding::decoder::Decoder;

// ---

/// Loads data from any `rrd` file or in-memory contents.
pub struct RrdLoader;

impl crate::DataLoader for RrdLoader {
    #[inline]
    fn name(&self) -> String {
        "rerun.data_loaders.Rrd".into()
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn load_from_file(
        &self,
        // NOTE: The Store ID comes from the rrd file itself.
        _store_id: re_log_types::StoreId,
        filepath: std::path::PathBuf,
        tx: std::sync::mpsc::Sender<crate::LoadedData>,
    ) -> Result<(), crate::DataLoaderError> {
        use anyhow::Context as _;

        re_tracing::profile_function!(filepath.display().to_string());

        let extension = filepath
            .extension()
            .unwrap_or_default()
            .to_ascii_lowercase()
            .to_string_lossy()
            .to_string();

        if extension != "rrd" {
            return Ok(()); // simply not interested
        }

        re_log::debug!(
            ?filepath,
            loader = self.name(),
            "Loading rrd data from filesystem…",
        );

        let version_policy = re_log_encoding::decoder::VersionPolicy::Warn;
        let file = std::fs::File::open(&filepath)
            .with_context(|| format!("Failed to open file {filepath:?}"))?;
        let file = std::io::BufReader::new(file);

        let decoder = re_log_encoding::decoder::Decoder::new(version_policy, file)?;
        decode_and_stream(&filepath, &tx, decoder);

        Ok(())
    }

    fn load_from_file_contents(
        &self,
        // NOTE: The Store ID comes from the rrd file itself.
        _store_id: re_log_types::StoreId,
        filepath: std::path::PathBuf,
        contents: std::borrow::Cow<'_, [u8]>,
        tx: std::sync::mpsc::Sender<crate::LoadedData>,
    ) -> Result<(), crate::DataLoaderError> {
        re_tracing::profile_function!(filepath.display().to_string());

        let version_policy = re_log_encoding::decoder::VersionPolicy::Warn;
        let contents = std::io::Cursor::new(contents);
        let decoder = match re_log_encoding::decoder::Decoder::new(version_policy, contents) {
            Ok(decoder) => decoder,
            Err(err) => match err {
                // simply not interested
                re_log_encoding::decoder::DecodeError::NotAnRrd
                | re_log_encoding::decoder::DecodeError::Options(_) => return Ok(()),
                _ => return Err(err.into()),
            },
        };
        decode_and_stream(&filepath, &tx, decoder);
        Ok(())
    }
}

fn decode_and_stream<R: std::io::Read>(
    filepath: &std::path::Path,
    tx: &std::sync::mpsc::Sender<crate::LoadedData>,
    decoder: Decoder<R>,
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
