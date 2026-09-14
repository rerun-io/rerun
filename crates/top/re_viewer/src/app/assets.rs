//! Registering local `.rrd` files as assets of a dataset in the internal catalog.

use std::path::{Path, PathBuf};

use anyhow::Context as _;
use re_log_types::EntryId;
use re_redap_client::ConnectionRegistryHandle;
use re_viewer_context::{CommandSender, SystemCommand, SystemCommandSender as _};

use super::add_data_source::RegistrationTarget;

/// Registers `.rrd` files as assets of the dataset a file was registered with.
pub async fn register_assets(
    connection_registry: &ConnectionRegistryHandle,
    command_sender: &CommandSender,
    target: &RegistrationTarget,
    paths: &[PathBuf],
) {
    if paths.is_empty() {
        return;
    }

    let Some(connection) = connection_registry.internal_connection_handle() else {
        re_log::error!("Failed to register assets: the internal catalog is not running");
        return;
    };

    for path in paths {
        let source_uri = match asset_file_url(path) {
            Ok(url) => url,
            Err(err) => {
                re_log::error!(
                    "Failed to register asset: {}\nFile path: {}",
                    re_error::format(err),
                    path.display()
                );
                continue;
            }
        };

        match connection
            .register_asset(
                dataset_id(target),
                source_uri.as_str(),
                re_redap_client::DEFAULT_ASSET_TASK_TIMEOUT,
            )
            .await
        {
            Ok(asset_id) => re_log::info!(
                "Registered asset as '{asset_id}'\nFile path: {}",
                path.display()
            ),
            Err(err) => re_log::error!(
                "Failed to register asset: {err}\nFile path: {}",
                path.display()
            ),
        }
    }

    // The asset dataset comes from the entry list, which the server only reports once the first
    // asset is registered.
    if let Some(origin) = connection_registry.internal_origin() {
        command_sender.send_system(SystemCommand::RefreshRedapServer(origin.clone()));
    }
}

/// The dataset the file was registered with.
fn dataset_id(target: &RegistrationTarget) -> EntryId {
    match target {
        RegistrationTarget::DatasetSegment(uri) => uri.dataset_id.into(),
        RegistrationTarget::Entry(entry_id) => *entry_id,
    }
}

/// The `file://` URL the server reads the asset from.
fn asset_file_url(path: &Path) -> anyhow::Result<url::Url> {
    let abs_path = std::path::absolute(path).with_context(|| {
        format!(
            "failed to resolve absolute path\nFile path: {}",
            path.display()
        )
    })?;

    url::Url::from_file_path(&abs_path).map_err(|()| {
        anyhow::anyhow!(
            "not an absolute file path\nFile path: {}",
            abs_path.display()
        )
    })
}
