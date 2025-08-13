from __future__ import annotations

from typing import TYPE_CHECKING, Any

from ._converters import to_np_float32, to_np_float64, to_np_uint32, to_np_uint64

if TYPE_CHECKING:
    import numpy as np
    import numpy.typing as npt


# This code is a straight port from Rust.
def find_non_empty_dim_indices(shape: list[int]) -> list[int]:
    """Returns the indices of an appropriate set of non-empty dimensions."""
    if len(shape) <= 2:
        return list(range(len(shape)))

    # Find a range of non-unit dimensions.
    # [1, 1, 1, 640, 480, 3, 1, 1, 1]
    #           ^---------^   goal range

    non_unit_indices = [d[0] for d in filter(lambda d: d[1] != 1, enumerate(shape))]

    # 0 is always a valid index.
    min = next(iter(non_unit_indices), 0)
    max = next(reversed(non_unit_indices), min)

    # Note, these are inclusive ranges

    # First, empty inner dimensions are more likely to be intentional than empty outer dimensions.
    # Grow to a min-size of 2.
    # (1x1x3x1) -> 3x1 mono rather than 1x1x3 RGB
    while max - min < 1 and max + 1 < len(shape):
        max += 1

    target_len = 2
    if shape[max] in (3, 4):
        target_len = 3

    # Next, consider empty outer dimensions if we still need them.
    # Grow up to 3 if the inner dimension is already 3 or 4 (Color Images)
    # Otherwise, only grow up to 2.
    # (1x1x3) -> 1x1x3 rgb rather than 1x3 mono
    while max - min + 1 < target_len and 0 < min:
        min -= 1

    return list(range(min, max + 1))


def flat_np_float32_array_from_array_like(data: Any, dimension: int) -> npt.NDArray[np.float32]:
    """Converts to a flat float numpy array from an arbitrary vector, validating for an expected dimensionality."""

    array = to_np_float32(data)
    return flat_np_array_from_array_like(array, dimension)


def flat_np_float64_array_from_array_like(data: Any, dimension: int) -> npt.NDArray[np.float64]:
    """Converts to a flat float numpy array from an arbitrary vector, validating for an expected dimensionality."""

    array = to_np_float64(data)
    return flat_np_array_from_array_like(array, dimension)


def flat_np_uint32_array_from_array_like(data: Any, dimension: int) -> npt.NDArray[np.uint32]:
    """Converts to a flat uint numpy array from an arbitrary vector, validating for an expected dimensionality."""

    array = to_np_uint32(data)
    return flat_np_array_from_array_like(array, dimension)


def flat_np_uint64_array_from_array_like(data: Any, dimension: int) -> npt.NDArray[np.uint64]:
    """Converts to a flat uint numpy array from an arbitrary vector, validating for an expected dimensionality."""

    array = to_np_uint64(data)
    return flat_np_array_from_array_like(array, dimension)


def flat_np_array_from_array_like(array: npt.NDArray[Any], dimension: int) -> npt.NDArray[Any]:
    """Converts to a flat numpy array from an arbitrary vector, validating for an expected dimensionality."""

    valid = True
    if len(array.shape) == 1:
        valid = (array.shape[0] % dimension) == 0
    elif len(array.shape) >= 2:
        # Get the last dimension that is not 1. If all dimensions are 1, this returns 1.
        last_non_singleton = next((d for d in reversed(array.shape[1:]) if d != 1), 1)
        valid = last_non_singleton == dimension

    if not valid:
        raise ValueError(
            f"Expected either a flat array with a length multiple of {dimension} elements, or an array with shape (`num_elements`, {dimension}). Shape of passed array was {array.shape}.",
        )

    return array.reshape((-1,))


if __name__ == "__main__":
    # Unit-test
    def expect(input: list[int], expected: list[int]) -> None:
        got = find_non_empty_dim_indices(input)
        assert got == expected, f"input: {input}, expected {expected}, got {got}"

    expect([], [])
    expect([0], [0])
    expect([1], [0])
    expect([100], [0])

    expect([640, 480], [0, 1])
    expect([640, 480, 1], [0, 1])
    expect([640, 480, 1, 1], [0, 1])
    expect([640, 480, 3], [0, 1, 2])
    expect([1, 640, 480], [1, 2])
    expect([1, 640, 480, 3, 1], [1, 2, 3])
    expect([1, 3, 640, 480, 1], [1, 2, 3])
    expect([1, 1, 640, 480], [2, 3])
    expect([1, 1, 640, 480, 1, 1], [2, 3])

    expect([1, 1, 3], [0, 1, 2])
    expect([1, 1, 3, 1], [2, 3])
