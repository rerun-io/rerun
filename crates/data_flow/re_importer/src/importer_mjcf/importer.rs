use std::path::{Path, PathBuf};

use anyhow::Context as _;
use crossbeam::channel::Sender;
use re_chunk::TimePoint;

use super::MjcfTree;
use super::model::{self, model_name_from_path};
use crate::{ImportedData, Importer, ImporterError};

#[derive(Clone, Copy)]
enum MjcfExtension {
    Mjcf,
    Xml,
}

/// An [`Importer`] for [MJCF](https://mujoco.readthedocs.io/en/stable/XMLreference.html), the XML
/// model format of the [MuJoCo](https://mujoco.org/) physics engine.
///
/// Handles `.mjcf` files, and `.xml` files whose root element is `<mujoco>`.
///
/// The model is logged in its reference configuration: every body becomes a frame, named after the
/// body, with static transforms from its parent body on `/tf_static`.
/// A body with several joints also has a frame between each pair of them, see [`MjcfTree::joint_frames`].
/// Joints can be animated by logging transforms between these frames, computed with [`MjcfTree`].
pub struct MjcfImporter;

impl Importer for MjcfImporter {
    fn name(&self) -> crate::ImporterName {
        "rerun.importers.Mjcf".to_owned()
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn import_from_path(
        &self,
        settings: &crate::ImporterSettings,
        filepath: PathBuf,
        tx: Sender<ImportedData>,
    ) -> Result<(), ImporterError> {
        if mjcf_extension(&filepath).is_none() {
            return Err(ImporterError::Incompatible(filepath));
        }

        let contents = std::fs::read(&filepath)?;
        self.import_from_file_contents(settings, filepath, contents.into(), tx)
    }

    fn import_from_file_contents(
        &self,
        settings: &crate::ImporterSettings,
        filepath: PathBuf,
        contents: std::borrow::Cow<'_, [u8]>,
        tx: Sender<ImportedData>,
    ) -> Result<(), ImporterError> {
        let Some(extension) = mjcf_extension(&filepath) else {
            return Err(ImporterError::Incompatible(filepath));
        };

        // `.xml` is far too generic to claim without looking inside.
        let text = match (std::str::from_utf8(&contents), extension) {
            (Ok(text), MjcfExtension::Mjcf) => text,
            (Ok(text), MjcfExtension::Xml) if model::has_mujoco_root(text) => text,
            (Err(err), MjcfExtension::Mjcf) => {
                return Err(anyhow::anyhow!("MJCF file is not valid UTF-8: {err}").into());
            }
            (Ok(_) | Err(_), MjcfExtension::Xml) => {
                return Err(ImporterError::Incompatible(filepath));
            }
        };

        re_tracing::profile_function!(filepath.display().to_string());

        let model_dir = filepath.parent().unwrap_or_else(|| Path::new(""));
        let tree = MjcfTree::from_xml(
            text,
            model_dir,
            &model_name_from_path(&filepath),
            settings.entity_path_prefix.clone(),
        )
        .context("Failed to parse MJCF file")?;

        let store_id = settings.opened_store_id_or_recommended();
        let mut send_error = None;
        let mut emit = |chunk| {
            if send_error.is_none() {
                send_error = re_quota_channel::send_crossbeam(
                    &tx,
                    ImportedData::Chunk {
                        importer_name: Self.name(),
                        store_id: store_id.clone(),
                        chunk,
                    },
                )
                .err();
            }
        };

        tree.emit(
            &mut emit,
            &settings.timepoint.clone().unwrap_or(TimePoint::STATIC),
            true,
        )
        .context("Failed to load MJCF file")?;

        if let Some(err) = send_error {
            return Err(anyhow::anyhow!(err.to_string()).into());
        }

        Ok(())
    }
}

fn mjcf_extension(path: &Path) -> Option<MjcfExtension> {
    let extension = path.extension()?;
    if extension.eq_ignore_ascii_case("mjcf") {
        Some(MjcfExtension::Mjcf)
    } else if extension.eq_ignore_ascii_case("xml") {
        Some(MjcfExtension::Xml)
    } else {
        None
    }
}
