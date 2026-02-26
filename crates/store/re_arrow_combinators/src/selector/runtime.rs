//! Runtime execution of [`Expr`] against Arrow [`ListArray`]s.
//!
//! This module implements execution of expressions against Arrow [`ListArray`]s.

use arrow::array::{Array as _, ListArray};

use crate::{
    Transform as _,
    index::GetIndexList,
    map::MapList,
    reshape::{Flatten, GetField},
};

use super::parser::{Expr, Segment, SegmentKind};

/// Executes the given expression against the source array.
///
/// Returns `None` if the expression was suppressed by an optional segment (e.g. `.field?`).
/// The caller decides how to handle the absent result.
pub fn execute_per_row(expr: &Expr, source: &ListArray) -> Result<Option<ListArray>, crate::Error> {
    // TODO(grtlr): It would be much cleaner if `MapList` (or equivalent would be called on this level).
    let result = expr.execute(source)?;

    if let Some(ref result) = result {
        re_log::debug_assert_eq!(
            result.len(),
            source.len(),
            "selectors should never change row count"
        );
    }

    Ok(result)
}

impl SegmentKind {
    fn execute(&self, source: &ListArray) -> Result<ListArray, crate::Error> {
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

impl Segment {
    fn execute(&self, source: &ListArray) -> Result<Option<ListArray>, crate::Error> {
        match self.kind.execute(source) {
            Ok(result) => Ok(Some(result)),
            // TODO(RR-3435): FixedSizeListArray errors must be suppressed via optional, but ListArray should not need it.
            Err(err) if self.optional => {
                re_log::trace!("Optional segment `{self}` suppressed error: {err}");
                Ok(None)
            }
            Err(err) => Err(err),
        }
    }
}

impl Expr {
    fn execute(&self, source: &ListArray) -> Result<Option<ListArray>, crate::Error> {
        match self {
            Self::Identity => Ok(Some(source.clone())),
            Self::Path(segments) => {
                let mut result = source.clone();
                for segment in segments {
                    match segment.execute(&result)? {
                        Some(next) => result = next,
                        None => return Ok(None),
                    }
                }
                Ok(Some(result))
            }
            Self::Pipe(left, right) => match left.as_ref().execute(source)? {
                Some(intermediate) => right.as_ref().execute(&intermediate),
                None => Ok(None),
            },
        }
    }
}
