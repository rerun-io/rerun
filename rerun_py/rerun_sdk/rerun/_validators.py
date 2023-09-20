from __future__ import annotations


# This code is a straight port from Rust.
def find_non_empty_dim_indices(shape: list[int]) -> list[int]:
    """Returns the indices of an appropriate set of non-empty dimensions."""
    if len(shape) <= 2:
        return list(range(len(shape)))

    # Find a range of non-unit dimensions.
    # [1, 1, 1, 640, 480, 3, 1, 1, 1]
    #           ^---------^   goal range

    non_unit_indices = list(d[0] for d in filter(lambda d: d[1] != 1, enumerate(shape)))

    # 0 is always a valid index.
    min = next(iter(non_unit_indices), 0)
    max = next(reversed(non_unit_indices), min)

    # Note, these are inclusive ranges

    # First, empty inner dimensions are more likely to be intentional than empty outer dimensions.
    # Grow to a min-size of 2.
    # (1x1x3x1) -> 3x1 mono rather than 1x1x3 RGB
    while max - min < 1 and max + 1 < len(shape):
        print(f"{max} {min} {len(shape)}")
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
