# Hypothesis 1: downcast PyComponentDescriptor in descriptor_to_rust()

## Hypothesis
`descriptor_to_rust()` in `arrow.rs` receives `Bound<'_, PyAny>` that is always a `PyComponentDescriptor` (already wrapping a Rust `ComponentDescriptor`). Currently does 3 `getattr()` calls with string extraction and re-interning. Downcast directly instead.

## Rationale
Eliminates 6 heap allocations + 6 string interning ops + 3 Python attribute lookups per log call.

## Code changes
In `rerun_py/src/arrow.rs`, added fast-path downcast before the existing getattr-based extraction:
```rust
if let Ok(py_descr) = component_descr.downcast::<PyComponentDescriptor>() {
    return Ok(py_descr.borrow().0.clone());
}
```

## Results (release build, 100 entities x 1000 time steps)

| Metric | Baseline | H1 | Change |
|--------|----------|-----|--------|
| Full log | 36.8k xforms/s | 37.3k xforms/s | +1.4% |

## Decision: KEEP (despite <10% threshold)
The improvement is negligible (~1.4%), below the 10% threshold. However, the change is zero-risk (falls back to existing path), removes unnecessary work, and is only 3 lines. Keeping it as a correctness/cleanliness improvement that combines well with H3 (descriptor caching).
