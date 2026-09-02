#![allow(clippy::iter_over_hash_type)]

//! A Rerun server implementation backed by an in-memory store.

#[cfg(not(target_arch = "wasm32"))]
mod entrypoint;
#[cfg(not(target_arch = "wasm32"))]
mod layers;
mod named_path;
mod rerun_cloud;
#[cfg(not(target_arch = "wasm32"))]
mod server;
mod store;

pub use self::named_path::{NamedPath, NamedPathCollection};
pub use self::rerun_cloud::{
    RerunCloudHandler, RerunCloudHandlerBuilder, RerunCloudHandlerSettings,
};
#[cfg(not(target_arch = "wasm32"))]
pub use self::{
    entrypoint::Args,
    layers::InjectedErrors,
    server::{Server, ServerBuilder, ServerError, ServerHandle},
};

/// The capability names this build supports.
///
/// On wasm the server reads `file://` sources from OPFS, and only the viewer itself puts files
/// there, so that build does not support registering them.
pub(crate) fn capability_names() -> Vec<String> {
    cfg_select! {
        target_arch = "wasm32" => { Vec::new() }
        _ => { vec![re_protos::capabilities::catalog_write_register("file")] }
    }
}

/// What this build of the server implements and supports, for any caller.
///
/// Same as what `/WhoAmI` advertises, for a client that talks to the server in-process and never
/// makes that call.
pub fn capabilities() -> re_protos::capabilities::ServerCapabilities {
    re_protos::capabilities::ServerCapabilities::from_advertised(capability_names())
}

/// What should we do on error?
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum OnError {
    Continue,
    Abort,
}

#[cfg(test)]
mod tests {
    /// The native build reads `file://` sources from the filesystem it can see, so it advertises
    /// registering that scheme.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn the_native_build_advertises_registering_files() {
        assert_eq!(super::capabilities().register_schemes(), vec!["file"]);
    }
}
