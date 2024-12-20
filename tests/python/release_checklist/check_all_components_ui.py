from __future__ import annotations

import math
import os
from argparse import Namespace
from typing import Any, Iterable
from uuid import uuid4

import numpy as np
import rerun as rr

README = """\
# Component UI

In the streams view, for each top-level entity, select it, and check that the component list in the selection panel looks Nice(tm).
"""


class TestCase:
    """
    Test case information.

    Usage:
    - For component which are typically used in batch (e.g. Point2D), use the `batch` keyword argument.
    - For union component (e.g. Rotation3D), use the `alternatives` keyword argument.
    - Otherwise, use the `single` positional argument.
    - To exclude a component, use `disabled=True`
    """

    def __init__(
        self,
        single: Any | None = None,
        *,
        batch: Iterable[Any] | None = None,
        alternatives: Iterable[Any] | None = None,
        disabled: bool = False,
    ):
        assert (
            (single is not None) ^ (batch is not None) ^ (alternatives is not None) ^ disabled
        ), "Exactly one of single, batch, or alternatives must be provided"

        if batch is not None:
            batch = list(batch)
            assert len(batch) > 1, "Batch must have at least two elements"

        if alternatives is not None:
            alternatives = list(alternatives)
            assert len(alternatives) > 1, "Alternatives must have at least two elements"

        self._single = single
        self._batch = batch
        self._alternatives = alternatives
        self._disabled = disabled

    def disabled(self) -> bool:
        return self._disabled

    def single(self) -> Any:
        if self._single is not None:
            return self._single
        elif self._batch is not None:
            return self._batch[0]
        elif self._alternatives is not None:
            return self._alternatives[0]
        assert False, "Unreachable"

    def batch(self) -> list[Any] | None:
        return self._batch

    def alternatives(self) -> list[Any] | None:
        if self._alternatives is not None:
            return self._alternatives[1:]
        else:
            return None


