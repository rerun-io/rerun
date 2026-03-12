//! Composable Arrow array transformations.

mod cast;
mod error;
mod index;
mod map;
mod reshape;
mod transform;

pub use self::{
    cast::{DowncastRef, ListToFixedSizeList, PrimitiveCast},
    error::Error,
    map::{
        MapFixedSizeList, MapList, MapPrimitive, ReplaceNull, StringPrefix, StringSuffix,
        string_prefix, string_prefix_nonempty, string_suffix, string_suffix_nonempty,
    },
    reshape::{Explode, Flatten, GetField, RowMajorToColumnMajor, StructToFixedList},
    transform::{Function, Then, Transform},
};

pub(crate) use self::{index::GetIndexList, reshape::PromoteInnerNulls};
