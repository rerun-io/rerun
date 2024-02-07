from __future__ import annotations

from rerun.archetypes import SeriesPoint
from rerun.components import Color, ColorBatch, MarkerShape, MarkerShapeBatch, MarkerSize, MarkerSizeBatch


def test_point_series() -> None:
    inputs = [
        SeriesPoint(),
        SeriesPoint(color=[255, 0, 0]),
        SeriesPoint(color=0xFF0000FF),
        SeriesPoint(marker_size=2),
        SeriesPoint(marker=MarkerShape.Diamond),
        SeriesPoint(marker="diamond"),
        SeriesPoint(color=[255, 0, 0], marker_size=2, marker="diamond"),
    ]

    for input in inputs:
        if input.color is not None:
            assert input.color == ColorBatch._optional([Color([255, 0, 0])])
        if input.marker_size is not None:
            assert input.marker_size == MarkerSizeBatch._optional([MarkerSize(2.0)])
        if input.marker is not None:
            assert input.marker == MarkerShapeBatch._optional([MarkerShape.Diamond])


if __name__ == "__main__":
    test_point_series()
