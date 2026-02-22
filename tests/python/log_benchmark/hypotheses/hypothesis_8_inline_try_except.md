# Hypothesis 8: Eliminate catch_and_log_exceptions overhead on hot path

## Hypothesis
The `catch_and_log_exceptions` context manager costs ~0.29 us per enter/exit cycle due to thread-local bookkeeping (depth tracking, strict_mode save/restore). On the Transform3D hot path there are 4 such cycles: 1 in `Transform3D.__init__`, 1 in `BaseBatch.__init__` for Translation3DBatch, 1 in `BaseBatch.__init__` for TransformMat3x3Batch, and 1 in `log()`. Replacing the inner 3 with inline `try/except` saves ~0.87 us/call.

## Code Changes

### `_baseclasses.py` — `BaseBatch.__init__`
Replaced `with catch_and_log_exceptions(self.__class__.__name__, strict=strict):` with an inline `try/except` that:
- On success: sets `self.pa_array` and returns (zero overhead beyond `try` frame setup at ~0.03 us)
- On exception: checks `strict_mode()`, re-raises if strict, otherwise emits a `RerunWarning` via `warnings.warn()` and falls through to the empty array default

Added `strict_mode` to the module's imports from `error_utils`.

### `archetypes/transform3d_ext.py` — `Transform3D.__init__`
Replaced `with catch_and_log_exceptions(context=self.__class__.__name__):` with an inline `try/except` that:
- On success: calls `__attrs_init__` and returns
- On exception: checks `strict_mode()`, re-raises if strict, otherwise emits a `RerunWarning` and falls through to `__attrs_clear__()`

The warning message includes `{type(exc).__name__}({exc})` to preserve the specific error text that `test_expected_warnings` validates.

The outer `log()` function retains the `@catch_and_log_exceptions()` decorator since it's the outermost handler and its overhead (~0.29 us) is acceptable at 1 call per log.

## Results (release build, micro-benchmark)

| Metric | Before H8 | After H8 | Change |
|--------|-----------|----------|--------|
| Translation3DBatch(list) | ~1.9 us | ~1.8 us | -5% |
| Transform3D(translation=list, mat3x3=np) | ~5.5 us | ~5.0 us | -9% |
| Full rr.log(path, Transform3D(...)) | ~15.5 us | ~13.5 us | -5% (cumulative) |

## Decision: KEEP
Eliminates ~0.87 us from the construction path. Error handling semantics are preserved: exceptions are caught, warnings are emitted, empty arrays are returned. The `test_expected_warnings` test passes with correct warning messages.
