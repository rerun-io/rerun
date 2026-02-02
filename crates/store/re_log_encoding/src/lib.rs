//! This crate covers two equally important but orthogonal matters:
//! * Converting between transport-level and application-level Rerun types.
//! * Encoding and decoding Rerun RRD streams.
//!
//! If you are working with one of the gRPC APIs (Redap or SDK comms), then you want to be looking
//! at the [`ToTransport`]/[`ToApplication`] traits. The [`rrd`] module is completely irrelevant in
//! that case. You can learn more about these traits below.
//!
//! If you are working with actual RRD streams (i.e. everything that does not go through gRPC:
//! files, standard I/O, HTTP, data loaders, etc), then have a look into the [`rrd`] module.
//! The [`ToTransport`]/[`ToApplication`] traits will also be useful to you. You can learn more
//! about these traits below.
//!
//! ## What are transport-level and application-level types?
//!
//! To put it in simple terms: transport-level types are the types that you find in `re_protos`, while
//! application-level types are those that you find in `re_log_types`.
//!
//! More generally, transport-level types are Rust objects that represent the decoded value of some
//! Rerun bytes, *and nothing more than that*. It's all they do: they map raw bytes to their native
//! Rust representation and vice-versa. They never apply any application-level logic beyond that.
//!
//! Transport-level types are used to unopinionatedly transport Rerun data across the many mediums
//! that Rerun support, while application-level types are used to build applications for end-users,
//! such as the Rerun viewer itself.
//!
//! Application-level types on the other hand are *very* opinionated, and often perform a lot of
//! transformations on the data, including but not limited to:
//! * Chunk/Sorbet migrations
//! * Application ID injection
//! * SDK version patching
//! * Backward-compatibility shenanigans
//! * Etc
//!
//! Application-level can _not_ be encoded/decoded, only transport-level types can. To encode an
//! application-level type, you must first convert it to transport-level type.
//! You can do so by using the [`ToApplication`] & [`ToTransport`] traits exposed by this crate.
//!
//! ## How do I make sense of all these different `LogMsg` types?!
//!
//! There are 3 different `LogMsg`-related types that you will very often encounter: `re_log_types::LogMsg`,
//! `re_protos::log_msg::v1alpha1::LogMsg` and `re_protos::log_msg::v1alpha1::log_msg::Msg`.
//!
//! Mixing them up is a common source of pain and confusion, so let's go over what each does:
//! * `re_log_types::LogMsg` is the application-level type that we use all across the viewer
//!   codebase. It can be obtained by calling `to_application()` on one of the transport-level
//!   `LogMsg` types which, among many other things, will perform Chunk/Sorbet-level migrations.
//!   `re_log_types::LogMsg` isn't used in Redap, where everything is done at the transport-level, always.
//! * `re_protos::log_msg::v1alpha1::LogMsg` is the transport-level definition of `LogMsg`. It is an
//!   artifact of how `oneof` works in Protobuf: all it does is carry a `re_protos::log_msg::v1alpha1::log_msg::Msg`.
//!   For that reason, it is never directly used, except by the legacy SDK comms protocol.
//! * Finally, `re_protos::log_msg::v1alpha1::log_msg::Msg` is the real transport-level type that we
//!   care about. It is used all over the place when encoding and decoding RRD streams.
//!
//! ## What are the different protocols supported by Rerun?
//!
//! Rerun currently supports 3 protocols:
//! * Redap (Rerun Data Protocol): our gRPC-based protocol used by our OSS and proprietary data platforms.
//! * SDK comms: our legacy gRPC-based protocol, currently used by everything relying on the old
//!   `StoreHub` model (logging, message proxy, etc).
//! * RRD streams: the binary protocol that we use for all stream-based interfaces (files, stdio,
//!   data-loaders, HTTP fetches, etc).
//!
//! *All these protocols use the exact same encoding*. There is only one encoding: the Rerun encoding.
//! It often happens that one protocol makes use of some types while others don't (e.g. the
//! top-level `LogMsg` object is never used in RRD streams, but is used in SDK comms), but for all
//! the types they do share, the encoding will be the exact same.

pub mod rrd;

mod app_id_injector;
mod transport_to_app;

pub mod external {
    pub use lz4_flex;
}

pub use self::app_id_injector::{
    ApplicationIdInjector, CachingApplicationIdInjector, DummyApplicationIdInjector,
};
pub use self::rrd::*;
pub use self::transport_to_app::{ToApplication, ToTransport};
