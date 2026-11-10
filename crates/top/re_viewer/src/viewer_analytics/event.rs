use re_analytics::event::{
    Id, Identify, OpenRecording, StoreInfo, ViewerRuntimeInformation, ViewerStarted,
};
use re_analytics::{Config, Property};

use crate::AppEnvironment;

pub fn identify(
    config: &Config,
    build_info: re_build_info::BuildInfo,
    app_env: &AppEnvironment,
) -> Identify {
    let (rust_version, llvm_version, python_version) = match app_env {
        AppEnvironment::RustSdk {
            rustc_version,
            llvm_version,
        }
        | AppEnvironment::RerunCli {
            rustc_version,
            llvm_version,
        } => (Some(rustc_version), Some(llvm_version), None),
        AppEnvironment::PythonSdk(version) => (None, None, Some(version.to_string())),
        _ => (None, None, None),
    };

    Identify {
        build_info,
        rust_version: rust_version.map(|s| s.to_owned()),
        llvm_version: llvm_version.map(|s| s.to_owned()),
        python_version,
        opt_in_metadata: config.opt_in_metadata.clone(),
    }
}

pub fn viewer_started(
    app_env: &AppEnvironment,
    egui_ctx: &egui::Context,
    adapter_backend: wgpu::Backend,
    device_tier: re_renderer::device_caps::DeviceCapabilityTier,
) -> ViewerStarted {
    // Note that some of these things can change at runtime,
    // but we send them only once at startup.
    // This means if the user changes the zoom factor we won't send it
    // until the next restart.
    let screen_info = re_analytics::event::ScreenInfo {
        pixels_per_point: egui_ctx.pixels_per_point(),
        native_pixels_per_point: egui_ctx.native_pixels_per_point(),
        zoom_factor: egui_ctx.zoom_factor(),
    };

    ViewerStarted {
        url: app_env.url().cloned(),
        app_env: app_env.name(),
        runtime_info: ViewerRuntimeInformation {
            is_docker: crate::docker_detection::is_docker(),
            is_wsl: super::wsl::is_wsl(),
            graphics_adapter_backend: adapter_backend.to_string(),
            re_renderer_device_tier: device_tier.to_string(),
            screen_info,
        },
    }
}

