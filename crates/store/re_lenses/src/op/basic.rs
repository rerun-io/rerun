//! Basic transforms for common operations.

use arrow::array::ArrayRef;
use arrow::datatypes::DataType;

use re_lenses_core::combinators::Error;

/// Creates a transform that casts the input array to a new [`DataType`].
pub fn cast(
    to_type: DataType,
) -> impl Fn(&ArrayRef) -> Result<Option<ArrayRef>, Error> + Send + Sync {
    move |source: &ArrayRef| {
        let cast_values = arrow::compute::cast(source, &to_type)?;
        Ok(Some(cast_values))
    }
}
