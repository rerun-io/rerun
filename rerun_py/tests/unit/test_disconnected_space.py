from __future__ import annotations

import rerun as rr
from rerun.components import DisconnectedSpace, DisconnectedSpaceBatch, DisconnectedSpaceLike


def test_disconnected_space() -> None:
    disconnected_spaces: list[DisconnectedSpaceLike] = [
        # DisconnectedSpaceLike: bool
        True,
        # DisconnectedSpaceLike: DisconnectedSpace
        DisconnectedSpace(),
    ]

    for disconnected_space in disconnected_spaces:
        print(f"rr.DisconnectedSpace(\n" f"    disconnected_space={disconnected_space}\n" f")")
        arch = rr.DisconnectedSpace(disconnected_space)
        print(f"{arch}\n")

        assert arch.disconnected_space == DisconnectedSpaceBatch([True])


if __name__ == "__main__":
    test_disconnected_space()
