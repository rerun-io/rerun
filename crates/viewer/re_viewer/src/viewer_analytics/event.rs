use crate::AppEnvironment;

use re_analytics::{
    event::{Id, Identify, OpenRecording, StoreInfo, ViewerRuntimeInformation, ViewerStarted},
    Config, Property,
};

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
    adapter_backend: wgpu::Backend,
    device_tier: re_renderer::device_caps::DeviceCapabilityTier,
) -> ViewerStarted {
    ViewerStarted {
        url: app_env.url().cloned(),
        app_env: app_env.name(),
        runtime_info: ViewerRuntimeInformation {
            is_wsl: super::wsl::is_wsl(),
            graphics_adapter_backend: adapter_backend.to_string(),
            re_renderer_device_tier: device_tier.to_string(),
        },
    }
}

pub fn open_recording(
    app_env: &AppEnvironment,
    entity_db: &re_entity_db::EntityDb,
) -> Option<OpenRecording> {
    let store_info = entity_db.store_info().map(|store_info| {
        let re_log_types::StoreInfo {
            application_id,
            store_id,
            store_source,
            store_version,
            cloned_from: _,
        } = store_info;

        let app_id_starts_with_rerun_example = application_id.as_str().starts_with("rerun_example");

        let (application_id_preprocessed, recording_id_preprocessed) =
            if app_id_starts_with_rerun_example {
                (
                    Id::Official(application_id.0.clone()),
                    Id::Official(store_id.to_string()),
                )
            } else {
                (
                    Id::Hashed(Property::from(application_id.0.clone()).hashed()),
                    Id::Hashed(Property::from(store_id.to_string()).hashed()),
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
            re_log::debug_once!("store version is undefined for this recording, this is a bug");
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
                rust_version_preprocessed = Some(rustc.to_string());
                llvm_version_preprocessed = Some(llvm.to_string());
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
        re_smart_channel::SmartChannelSource::File(_) => Some("file"), // .rrd, .png, .glb, …
        re_smart_channel::SmartChannelSource::RrdHttpStream { .. } => Some("http"),
        // TODO(grtlr): Differentiate between `recording` and `catalog`
        // re_smart_channel::SmartChannelSource::RedapGrpcStream { .. } => "recording | catalog",
        re_smart_channel::SmartChannelSource::RedapGrpcStreamLegacy { .. }
        | re_smart_channel::SmartChannelSource::RedapGrpcStream { .. } => None,
        re_smart_channel::SmartChannelSource::MessageProxy { .. } => Some("grpc"),
        // vvv spawn(), connect() vvv
        re_smart_channel::SmartChannelSource::RrdWebEventListener { .. } => Some("web_event"),
        re_smart_channel::SmartChannelSource::JsChannel { .. } => Some("javascript"), // mediated via rerun-js
        re_smart_channel::SmartChannelSource::Sdk => Some("sdk"),                     // show()
        re_smart_channel::SmartChannelSource::Stdin => Some("stdin"),
    })?;

    Some(OpenRecording {
        url: app_env.url().cloned(),
        app_env: app_env.name(),
        store_info,
        data_source,
    })
}
