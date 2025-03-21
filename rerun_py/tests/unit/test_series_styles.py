from __future__ import annotations

from rerun.archetypes import SeriesLines, SeriesPoints
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
        SeriesLines(),
        SeriesLines(colors=[255, 0, 0]),
        SeriesLines(colors=0xFF0000FF),
        SeriesLines(widths=2),
        SeriesLines(widths=2.0),
        SeriesLines(names="my plot"),
        SeriesLines(colors=[255, 0, 0], widths=2, names="my plot"),
    ]

    for input in inputs:
        if input.colors is not None:
            assert input.colors == ColorBatch._converter([Color([255, 0, 0])])
        if input.widths is not None:
            assert input.widths == StrokeWidthBatch._converter([StrokeWidth(2.0)])
        if input.names is not None:
            assert input.names == NameBatch._converter([Name("my plot")])


def test_point_series() -> None:
    inputs = [
        SeriesPoints(),
        SeriesPoints(colors=[255, 0, 0]),
        SeriesPoints(colors=0xFF0000FF),
        SeriesPoints(marker_sizes=2),
        SeriesPoints(markers=MarkerShape.Diamond),
        SeriesPoints(markers="diamond"),
        SeriesPoints(names="my plot"),
        SeriesPoints(colors=[255, 0, 0], marker_sizes=2, markers="diamond", names="my plot"),
    ]

    for input in inputs:
        if input.colors is not None:
            assert input.colors == ColorBatch._converter([Color([255, 0, 0])])
        if input.marker_sizes is not None:
            assert input.marker_sizes == MarkerSizeBatch._converter([MarkerSize(2.0)])
        if input.markers is not None:
            assert input.markers == MarkerShapeBatch._converter([MarkerShape.Diamond])
        if input.names is not None:
            assert input.names == NameBatch._converter([Name("my plot")])


if __name__ == "__main__":
    test_point_series()
