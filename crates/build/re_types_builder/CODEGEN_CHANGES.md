# Code Generator Changes for expect() Migration

## What Changed

The code generator has been updated to emit `expect()` with descriptive messages instead of `unwrap()` with `#[expect(clippy::unwrap_used)]` attributes.

### Modified Files

1. **crates/build/re_types_builder/src/codegen/rust/serializer.rs**
   - Line 636-639 and 655-658: Changed `offsets.last().copied().unwrap()` to use `expect()`
   - Message: "Offsets buffer must have at least one element"

2. **crates/build/re_types_builder/src/codegen/rust/deserializer.rs**
   - Line 748-750: Changed `array_init::from_iter(data).unwrap()` to use `expect()`
   - Message: "Array length must match expected size"

## Why These Changes Are Safe

### Serializer Pattern (`offsets.last().copied().unwrap()`)
- Arrow's `OffsetBuffer::from_lengths()` always creates a buffer with at least one element (the initial 0)
- The last element represents the total buffer capacity needed
- This is a fundamental invariant of Arrow's offset-based arrays

### Deserializer Pattern (`array_init::from_iter(data).unwrap()`)
- Used when deserializing fixed-size arrays (e.g., `[f32; 2]` for Vec2D)
- Length is validated before this point through Arrow's FixedSizeListArray
- The comment "Unwrapping cannot fail: the length must be correct" is preserved
- Failure would indicate corrupt data or a bug in the generator

## How to Regenerate Types

To apply these changes to the generated code, run:

```bash
pixi run codegen
```

Or manually:

```bash
cargo run --bin build_re_types
```

**Note:** Requires `flatc` (FlatBuffers compiler) to be installed.

## Impact

This change will affect approximately **23 auto-generated datatype files** in:
- `crates/store/re_types/src/datatypes/*.rs`
- `crates/store/re_types_core/src/datatypes/*.rs`

Each will have improved error messages that help with debugging if the impossible happens.

## Testing

The code generator itself compiles successfully with these changes:
```bash
cargo check -p re_types_builder  # ✓ Passes
```

Once types are regenerated, verify with:
```bash
cargo check --workspace  # Should pass with no new errors
cargo clippy --workspace  # Should pass with no new clippy warnings
```

## Related

- Issue #3408: Forbid unwrap()
- All unwraps were already marked with `#[expect(clippy::unwrap_used)]`
- This change makes error messages more helpful without changing behavior
