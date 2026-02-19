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
) -> Option<OpenRecording> {
    let store_info = entity_db.store_info().map(|store_info| {
        let re_log_types::StoreInfo {
            store_id,
            store_source,
            store_version,
            cloned_from: _,
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

        use re_log_types::StoreSource as S;
        let store_source_preprocessed = match &store_source {
            S::Unknown => "unknown".to_owned(),
            S::CSdk => "c_sdk".to_owned(),
            S::PythonSdk(_version) => "python_sdk".to_owned(),
            S::RustSdk { .. } => "rust_sdk".to_owned(),
            S::File { file_source } => match file_source {
                re_log_types::FileSource::Cli => "file_cli".to_owned(),
                re_log_types::FileSource::Uri => "file_uri".to_owned(),
                re_log_types::FileSource::DragAndDrop { .. } => "file_drag_and_drop".to_owned(),
                re_log_types::FileSource::FileDialog { .. } => "file_dialog".to_owned(),
                re_log_types::FileSource::Sdk => "file_sdk".to_owned(),
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

    let data_source = entity_db.data_source.as_ref().map(|v| match v {
        re_log_channel::LogSource::File(_) => Some("file"), // .rrd, .png, .glb, …
        re_log_channel::LogSource::HttpStream { .. } => Some("http"),
        re_log_channel::LogSource::RedapGrpcStream { .. } => None,
        re_log_channel::LogSource::MessageProxy { .. } => Some("grpc"),
        // vvv spawn(), connect() vvv
        re_log_channel::LogSource::RrdWebEvent => Some("web_event"),
        re_log_channel::LogSource::JsChannel { .. } => Some("javascript"), // mediated via rerun-js
        re_log_channel::LogSource::Sdk => Some("sdk"),                     // show()
        re_log_channel::LogSource::Stdin => Some("stdin"),
    })?;

    Some(OpenRecording {
        url: app_env.url().cloned(),
        app_env: app_env.name(),
        store_info,
        data_source,
    })
}