ALL_COMPONENTS: dict[str, TestCase] = {
    "AggregationPolicyBatch": TestCase(rr.components.AggregationPolicy.Average),
    "AlbedoFactorBatch": TestCase(
        batch=[
            rr.components.AlbedoFactor([255, 255, 0, 255]),
            rr.components.AlbedoFactor([255, 0, 255, 255]),
            rr.components.AlbedoFactor([0, 255, 255, 255]),
        ]
    ),
    "AnnotationContextBatch": TestCase([
        rr.datatypes.ClassDescriptionMapElem(
            class_id=1,
            class_description=rr.datatypes.ClassDescription(
                info=rr.datatypes.AnnotationInfo(id=1, label="label", color=(255, 0, 0, 255)),
                keypoint_annotations=[
                    rr.datatypes.AnnotationInfo(id=1, label="one", color=(255, 0, 0, 255)),
                    rr.datatypes.AnnotationInfo(id=2, label="two", color=(255, 255, 0, 255)),
                    rr.datatypes.AnnotationInfo(id=3, label="three", color=(255, 0, 255, 255)),
                ],
                keypoint_connections=[(1, 2), (2, 3), (3, 1)],
            ),
        )
    ]),
    "AxisLengthBatch": TestCase(batch=[100.0, 200.0, 300.0]),
    "BlobBatch": TestCase(
        alternatives=[
            b"\x00\x01\x02\x03\x04\x05\x06\x07\x08\x09",
            np.random.randint(0, 255, (10, 10), dtype=np.uint8).tobytes(),
        ]
    ),
    "ClassIdBatch": TestCase(batch=[1, 2, 3, 6]),
    "ClearIsRecursiveBatch": TestCase(disabled=True),  # disabled because it messes with the logging
    "ColorBatch": TestCase(batch=[(255, 0, 0, 255), (0, 255, 0, 255), (0, 0, 255, 255)]),
    "ColormapBatch": TestCase(rr.components.Colormap.Viridis),
    "DepthMeterBatch": TestCase(1000.0),
    "DrawOrderBatch": TestCase(100.0),
    "EntityPathBatch": TestCase("my/entity/path"),
    "FillModeBatch": TestCase(
        batch=[
            rr.components.FillMode.MajorWireframe,
            rr.components.FillMode.DenseWireframe,
            rr.components.FillMode.Solid,
        ]
    ),
    "FillRatioBatch": TestCase(0.5),
    "GammaCorrectionBatch": TestCase(2.2),
    "GeoLineStringBatch": TestCase(
        batch=[
            ((0, 0), (1, 1), (2, 2)),
            ((3, 3), (4, 4), (5, 5)),
        ]
    ),
    "GraphEdgeBatch": TestCase(
        batch=[
            rr.components.GraphEdge("a", "b"),
            rr.components.GraphEdge("b", "c"),
            rr.components.GraphEdge("c", "a"),
        ]
    ),
    "GraphNodeBatch": TestCase(batch=["a", "b", "c"]),
    "GraphTypeBatch": TestCase(rr.components.GraphType.Directed),
    "HalfSize2DBatch": TestCase(batch=[(5.0, 10.0), (50, 30), (23, 45)]),
    "HalfSize3DBatch": TestCase(batch=[(5.0, 10.0, 20.0), (50, 30, 40), (23, 45, 67)]),
    "ImageBufferBatch": TestCase(
        alternatives=[
            b"\x00\x01\x02\x03\x04\x05\x06\x07\x08\x09",
            np.random.randint(0, 255, (10, 10), dtype=np.uint8).tobytes(),
        ]
    ),
    "ImageFormatBatch": TestCase(
        rr.datatypes.ImageFormat(
            width=1920,
            height=1080,
            pixel_format=rr.PixelFormat.NV12,
            color_model=rr.ColorModel.RGB,
            channel_datatype=rr.ChannelDatatype.F16,
        ),
    ),
    "ImagePlaneDistanceBatch": TestCase(batch=[100.0, 200.0, 300.0]),
    "KeypointIdBatch": TestCase(batch=[5, 6, 7]),
    "LatLonBatch": TestCase(batch=[(0, 1), (2, 3), (4, 5)]),
    "LengthBatch": TestCase(batch=[100.0, 200.0, 300.0]),
    "LineStrip2DBatch": TestCase(
        batch=[
            ((0, 0), (1, 1), (2, 2)),
            ((3, 3), (4, 4), (5, 5)),
            ((6, 6), (7, 7), (8, 8)),
        ]
    ),
    "LineStrip3DBatch": TestCase(
        batch=[
            ((0, 0, 0), (1, 1, 1), (2, 2, 2)),
            ((3, 3, 3), (4, 4, 4), (5, 5, 5)),
            ((6, 6, 6), (7, 7, 7), (8, 8, 8)),
        ]
    ),
    "MagnificationFilterBatch": TestCase(rr.components.MagnificationFilter.Linear),
    "MarkerShapeBatch": TestCase(
        batch=[
            rr.components.MarkerShape.Plus,
            rr.components.MarkerShape.Cross,
            rr.components.MarkerShape.Circle,
        ]
    ),
    "MarkerSizeBatch": TestCase(batch=[5.0, 1.0, 2.0]),
    "MediaTypeBatch": TestCase("application/jpg"),
    "NameBatch": TestCase(batch=["Hello World", "Foo Bar", "Baz Qux"]),
    "OpacityBatch": TestCase(0.5),
    "PinholeProjectionBatch": TestCase([(0, 1, 2), (3, 4, 5), (6, 7, 8)]),
    "Plane3DBatch": TestCase(rr.datatypes.Plane3D(normal=(1, 2, 3), distance=4)),
    "PoseRotationAxisAngleBatch": TestCase(
        rr.datatypes.RotationAxisAngle(axis=(1, 0, 0), angle=rr.datatypes.Angle(rad=math.pi))
    ),
    "PoseRotationQuatBatch": TestCase(batch=((1, 0, 0, 0), (0, 1, 0, 0), (0, 0, 1, 0))),
    "PoseScale3DBatch": TestCase(batch=[(1, 2, 3), (4, 5, 6), (7, 8, 9)]),
    "PoseTransformMat3x3Batch": TestCase(rr.datatypes.Mat3x3([1, 2, 3, 4, 5, 6, 7, 8, 9])),
    "PoseTranslation3DBatch": TestCase(batch=[(1, 2, 3), (4, 5, 6), (7, 8, 9)]),
    "Position2DBatch": TestCase(batch=[(0, 1), (2, 3), (4, 5)]),
    "Position3DBatch": TestCase(batch=[(0, 3, 4), (1, 4, 5), (2, 5, 6)]),
    "RadiusBatch": TestCase(batch=[4.5, 5, 6, 7]),
    "Range1DBatch": TestCase((0, 5)),
    "RecordingUriBatch": TestCase(
        batch=[
            rr.components.RecordingUri("file:///path/to/file"),
            rr.components.RecordingUri("rerun://localhost:51234/recording/some-recording-id"),
        ]
    ),
    "ResolutionBatch": TestCase((1920, 1080)),
    "RotationAxisAngleBatch": TestCase(
        rr.datatypes.RotationAxisAngle(axis=(1, 0, 0), angle=rr.datatypes.Angle(rad=math.pi))
    ),
    "RotationQuatBatch": TestCase(batch=((1, 0, 0, 0), (0, 1, 0, 0), (0, 0, 1, 0))),
    "ScalarBatch": TestCase(3),
    "Scale3DBatch": TestCase(batch=[(1, 2, 3), (4, 5, 6), (7, 8, 9)]),
    "ShowLabelsBatch": TestCase(alternatives=[True, False]),
    "StrokeWidthBatch": TestCase(2.0),
    "TensorDataBatch": TestCase(
        alternatives=[
            rr.datatypes.TensorData(array=np.random.randint(0, 255, (10, 10), dtype=np.uint8)),
            rr.datatypes.TensorData(array=np.random.randint(0, 255, (10, 10, 3), dtype=np.uint8)),
            rr.datatypes.TensorData(array=np.random.randint(0, 255, (5, 3, 6, 4), dtype=np.uint8)),
            rr.datatypes.TensorData(
                array=np.random.randint(0, 255, (5, 3, 6, 4), dtype=np.uint8),
                dim_names=["hello", "brave", "new", "world"],
            ),
            rr.datatypes.TensorData(array=np.random.randint(0, 255, (5, 3, 6, 4, 3), dtype=np.uint8)),
        ]
    ),
    "TensorDimensionIndexSelectionBatch": TestCase([
        rr.TensorDimensionIndexSelection(0, 1024),
    ]),
    "TensorHeightDimensionBatch": TestCase(1),
    "TensorWidthDimensionBatch": TestCase(0),
    "Texcoord2DBatch": TestCase(batch=[(0, 0), (1, 1), (2, 2)]),
    "TextBatch": TestCase("Hello world"),
    "TextLogLevelBatch": TestCase(batch=["INFO", "CRITICAL", "WARNING"]),
    "TransformMat3x3Batch": TestCase(rr.datatypes.Mat3x3([1, 2, 3, 4, 5, 6, 7, 8, 9])),
    "TransformRelationBatch": TestCase(
        batch=[
            rr.TransformRelation.ChildFromParent,
            rr.TransformRelation.ParentFromChild,
        ]
    ),
    "Translation3DBatch": TestCase(batch=[(1, 2, 3), (4, 5, 6), (7, 8, 9)]),
    "TriangleIndicesBatch": TestCase(batch=[(0, 1, 2), (3, 4, 5), (6, 7, 8)]),
    "ValueRangeBatch": TestCase((0, 5)),
    "Vector2DBatch": TestCase(batch=[(0, 1), (2, 3), (4, 5)]),
    "Vector3DBatch": TestCase(batch=[(0, 3, 4), (1, 4, 5), (2, 5, 6)]),
    "VideoTimestampBatch": TestCase(rr.components.VideoTimestamp(seconds=0.0)),
    "ViewCoordinatesBatch": TestCase(rr.components.ViewCoordinates.LBD),
    "VisualizerOverridesBatch": TestCase(disabled=True),  # no Python-based serialization
}


