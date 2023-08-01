from __future__ import annotations

import itertools
from typing import Optional, cast

import numpy as np
import rerun.experimental as rr2
from rerun.experimental import cmp as rrc
from rerun.experimental import dt as rrd

U64_MAX_MINUS_1 = 2**64 - 2
U64_MAX = 2**64 - 1


def test_arrows3d() -> None:
    arrows_arrays: list[rrd.Arrow3DArrayLike] = [
        [],
        # Arrow3DArrayLike: Sequence[Arrow3DLike]: Arrow3D
        [
            rrd.Arrow3D(origin=[1, 2, 3], vector=[4, 5, 6]),
            rrd.Arrow3D(origin=[10, 20, 30], vector=[40, 50, 60]),
        ],
    ]

    radii_arrays: list[rrc.RadiusArrayLike | None] = [
        None,
        [],
        np.array([]),
        # RadiusArrayLike: Sequence[RadiusLike]: float
        [1, 10],
        # RadiusArrayLike: Sequence[RadiusLike]: Radius
        [
            rrc.Radius(1),
            rrc.Radius(10),
        ],
        # RadiusArrayLike: npt.NDArray[np.float32]
        np.array([1, 10], dtype=np.float32),
    ]

    colors_arrays: list[rrc.ColorArrayLike | None] = [
        None,
        [],
        np.array([]),
        # ColorArrayLike: Sequence[ColorLike]: int
        [
            0xAA0000CC,
            0x00BB00DD,
        ],
        # ColorArrayLike: Sequence[ColorLike]: Color
        [
            rrc.Color(0xAA0000CC),
            rrc.Color(0x00BB00DD),
        ],
        # ColorArrayLike: Sequence[ColorLike]: npt.NDArray[np.uint8]
        np.array(
            [
                [0xAA, 0x00, 0x00, 0xCC],
                [0x00, 0xBB, 0x00, 0xDD],
            ],
            dtype=np.uint8,
        ),
        # ColorArrayLike: Sequence[ColorLike]: npt.NDArray[np.uint32]
        np.array(
            [
                [0xAA0000CC],
                [0x00BB00DD],
            ],
            dtype=np.uint32,
        ),
        # ColorArrayLike: Sequence[ColorLike]: npt.NDArray[np.float32]
        np.array(
            [
                [0xAA / 0xFF, 0.0, 0.0, 0xCC / 0xFF],
                [0.0, 0xBB / 0xFF, 0.0, 0xDD / 0xFF],
            ],
            dtype=np.float32,
        ),
        # ColorArrayLike: Sequence[ColorLike]: npt.NDArray[np.float64]
        np.array(
            [
                [0xAA / 0xFF, 0.0, 0.0, 0xCC / 0xFF],
                [0.0, 0xBB / 0xFF, 0.0, 0xDD / 0xFF],
            ],
            dtype=np.float64,
        ),
        # ColorArrayLike: npt.NDArray[np.uint8]
        np.array(
            [
                0xAA,
                0x00,
                0x00,
                0xCC,
                0x00,
                0xBB,
                0x00,
                0xDD,
            ],
            dtype=np.uint8,
        ),
        # ColorArrayLike: npt.NDArray[np.uint32]
        np.array(
            [
                0xAA0000CC,
                0x00BB00DD,
            ],
            dtype=np.uint32,
        ),
        # ColorArrayLike: npt.NDArray[np.float32]
        np.array(
            [
                0xAA / 0xFF,
                0.0,
                0.0,
                0xCC / 0xFF,
                0.0,
                0xBB / 0xFF,
                0.0,
                0xDD / 0xFF,
            ],
            dtype=np.float32,
        ),
        # ColorArrayLike: npt.NDArray[np.float64]
        np.array(
            [
                0xAA / 0xFF,
                0.0,
                0.0,
                0xCC / 0xFF,
                0.0,
                0xBB / 0xFF,
                0.0,
                0xDD / 0xFF,
            ],
            dtype=np.float64,
        ),
    ]

    labels_arrays: list[rrc.LabelArrayLike | None] = [
        None,
        [],
        # LabelArrayLike: Sequence[LabelLike]: str
        ["hello", "friend"],
        # LabelArrayLike: Sequence[LabelLike]: Label
        [
            rrc.Label("hello"),
            rrc.Label("friend"),
        ],
    ]

    class_id_arrays = [
        [],
        np.array([]),
        # ClassIdArrayLike: Sequence[ClassIdLike]: int
        [126, 127],
        # ClassIdArrayLike: Sequence[ClassIdLike]: ClassId
        [rrc.ClassId(126), rrc.ClassId(127)],
        # ClassIdArrayLike: np.NDArray[np.uint8]
        np.array([126, 127], dtype=np.uint8),
        # ClassIdArrayLike: np.NDArray[np.uint16]
        np.array([126, 127], dtype=np.uint16),
        # ClassIdArrayLike: np.NDArray[np.uint32]
        np.array([126, 127], dtype=np.uint32),
        # ClassIdArrayLike: np.NDArray[np.uint64]
        np.array([126, 127], dtype=np.uint64),
    ]

    instance_key_arrays = [
        [],
        np.array([]),
        # InstanceKeyArrayLike: Sequence[InstanceKeyLike]: int
        [U64_MAX_MINUS_1, U64_MAX],
        # InstanceKeyArrayLike: Sequence[InstanceKeyLike]: InstanceKey
        [rrc.InstanceKey(U64_MAX_MINUS_1), rrc.InstanceKey(U64_MAX)],
        # InstanceKeyArrayLike: np.NDArray[np.uint64]
        np.array([U64_MAX_MINUS_1, U64_MAX], dtype=np.uint64),
    ]

    all_arrays = itertools.zip_longest(
        arrows_arrays,
        radii_arrays,
        colors_arrays,
        labels_arrays,
        class_id_arrays,
        instance_key_arrays,
    )

    for arrows, radii, colors, labels, class_ids, instance_keys in all_arrays:
        arrows = arrows if arrows is not None else arrows_arrays[-1]

        # make Pyright happy as it's apparently not able to track typing info trough zip_longest
        arrows = cast(Optional[rrd.Arrow3DArrayLike], arrows)
        radii = cast(Optional[rrc.RadiusArrayLike], radii)
        colors = cast(Optional[rrc.ColorArrayLike], colors)
        labels = cast(Optional[rrc.LabelArrayLike], labels)
        class_ids = cast(Optional[rrc.ClassIdArrayLike], class_ids)
        instance_keys = cast(Optional[rrc.InstanceKeyArrayLike], instance_keys)

        print(
            f"rr2.Arrows3D(\n"
            f"    {arrows}\n"
            f"    radii={radii}\n"
            f"    colors={colors}\n"
            f"    labels={labels}\n"
            f"    class_ids={class_ids}\n"
            f"    instance_keys={instance_keys}\n"
            f")"
        )
        arch = rr2.Arrows3D(
            arrows,
            radii=radii,
            colors=colors,
            labels=labels,
            class_ids=class_ids,
            instance_keys=instance_keys,
        )
        print(f"{arch}\n")

        assert arch.arrows == rrc.Arrow3DArray.from_similar(
            [
                rrd.Arrow3D(origin=[1, 2, 3], vector=[4, 5, 6]),
                rrd.Arrow3D(origin=[10, 20, 30], vector=[40, 50, 60]),
            ]
            if non_empty(arrows)
            else []
        )
        assert arch.radii == rrc.RadiusArray.from_similar([1, 10] if non_empty(radii) else [])
        assert arch.colors == rrc.ColorArray.from_similar([0xAA0000CC, 0x00BB00DD] if non_empty(colors) else [])
        assert arch.labels == rrc.LabelArray.from_similar(["hello", "friend"] if non_empty(labels) else [])
        assert arch.class_ids == rrc.ClassIdArray.from_similar([126, 127] if non_empty(class_ids) else [])
        assert arch.instance_keys == rrc.InstanceKeyArray.from_similar(
            [U64_MAX_MINUS_1, U64_MAX] if non_empty(instance_keys) else []
        )


def non_empty(v: object) -> bool:
    return v is not None and len(v) > 0  # type: ignore[arg-type]


if __name__ == "__main__":
    test_arrows3d()
