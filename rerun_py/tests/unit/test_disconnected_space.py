from __future__ import annotations

import itertools
from typing import Optional, cast

import rerun.experimental as rr_exp
from rerun.experimental import cmp as rr_cmp


def test_disconnected_space() -> None:
    disconnected_spaces: list[rr_cmp.DisconnectedSpace] = [
        # DisconnectedSpaceLike: bool
        True,
        # DisconnectedSpaceLike: DisconnectedSpace
        rr_cmp.DisconnectedSpace(True),
    ]

    all_arrays = itertools.zip_longest(
        disconnected_spaces,
    )

    for disconnected_space in all_arrays:
        # make Pyright happy as it's apparently not able to track typing info trough zip_longest
        disconnected_space = cast(Optional[rr_cmp.DisconnectedSpaceArrayLike], disconnected_space)

        print(f"rr_exp.DisconnectedSpace(\n" f"    disconnected_space={disconnected_space}\n" f")")
        arch = rr_exp.DisconnectedSpace(disconnected_space)
        print(f"{arch}\n")

        assert arch.disconnected_space == rr_cmp.DisconnectedSpaceArray.from_similar([True])


if __name__ == "__main__":
    test_disconnected_space()
