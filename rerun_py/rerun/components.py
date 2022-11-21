import pyarrow as pa

RectType = pa.struct(
    [
        pa.field("x", pa.float32(), nullable=False),
        pa.field("y", pa.float32(), nullable=False),
        pa.field("w", pa.float32(), nullable=False),
        pa.field("h", pa.float32(), nullable=False),
    ]
)
RectField = pa.field(
    name="rect",
    type=pa.list_(RectType),
    nullable=True,
    metadata={
        # "ARROW:extension:name": "rerun.rect",
    },
)

Color32Type = pa.uint32()
ColorField = pa.field(
    name="rgbacolor",
    type=pa.list_(Color32Type),
    nullable=False,
    metadata={
        # "ARROW:extension:name": "rerun.rgbacolor",
    },
)

ClearedField = pa.field(
    name="cleared",
    type=pa.bool_(),
    nullable=True,
    metadata={
        # "ARROW:extension:name": "rerun.cleared",
    },
)
