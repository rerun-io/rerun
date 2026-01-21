//! Lenses allow you to extract, transform, and restructure component data. They
//! are applied to chunks that match the specified entity path filter and contain
//! the target component.
//!
//! See [`Lens`] for more details and assumptions. One way to make use of lenses is
//! by using the [`LensesSink`].

mod sink;

// Re-export everything from re_lenses
pub use re_lenses::{
    ColumnsBuilder, Lens, LensBuilder, LensError, Lenses, Op, OpError, OutputMode, PartialChunk,
    ScatterColumnsBuilder, StaticColumnsBuilder,
};

// Keep the sink in re_sdk since it depends on LogSink
pub use self::sink::LensesSink;