pub fn open_recording(
    app_env: &AppEnvironment,
    entity_db: &re_entity_db::EntityDb,
) -> OpenRecording {
    let data_source = entity_db.primary_data_source();

    // Don't share recording ids from a Rerun server
    let is_redap = matches!(
        data_source,
        Some(re_log_channel::LogSource::RedapGrpcStream { .. })
    );
    let store_info = entity_db.store_info().map(|store_info| {
        let re_log_msg::StoreInfo {
            store_id,
            store_source,
            store_version,
        } = store_info;

        let application_id = store_id.application_id();
        let recording_id = store_id.recording_id();

        let app_id_starts_with_rerun_example = application_id.as_str().starts_with("rerun_example");

        let (application_id_preprocessed, recording_id_preprocessed) =
            if app_id_starts_with_rerun_example {
                (
                    Id::Official(application_id.to_string()),
                    Id::Official(recording_id.to_string()),
                )
            } else {
                (
                    Id::Hashed(Property::from(application_id.as_str()).hashed()),
                    Id::Hashed(Property::from(recording_id.as_str()).hashed()),
                )
            };

        use re_log_msg::StoreSource as S;
        let store_source_preprocessed = match &store_source {
            S::Unknown => "unknown".to_owned(),
            S::CSdk => "c_sdk".to_owned(),
            S::PythonSdk(_version) => "python_sdk".to_owned(),
            S::RustSdk { .. } => "rust_sdk".to_owned(),
            S::File { file_source } => match file_source {
                re_log_msg::FileSource::Cli => "file_cli".to_owned(),
                re_log_msg::FileSource::Uri => "file_uri".to_owned(),
                re_log_msg::FileSource::DragAndDrop { .. } => "file_drag_and_drop".to_owned(),
                re_log_msg::FileSource::FileDialog { .. } => "file_dialog".to_owned(),
                re_log_msg::FileSource::Sdk => "file_sdk".to_owned(),
            },
            S::Viewer => "viewer".to_owned(),
            S::Other(other) => other.clone(),
        };

        let store_version_preprocessed = if let Some(store_version) = store_version {
            store_version.to_string()
        } else {
            re_log::trace_once!("store version is undefined for this recording, this is a bug");
            "undefined".to_owned()
        };

        // `rust+llvm` and `python` versions are mutually exclusive
        let mut rust_version_preprocessed = None;
        let mut llvm_version_preprocessed = None;
        let mut python_version_preprocessed = None;
        match &store_source {
            S::File { .. } => {
                rust_version_preprocessed = Some(env!("RE_BUILD_RUSTC_VERSION").to_owned());
                llvm_version_preprocessed = Some(env!("RE_BUILD_LLVM_VERSION").to_owned());
            }
            S::RustSdk {
                rustc_version: rustc,
                llvm_version: llvm,
            } => {
                rust_version_preprocessed = Some(rustc.clone());
                llvm_version_preprocessed = Some(llvm.clone());
            }
            S::PythonSdk(version) => {
                python_version_preprocessed = Some(version.to_string());
            }
            // TODO(andreas): Send C SDK version and set it.
            S::CSdk | S::Unknown | S::Viewer | S::Other(_) => {}
        }

        StoreInfo {
            application_id: application_id_preprocessed,
            recording_id: recording_id_preprocessed,
            store_source: store_source_preprocessed,
            store_version: store_version_preprocessed,
            rust_version: rust_version_preprocessed,
            llvm_version: llvm_version_preprocessed,
            python_version: python_version_preprocessed,
            app_id_starts_with_rerun_example,
        }
    });

    let data_source = data_source.map(|data_source| match data_source {
        re_log_channel::LogSource::File { .. } => "file", // .rrd, .png, .glb, …
        re_log_channel::LogSource::HttpStream { .. } => "http",
        re_log_channel::LogSource::RedapGrpcStream { .. } => "redap",
        re_log_channel::LogSource::MessageProxy { .. } => "grpc",
        // vvv spawn(), connect() vvv
        re_log_channel::LogSource::RrdWebEvent => "web_event",
        re_log_channel::LogSource::JsChannel { .. } => "javascript", // mediated via rerun-js
        re_log_channel::LogSource::Sdk => "sdk",                     // show()
        re_log_channel::LogSource::Stdin => "stdin",
    });

    OpenRecording {
        url: app_env.url().cloned(),
        app_env: app_env.name(),
        store_info: store_info.filter(|_| !is_redap),
        data_source,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use re_log_channel::LogSource;
    use re_log_msg::{SetStoreInfo, StoreInfo, StoreSource};
    use re_log_types::{StoreId, StoreKind};

    use super::*;

    fn opened_from(source: Option<LogSource>) -> OpenRecording {
        let store_id = StoreId::random(StoreKind::Recording, "my_dataset");
        let mut entity_db = re_entity_db::EntityDb::new(store_id.clone());
        entity_db.set_store_info(SetStoreInfo {
            row_id: re_chunk::RowId::new().as_tuid(),
            info: StoreInfo::new(store_id, StoreSource::Unknown),
        });
        if let Some(source) = source {
            entity_db.add_data_source(&Arc::new(source));
        }
        open_recording(&AppEnvironment::Test, &entity_db)
    }

    #[test]
    fn server_recordings_are_reported_without_their_ids() {
        let segment = opened_from(Some(LogSource::RedapGrpcStream {
            uri: "rerun://127.0.0.1:1234/dataset/00000000000000000000000000000001?segment_id=pid"
                .parse()
                .unwrap(),
            open_behavior: re_log_channel::RecordingOpenBehavior::OpenAndSelect,
        }));
        assert_eq!(segment.data_source, Some("redap"));
        assert!(
            segment.store_info.is_none(),
            "the IDs name the dataset and segment"
        );

        let from_sdk = opened_from(Some(LogSource::Sdk));
        assert_eq!(from_sdk.data_source, Some("sdk"));
        assert!(from_sdk.store_info.is_some());
    }

    #[test]
    fn recordings_without_a_data_source_are_reported() {
        let unknown = opened_from(None);
        assert_eq!(unknown.data_source, None);
        assert!(unknown.store_info.is_some());
    }
}
