from __future__ import annotations

import itertools

import rerun.experimental as rr2
from rerun.experimental import cmp as rr_cmp


def test_disconnected_space() -> None:
    disconnected_spaces: list[rr_cmp.DisconnectedSpaceLike] = [
        # DisconnectedSpaceLike: bool
        True,
        # DisconnectedSpaceLike: DisconnectedSpace
        rr_cmp.DisconnectedSpace(True),
    ]

    all_arrays = itertools.zip_longest(
        disconnected_spaces,
    )

    for disconnected_space in all_arrays:
        print(f"rr2.DisconnectedSpace(\n" f"    disconnected_space={disconnected_space}\n" f")")
        arch = rr2.DisconnectedSpace(disconnected_space)
        print(f"{arch}\n")

        assert arch.disconnected_space == rr_cmp.DisconnectedSpaceBatch([True])


if __name__ == "__main__":
    test_disconnected_space()
