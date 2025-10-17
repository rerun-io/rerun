//! * Encode/decode and serialize/deserialize RRD streams.
//! * Convert between transport-level (Protobuf) and application-level types (`re_log_types`).
//!
//! If you are working with one of the gRPC APIs (Redap or SDK comms), then you want to be looking
//! into the `transport_to_app` module. The [`rrd`] module is completely irrelevant in that case.
//!
//! If you are working with actual RRD streams (i.e. everything that does not go through gRPC:
//! files, standard I/O, HTTP, data loaders, etc), then look into the [`rrd`] module.

// TODO: this needs to document the different protocols that we have: SDK comms, Redap, RRD
// TODO: RRD: files, stdio, HTTP, data loaders, etc
// TODO: RRD is the expectation, the rest is the exception
// TODO: the job of encoders/decoders is to provide the IO, state machines
// TODO: clearly one has to explain transport vs app level types to begin with, this is so fundamental

// TODO:
// * RRD streams: files, stdio (data-loaders), HTTP, etc
// * SDK comms (legacy storehub): log and fetch data ish from web viewer (message proxy)
// * Redap: everything else

// TODO:
// * explain the different logmsgs, who uses what and why.

pub mod rrd;

mod app_id_injector;
mod transport_to_app;

pub mod external {
    pub use lz4_flex;
}

pub use self::rrd::*;

pub use self::app_id_injector::{
    ApplicationIdInjector, CachingApplicationIdInjector, DummyApplicationIdInjector,
};
pub use self::transport_to_app::{ToApplication, ToTransport};
