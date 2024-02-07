from __future__ import annotations

from rerun.archetypes import SeriesLine, SeriesPoint
from rerun.components import (
    Color,
    ColorBatch,
    MarkerShape,
    MarkerShapeBatch,
    MarkerSize,
    MarkerSizeBatch,
    Name,
    NameBatch,
    StrokeWidth,
    StrokeWidthBatch,
)


def test_line_series() -> None:
    inputs = [
        SeriesLine(),
        SeriesLine(color=[255, 0, 0]),
        SeriesLine(color=0xFF0000FF),
        SeriesLine(width=2),
        SeriesLine(width=2.0),
        SeriesLine(name="my plot"),
        SeriesLine(color=[255, 0, 0], width=2, name="my plot"),
    ]

    for input in inputs:
        if input.color is not None:
            assert input.color == ColorBatch._optional([Color([255, 0, 0])])
        if input.width is not None:
            assert input.width == StrokeWidthBatch._optional([StrokeWidth(2.0)])
        if input.name is not None:
            assert input.name == NameBatch._optional([Name("my plot")])


def test_point_series() -> None:
    inputs = [
        SeriesPoint(),
        SeriesPoint(color=[255, 0, 0]),
        SeriesPoint(color=0xFF0000FF),
        SeriesPoint(marker_size=2),
        SeriesPoint(marker=MarkerShape.Diamond),
        SeriesPoint(marker="diamond"),
        SeriesPoint(name="my plot"),
        SeriesPoint(color=[255, 0, 0], marker_size=2, marker="diamond", name="my plot"),
    ]

    for input in inputs:
        if input.color is not None:
            assert input.color == ColorBatch._optional([Color([255, 0, 0])])
        if input.marker_size is not None:
            assert input.marker_size == MarkerSizeBatch._optional([MarkerSize(2.0)])
        if input.marker is not None:
            assert input.marker == MarkerShapeBatch._optional([MarkerShape.Diamond])
        if input.name is not None:
            assert input.name == NameBatch._optional([Name("my plot")])


if __name__ == "__main__":
    test_point_series()
