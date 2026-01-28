//! Lenses allow you to extract, transform, and restructure component data. They
//! are applied to chunks that match the specified entity path filter and contain
//! the target component.
//!
//! See [`crate::lenses::Lens`] for more details and assumptions. One way to make use of lenses is
//! by using the [`crate::lenses::LensesSink`].

mod sink;

// Re-exports from re_lenses.
// We should be careful not to expose too much implementation details here.
pub use re_lenses::{
    ColumnsBuilder, Lens, LensBuilder, LensError, Lenses, Op, OpError, OutputMode, PartialChunk,
    ScatterColumnsBuilder, StaticColumnsBuilder,
};

pub use re_arrow_combinators::Selector;

// We keep the sink in re_sdk since it depends on LogSink.
pub use self::sink::LensesSink;
