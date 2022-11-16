from importlib.metadata import metadata
from typing import Iterable, Optional, Sequence
import pyarrow as pa
import pyarrow.flight as pf

import numpy as np
import numpy.typing as npt

from rerun_sdk.color_conversion import u8_array_to_rgba

Point2D = pa.struct([("x", pa.float32()), ("y", pa.float32())])
Point3D = pa.struct([("x", pa.float32()), ("y", pa.float32()), ("z", pa.float32())])
Quaternion = pa.list_(pa.field("quaternion", pa.float32(), nullable=False), 4)


Vec3Type = pa.list_(pa.field("item", pa.float32(), nullable=False))
ColorType = pa.struct([("r", pa.uint8()), ("g", pa.uint8()), ("b", pa.uint8()), ("a", pa.uint8())])


class BBox2DType(pa.ExtensionType):
    def __init__(self) -> None:
        inner = pa.struct(
            [
                ("min", pa.list_(pa.float32(), 2)),
                ("max", pa.list_(pa.float32(), 2)),
            ]
        )
        pa.ExtensionType.__init__(self, inner, "rerun.bbox2d")

    def __arrow_ext_serialize__(self) -> bytes:
        # since we don't have a parameterized type, we don't need extra
        # metadata to be deserialized
        return b""

    @classmethod
    def __arrow_ext_deserialize__(cls, storage_type, serialized) -> "BBox2DType":
        # return an instance of this subclass given the serialized
        # metadata.
        return cls()


pa.register_extension_type(BBox2DType())


class CameraType(pa.ExtensionType):
    CameraSpaceConvention = pa.dictionary(index_type=pa.uint8(), value_type=pa.string())

    Extrinsics = pa.struct(
        [
            pa.field("rotation", type=Quaternion),
            pa.field("position", type=Point3D),
            pa.field("camera_space_convention", type=CameraSpaceConvention),
        ]
    )

    def __init__(self) -> None:
        inner = pa.struct(
            [
                pa.field("extrinsics", type=self.Extrinsics, nullable=True),
            ]
        )
        super().__init__(inner, "rerun.camera")

    def __arrow_ext_serialize__(self) -> bytes:
        return b""

    @classmethod
    def __arrow_ext_deserialize__(cls, storage_type, serialized) -> "CameraType":
        return cls()


pa.register_extension_type(CameraType())


class Points2DType(pa.ExtensionType):
    def __init__(self) -> None:
        inner = pa.list_(pa.list_(pa.field("point2d", type=Point2D)))
        super().__init__(inner, "rerun.points2d")

    def __arrow_ext_serialize__(self) -> bytes:
        return b""

    @classmethod
    def __arrow_ext_deserialize__(cls, storage_type, serialized) -> "Points2DType":
        return cls()


pa.register_extension_type(Points2DType())


class Points3DType(pa.ExtensionType):
    def __init__(self) -> None:
        inner = pa.list_(pa.field("point3d", type=Point3D))
        super().__init__(inner, "rerun.points3d")

    def __arrow_ext_serialize__(self) -> bytes:
        return b""

    @classmethod
    def __arrow_ext_deserialize__(cls, storage_type, serialized) -> "Points3DType":
        return cls()


pa.register_extension_type(Points3DType())


class ColorsType(pa.ExtensionType):
    def __init__(self) -> None:
        inner = pa.list_(pa.field("colors", type=ColorType))
        super().__init__(inner, "rerun.colors")

    def __arrow_ext_serialize__(self) -> bytes:
        return b""

    @classmethod
    def __arrow_ext_deserialize__(cls, storage_type, serialized) -> "PointColorsType":
        return cls()


# class ColorType(pa.ExtensionType):
#    def __init__(self) -> None:
#        super().__init__(pa.list_(pa.uint8(), 4), "rerun.color")
#
#    def __arrow_ext_serialize__(self) -> bytes:
#        return b""
#
#    @classmethod
#    def __arrow_ext_deserialize__(cls, storage_type, serialized) -> "ColorType":
#        return cls()

# pa.register_extension_type(ColorType())


client = pf.FlightClient("grpc://localhost:9877")