def log_readme() -> None:
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), static=True)


def log_some_views() -> None:
    # check that we didn't forget a component
    missing_components = set(c for c in dir(rr.components) if c.endswith("Batch")) - set(ALL_COMPONENTS.keys())
    assert (
        len(missing_components) == 0
    ), f"Some components are missing from the `ALL_COMPONENTS` dictionary: {missing_components}"

    # log all components as len=1 batches
    rr.log(
        "all_components_single",
        [
            getattr(rr.components, component_name)(test_case.single())
            for component_name, test_case in ALL_COMPONENTS.items()
            if not test_case.disabled()
        ],
    )

    # log all components as batches (except those for which it doesn't make sense)
    rr.log(
        "all_components_batches",
        [
            getattr(rr.components, component_name)(test_case.batch())
            for component_name, test_case in ALL_COMPONENTS.items()
            if test_case.batch() is not None
        ],
    )

    # log all alternative values for components
    for component_name, test_case in ALL_COMPONENTS.items():
        alternatives = test_case.alternatives()
        if alternatives is None:
            continue

        for i, alternative in enumerate(alternatives):
            rr.log(
                f"all_components_alternative_{i}",
                [getattr(rr.components, component_name)(alternative)],
            )


def run(args: Namespace) -> None:
    rr.script_setup(args, f"{os.path.basename(__file__)}", recording_id=uuid4())

    log_readme()
    log_some_views()

    rr.send_blueprint(rr.blueprint.Blueprint(auto_layout=True, auto_views=True), make_active=True, make_default=True)


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Interactive release checklist")
    rr.script_add_args(parser)
    args = parser.parse_args()
    run(args)
