from rerun_bindings import _IndexValuesLikeInternal as IndexValuesLike
import pyarrow as pa
import numpy as np
import pytest
from contextlib import nullcontext

# Generate source data so truncation doesn't lose correctness.
MS_TO_NS = 1_000_000
SOME_ARRAY = np.arange(0, MS_TO_NS * 1000, MS_TO_NS, dtype=np.int64)


@pytest.mark.parametrize(
    "input, expected, context",
    [
        (SOME_ARRAY, SOME_ARRAY, nullcontext()),
        (SOME_ARRAY.astype("datetime64[ns]"), SOME_ARRAY, nullcontext()),
        (SOME_ARRAY.astype("datetime64[ns]").astype("datetime64[ms]"), SOME_ARRAY, nullcontext()),
        (pa.array(SOME_ARRAY), SOME_ARRAY, nullcontext()),
        # Check error modes
        (SOME_ARRAY.astype(np.float32), SOME_ARRAY, pytest.raises(TypeError, match="IndexValuesLike must be a")),
    ],
)
def test_index_values_like_to_index_values(input, expected, context) -> None:
    """Verify that IndexValuesLike converts to the expected list of i64 index values."""
    with context:
        result = IndexValuesLike(input).to_index_values()
        assert np.array_equal(result, expected)