def connect() -> None:
    global client
    client.connect(("127.0.0.1", 9877))
    client.wait_for_available()


def disconnect() -> None:
    client.do_action("shutdown")


def log_arrow(
    obj_path: str,
    origin: npt.ArrayLike,
    vector: npt.ArrayLike,
    color: Optional[Sequence[int]] = None,
    label: Optional[str] = None,
    width_scale: Optional[float] = None,
    timeless: bool = False,
    space: Optional[str] = None,
) -> None:
    import datetime

    TimeType = pa.timestamp("ns", tz="Europe/Berlin")

    time_array = pa.array([datetime.datetime.now()], type=TimeType)
    origin_array = pa.array([origin], type=Vec3Type)
    vector_array = pa.array([vector], type=Vec3Type)
    colors_array = pa.array([tuple(color) if color else None], type=ColorType)
    label_array = pa.array([label], type=pa.string())
    width_scale_array = pa.array([width_scale], type=pa.float32())

    schema = pa.schema(
        [
            pa.field("time", TimeType),
            pa.field("origin", Vec3Type, metadata={"ARROW:extension:name": "rerun.vec3"}),
            pa.field("vector", Vec3Type, metadata={"ARROW:extension:name": "rerun.vec3"}),
            pa.field("color", ColorType, metadata={"ARROW:extension:name": "rerun.rgbacolor"}),
            pa.field("label", label_array.type),
            pa.field("width_scale", width_scale_array.type),
        ],
        metadata={"ARROW:extension:name": "rerun.3darrow"},
    )

    batch = pa.record_batch(
        [
            time_array,
            origin_array,
            vector_array,
            colors_array,
            label_array,
            width_scale_array,
        ],
        schema=schema,
    )

    global client

    desc = pf.FlightDescriptor.for_path(obj_path)
    (writer, reader) = client.do_put(desc, batch.schema)
    writer.write_batch(batch)
    writer.done_writing()

    print("resp:", reader.read())


def log_points(
    obj_path: str,
    positions: np.ndarray,
    colors: npt.NDArray[np.uint8],
    timeless: bool = False,
    space: Optional[str] = None,
) -> None:
    if positions.shape[1] == 2:
        points_type = Points2DType()
    elif positions.shape[1] == 3:
        points_type = Points3DType()
    else:
        raise RuntimeError("Should be dim 2 or 3")

    if len(colors.shape) == 1:
        colors = colors.reshape(1, colors.shape[0])

    points_array = pa.ExtensionArray.from_storage(
        typ=points_type,
        storage=pa.array(
            [[tuple(p) for p in positions]],
            points_type.storage_type,
        ),
    )

    colors_array = pa.ExtensionArray.from_storage(
        typ=ColorsType(),
        storage=pa.array([[tuple(c) for c in colors]], ColorsType().storage_type),
    )

    space_array = pa.array([space], type=pa.string())

    batch = pa.record_batch([points_array, colors_array, space_array], names=["points", "colors", "space"])

    global client

    desc = pf.FlightDescriptor.for_path(obj_path)
    (writer, reader) = client.do_put(desc, batch.schema)
    writer.write_batch(batch)
    writer.done_writing()

    print("resp:", reader.read())


def log_rects(
    obj_path: str,
    rect_format: str,
    rects: npt.ArrayLike,
    colors: npt.NDArray[np.uint8],
    labels: Iterable[str],
    timeless: bool,
    space: Optional[str],
):
    rects = np.require(rects, dtype="float32")
    rects_reshaped = [{"min": x[:2], "max": x[2:]} for x in rects]

    bbox_array = pa.ExtensionArray.from_storage(
        typ=BBox2DType(),
        storage=pa.array(rects_reshaped, BBox2DType().storage_type),
    )

    colors_array = pa.ExtensionArray.from_storage(
        typ=ColorType(),
        storage=pa.array(colors.tolist(), ColorType().storage_type),
    )

    labels_array = pa.array(labels)

    batch = pa.record_batch([bbox_array, colors_array, labels_array], names=["bbox", "color", "label"])

    print(batch.to_pandas())
