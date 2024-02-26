use crate::AppEnvironment;
use re_analytics::Config;
use re_analytics::Property;

use re_analytics::event::{Id, Identify, OpenRecording, StoreInfo, ViewerStarted};

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

pub fn viewer_started(app_env: &AppEnvironment) -> ViewerStarted {
    ViewerStarted {
        url: app_env.url().cloned(),
        app_env: app_env.name(),
    }
}

pub fn open_recording(
    app_env: &AppEnvironment,
    entity_db: &re_entity_db::EntityDb,
) -> OpenRecording {
    let store_info = entity_db.store_info().map(|store_info| {
        let application_id = if store_info.is_official_example {
            Id::Official(store_info.application_id.0.clone())
        } else {
            Id::Hashed(Property::from(store_info.application_id.0.clone()).hashed())
        };

        let recording_id = if store_info.is_official_example {
            Id::Official(store_info.store_id.to_string())
        } else {
            Id::Hashed(Property::from(store_info.store_id.to_string()).hashed())
        };

        use re_log_types::StoreSource as S;
        let store_source = match &store_info.store_source {
            S::Unknown => "unknown".to_owned(),
            S::CSdk => "c_sdk".to_owned(),
            S::PythonSdk(_version) => "python_sdk".to_owned(),
            S::RustSdk { .. } => "rust_sdk".to_owned(),
            S::File { file_source } => match file_source {
                re_log_types::FileSource::Cli => "file_cli".to_owned(),
                re_log_types::FileSource::DragAndDrop => "file_drag_and_drop".to_owned(),
                re_log_types::FileSource::FileDialog => "file_dialog".to_owned(),
            },
            S::Viewer => "viewer".to_owned(),
            S::Other(other) => other.clone(),
        };

        // `rust+llvm` and `python` versions are mutually exclusive
        let mut rust_version = None;
        let mut llvm_version = None;
        let mut python_version = None;
        match &store_info.store_source {
            S::File { .. } => {
                rust_version = Some(env!("RE_BUILD_RUSTC_VERSION").to_owned());
                llvm_version = Some(env!("RE_BUILD_LLVM_VERSION").to_owned());
            }
            S::RustSdk {
                rustc_version: rustc,
                llvm_version: llvm,
            } => {
                rust_version = Some(rustc.to_string());
                llvm_version = Some(llvm.to_string());
            }
            S::PythonSdk(version) => {
                python_version = Some(version.to_string());
            }
            // TODO(andreas): Send C SDK version and set it.
            S::CSdk | S::Unknown | S::Viewer | S::Other(_) => {}
        }

        let is_official_example = store_info.is_official_example;
        let app_id_starts_with_rerun_example = store_info
            .application_id
            .as_str()
            .starts_with("rerun_example");

        StoreInfo {
            application_id,
            recording_id,
            store_source,
            rust_version,
            llvm_version,
            python_version,
            is_official_example,
            app_id_starts_with_rerun_example,
        }
    });

    let data_source = entity_db.data_source.as_ref().map(|v| match v {
        re_smart_channel::SmartChannelSource::File(_) => "file", // .rrd, .png, .glb, …
        re_smart_channel::SmartChannelSource::RrdHttpStream { .. } => "http",
        re_smart_channel::SmartChannelSource::RrdWebEventListener { .. } => "web_event",
        re_smart_channel::SmartChannelSource::Sdk => "sdk", // show()
        re_smart_channel::SmartChannelSource::WsClient { .. } => "ws_client", // spawn()
        re_smart_channel::SmartChannelSource::TcpServer { .. } => "tcp_server", // connect()
        re_smart_channel::SmartChannelSource::Stdin => "stdin",
    });

    OpenRecording {
        url: app_env.url().cloned(),
        app_env: app_env.name(),
        store_info,
        data_source,
    }
}
