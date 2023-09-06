from __future__ import annotations


# This code follows closely from `image_ext.rs`
def find_non_empty_dim_indices(shape: list[int]) -> list[int]:
    """Returns the indices of an appropriate set of non-empty dimensions."""
    if len(shape) < 2:
        return list(range(len(shape)))

    indices = list(d[0] for d in filter(lambda d: d[1] != 1, enumerate(shape)))

    # 0 must be valid since shape isn't empty or we would have returned an Err above
    first_non_empty = next(iter(indices), 0)
    last_non_empty = next(reversed(indices), first_non_empty)

    # Note, these are inclusive ranges

    # First, empty inner dimensions are more likely to be intentional than empty outer dimensions.
    # Grow to a min-size of 2.
    # (1x1x3x1) -> 3x1 mono rather than 1x1x3 RGB
    while (last_non_empty - first_non_empty) < 1 and last_non_empty < (len(shape) - 1):
        print(f"{last_non_empty} {first_non_empty} {len(shape)}")
        last_non_empty += 1

    target = 1
    if shape[last_non_empty] in (3, 4):
        target = 2

    # Next, consider empty outer dimensions if we still need them.
    # Grow up to 3 if the inner dimension is already 3 or 4 (Color Images)
    # Otherwise, only grow up to 2.
    # (1x1x3) -> 1x1x3 rgb rather than 1x3 mono
    while (last_non_empty - first_non_empty) < target and first_non_empty > 0:
        first_non_empty -= 1

    return list(range(first_non_empty, last_non_empty + 1))
