from __future__ import annotations

import itertools

import rerun as rr
from rerun.components import DisconnectedSpace, DisconnectedSpaceBatch, DisconnectedSpaceLike


def test_disconnected_space() -> None:
    disconnected_spaces: list[DisconnectedSpaceLike] = [
        # DisconnectedSpaceLike: bool
        True,
        # DisconnectedSpaceLike: DisconnectedSpace
        DisconnectedSpace(True),
    ]

    all_arrays = itertools.zip_longest(
        disconnected_spaces,
    )

    for disconnected_space in all_arrays:
        print(f"rr.DisconnectedSpace(\n" f"    disconnected_space={disconnected_space}\n" f")")
        arch = rr.DisconnectedSpace(disconnected_space)
        print(f"{arch}\n")

        assert arch.disconnected_space == DisconnectedSpaceBatch([True])


if __name__ == "__main__":
    test_disconnected_space()
