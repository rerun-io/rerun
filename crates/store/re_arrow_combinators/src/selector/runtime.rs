//! Runtime execution of [`Expr`] against Arrow [`ListArray`]s.
//!
//! This module implements the [`Transform`] trait for expressions,

use arrow::array::{Array as _, ListArray};

use crate::{
    Transform,
    index::GetIndexList,
    map::MapList,
    reshape::{Flatten, GetField},
};

use super::parser::{Expr, Segment};

pub fn execute_per_row(expr: &Expr, source: &ListArray) -> Result<ListArray, crate::Error> {
    // TODO(grtlr): It would be much cleaner if `MapList` (or equivalent would be called on this level).
    let result = expr.transform(source)?;

    re_log::debug_assert_eq!(
        result.len(),
        source.len(),
        "selectors should never change row count"
    );

    Ok(result)
}

impl Transform for Segment {
    type Source = ListArray;
    type Target = ListArray;

    fn transform(&self, source: &Self::Source) -> std::result::Result<Self::Target, crate::Error> {
        match self {
            Self::Field(field_name) => {
                MapList::new(GetField::new(field_name.clone())).transform(source)
            }
            Self::Index(index) => MapList::new(GetIndexList::new(*index)).transform(source),
            Self::Each => {
                // In Arrow's columnar context, [] flattens one level of list nesting
                // while preserving row count, rather than exploding to create new rows.
                // This reinterprets jq's streaming iteration as structural unwrapping.
                if source
                    .values()
                    .as_any()
                    .downcast_ref::<ListArray>()
                    .is_some()
                {
                    // Flatten nested lists: List<List<T>> -> List<T>
                    Flatten::new().transform(source)
                } else {
                    Err(crate::Error::TypeMismatch {
                        expected: "ListArray".into(),
                        actual: source.value_type(),
                        context: "Each ([]) operator requires nested lists".into(),
                    })
                }
            }
        }
    }
}

impl Transform for Expr {
    type Source = ListArray;
    type Target = ListArray;

    fn transform(&self, source: &Self::Source) -> std::result::Result<Self::Target, crate::Error> {
        match self {
            Self::Identity => Ok(source.clone()),
            Self::Path(segments) => {
                let mut result = source.clone();
                for segment in segments {
                    result = segment.transform(&result)?;
                }
                Ok(result)
            }
            Self::Pipe(left, right) => {
                let intermediate = left.as_ref().transform(source)?;
                right.as_ref().transform(&intermediate)
            }
        }
    }
}
