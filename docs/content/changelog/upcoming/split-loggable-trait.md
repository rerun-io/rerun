---
title: "`Loggable` replaced by four (de)serialization traits"
hidden: true
type: breaking
---

### `Loggable` replaced by four (de)serialization traits

**Rust SDK only.** The Python and C++ SDKs are unaffected, and so is the data format — the Arrow
encodings are byte-for-byte identical.

This only matters if you implement your own components or encodings in Rust, which is rare.
Logging built-in archetypes and components needs no changes.

The `Loggable` trait bundled five functions, forcing every type to provide all of them even when some were meaningless.
Types whose Arrow encoding is never nullable had to supply a `to_arrow_opt` that failed at runtime, and a type that implemented only serialization got default `from_arrow`/`from_arrow_opt` bodies that call each other, recursing forever.

`Loggable` is gone. The functions now live in separate traits, so a type implements only what makes sense for it:

| Trait           | Function(s)                            | Status                     |
| --------------- | -------------------------------------- | -------------------------- |
| `ArrowDatatype` | `arrow_datatype`, `arrow_empty`        | supertrait of the other four |
| `ToArrow`       | `to_arrow`                             | required by `Component`    |
| `ToArrowOpt`    | `to_arrow_opt`                         | optional                   |
| `FromArrow`     | `from_arrow`, `verify_arrow_array`     | required by `Component`    |
| `FromArrowOpt`  | `from_arrow_opt`                       | optional                   |

`Component` now requires `ToArrow + FromArrow`: a component must round-trip, but does not have to be nullable.

The nullable variants are only implemented where they are actually needed, so most built-in types no longer have them.
Of the roughly 90 built-in encodings, 19 do — the ones that appear as a nullable field of another type, such as `Utf8`, `Blob`, `ImageFormat`, `PixelFormat` and `TensorBuffer`.
Components inherit the traits from the encoding they wrap, so 11 of them have the nullable variants (`Text`, `Name`, `MediaType`, `ImageBuffer`, …) and the rest, including `Position2D` and `Color`, do not.
Prefer the non-nullable variants in new code.

#### Migration

Split your `impl Loggable` into one `impl` per trait, and import the specific traits you call.
Implement `ToArrow` and `FromArrow`; only add the `*Opt` variants if your type is used as a nullable field of another type.

Before:

```rust
use rerun::Loggable as _;

impl rerun::Loggable for Confidence {
    fn arrow_datatype() -> arrow::datatypes::DataType {
        rerun::Float32::arrow_datatype()
    }

    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<std::borrow::Cow<'a, Self>>>>,
    ) -> rerun::SerializationResult<arrow::array::ArrayRef>
    where
        Self: 'a,
    {
        rerun::Float32::to_arrow_opt(data.into_iter().map(|opt| opt.map(Into::into).map(|c| c.0)))
    }
}
```

After:

```rust
impl rerun::ArrowDatatype for Confidence {
    fn arrow_datatype() -> arrow::datatypes::DataType {
        <rerun::Float32 as rerun::ArrowDatatype>::arrow_datatype()
    }
}

impl rerun::ToArrow for Confidence {
    fn to_arrow<'a>(
        data: impl IntoIterator<Item = impl Into<std::borrow::Cow<'a, Self>>>,
    ) -> rerun::SerializationResult<arrow::array::ArrayRef>
    where
        Self: 'a,
    {
        <rerun::Float32 as rerun::ToArrow>::to_arrow(data.into_iter().map(Into::into).map(|c| c.0))
    }
}

impl rerun::FromArrow for Confidence {
    fn from_arrow(
        data: &dyn arrow::array::Array,
    ) -> rerun::DeserializationResult<Vec<Self>> {
        Ok(<rerun::Float32 as rerun::FromArrow>::from_arrow(data)?
            .into_iter()
            .map(Confidence)
            .collect())
    }
}
```

A type that is never nullable is now done at that point — `RowId`, `ChunkId`, `Tuid` and `EntityPath` all skip the `*Opt` traits entirely.

If you do need the nullable variants, implement whichever direction is natural and derive the other with a macro, since sibling traits cannot supply each other's default bodies:

```rust
rerun::macros::impl_to_arrow_via_to_arrow_opt!(Confidence);      // `ToArrow` from `ToArrowOpt`
rerun::macros::impl_from_arrow_via_from_arrow_opt!(Confidence);  // `FromArrow` from `FromArrowOpt`
rerun::macros::impl_from_arrow_opt_via_from_arrow!(Confidence);  // `FromArrowOpt` from `FromArrow`
```

Call sites that did `use rerun::Loggable as _;` should import the traits whose functions they actually call, e.g. `use rerun::{FromArrow as _, ToArrow as _};`.

#### Batches of optional components

`Vec<Option<C>>`, `[Option<C>; N]` and `[Option<C>]` implement `ComponentBatch` only when `C: ToArrowOpt`.
Since most components no longer implement it, logging a batch with gaps in it no longer compiles for them:

```rust
// No longer compiles: `Position2D` is not `ToArrowOpt`.
vec![Some(Position2D::new(1.0, 2.0)), None].serialized(descriptor)

// Still fine: `Text` wraps `Utf8`, which keeps the nullable traits.
vec![Some(Text::from("a")), None].serialized(descriptor)
```

Log the present values instead, or use `RecordingStream::send_columns` with explicit partition lengths if you need to express absence.

Example: [`custom_data`](https://github.com/rerun-io/rerun/blob/main/docs/snippets/all/tutorials/custom_data.rs)
