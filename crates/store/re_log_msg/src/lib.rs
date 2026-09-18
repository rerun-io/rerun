//! The messages that carry Rerun data between the SDK, `.rrd` files and the viewer.
//!
//! A [`LogMsg`] is the envelope for a recording or blueprint: it announces a store with a
//! [`StoreInfo`], carries one chunk at a time as an [`ArrowMsg`], and activates a fully
//! transmitted blueprint with a [`BlueprintActivationCommand`].
//!
//! A [`TableMsg`] carries a standalone table. Tables are never stored in `.rrd` files, and travel
//! on a separate channel from [`LogMsg`].
//!
//! The building blocks these messages are made of — entity paths, timelines, store ids — live in
//! `re_log_types`.

mod arrow_msg;
mod log_msg;
mod store_info;
mod table_msg;

pub use self::arrow_msg::{ArrowMsg, ArrowRecordBatchReleaseCallback};
pub use self::log_msg::{BlueprintActivationCommand, LogMsg};
pub use self::store_info::{
    FileSource, PythonVersion, PythonVersionParseError, SetStoreInfo, StoreInfo, StoreSource,
};
pub use self::table_msg::TableMsg;
