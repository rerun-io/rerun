//! Defines the runtime configuration for the Rerun viewer, server & SDK.

// TODO:
// - per client
// - per store
// - sdk, server, viewer
//
// this should go through network
// viewer should ask SDK and server about their configs

// carrying configs around is a mess:
// - sdk can create a viewer... which can create servers??

// TODO: clap integration

// This actually all relates to https://github.com/rerun-io/rerun/issues/1542 in many ways

// TODO: should we centralize all this? should every pkg provide its own?

pub struct SdkArgs {
    // TODO:
    // - rerun (enable/disable toggler)
}

pub struct ViewerArgs {
    // TODO:
    // - mem limit
    // - shader path
    // - track allocations

    // pub memory_limit:
}

pub struct ServerArgs {
    // TODO:
    // - main server port
    // - wasm server port
    // - websocket server port
}

// wgpu_backend
// wgpu_power_pref
