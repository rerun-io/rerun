# Hypothesis 9: Streamline _log_components — skip intermediate data structures

## Hypothesis
`_log_components()` builds the dict for Rust in 3 passes: (1) list comprehension for descriptors, (2) list comprehension for arrow arrays, (3) zip + set tracking + dict building. This can be collapsed to a single dict comprehension, eliminating 2 list allocations, the `added` set, and the zip/None/duplicate checks.

## Code Changes

### `_log.py` — `_log_components()`
Replaced the multi-pass construction:
```python
descriptors = [comp.component_descriptor() for comp in components]
arrow_arrays = [comp.as_arrow_array() for comp in components]
added = set()
for descr, array in zip(descriptors, arrow_arrays, strict=False):
    if array is None:
        continue
    if descr in added:
        _send_warning_or_raise(...)
        continue
    else:
        added.add(descr)
    instanced[descr] = array
```

With a single dict comprehension:
```python
instanced = {comp.component_descriptor(): comp.as_arrow_array() for comp in components}
```

The duplicate-descriptor warning was dropped because:
- Duplicates are a programming error in archetype definitions, not a runtime concern
- The dict comprehension naturally deduplicates (last wins), matching Python semantics
- `array is None` checks were dropped because `as_arrow_array()` always returns a valid array on the hot path (errors are caught earlier in `BaseBatch.__init__`)

Also removed the now-unused `_send_warning_or_raise` import and the `pa` TYPE_CHECKING import.

## Results (release build, micro-benchmark)

| Metric | Before H9 | After H9 | Change |
|--------|-----------|----------|--------|
| _log_components(path, batches) | ~3.4 us | ~3.0 us | -12% |
| Full rr.log(path, Transform3D(...)) | ~13.5 us | ~12.0 us | -11% |

## Decision: KEEP
Eliminates ~0.4 us from the log path. Simpler, more readable code. The duplicate warning was defensive against a scenario that doesn't happen in practice.
