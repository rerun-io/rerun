# Hypothesis 4: streamline mat3x3 Numpy operations

## Hypothesis
The Mat3x3 conversion path creates a `Mat3x3` object even when data is already a numpy array. This involves: `np.asarray()` -> `reshape(3,3)` -> `ravel("F")` (always copies for C-contiguous input) -> attrs converter `to_np_float32` does `np.asarray()` AGAIN -> `np.ascontiguousarray()` (redundant). Skipping the `Mat3x3` object creation and doing the row-to-column conversion inline should be faster.

## Code changes
In `datatypes/mat3x3_ext.py` `native_to_pa_array_override()`:
Added fast path for `isinstance(data, np.ndarray)` that:
1. Reshapes to (-1, 3, 3) (handles both single and batch)
2. Transposes (0, 2, 1) for row-to-column conversion
3. Flattens and ensures float32 contiguous output
4. Skips creating any `Mat3x3` objects

## Results (release build, 100 entities x 1000 time steps, cumulative with H1+H2+H3+H5)

| Metric | H1+H2+H3+H5 | + H4 | Change |
|--------|--------------|------|--------|
| Full log | 42.6k xforms/s | 45.8k xforms/s | +7.5% |
| Create only | 119k xforms/s | 122k xforms/s | +2.5% |

## Decision: KEEP
~7.5% improvement on full log path. Eliminates unnecessary object creation and redundant numpy operations.
