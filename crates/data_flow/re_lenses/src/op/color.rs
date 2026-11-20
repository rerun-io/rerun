//! Color transforms for arrays.

use std::sync::Arc;

use arrow::array::{Array as _, ArrayRef, UInt32Array};
use arrow::row::{RowConverter, SortField};
use re_lenses_core::combinators::Error;
use re_log_types::hash::Hash64;
use re_sdk_types::components::Color;

/// Maps each value to a default color, as used by color fallbacks.
///
/// Accepts any datatype supported by Arrow's row format.
/// Equal values of the same datatype get the same color, but different values can share a color.
/// Nulls stay null.
/// The output is `UInt32` RGBA, which matches [`Color`].
pub fn color_for() -> impl Fn(&ArrayRef) -> Result<Option<ArrayRef>, Error> + Send + Sync {
    |source: &ArrayRef| {
        let converter = RowConverter::new(vec![SortField::new(source.data_type().clone())])?;
        let rows = converter.convert_columns(std::slice::from_ref(source))?;

        let nulls = source.logical_nulls();
        let colors: UInt32Array = (0..source.len())
            .map(|i| {
                nulls.as_ref().is_none_or(|n| n.is_valid(i)).then(|| {
                    let hash = Hash64::hash(rows.row(i).as_ref()).hash64();
                    Color::auto_from_hash(hash).to_u32()
                })
            })
            .collect();

        Ok(Some(Arc::new(colors) as ArrayRef))
    }
}
