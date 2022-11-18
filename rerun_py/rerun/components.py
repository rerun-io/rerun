import pyarrow as pa

RectType = pa.struct([("x", pa.float32()), ("y", pa.float32()), ("w", pa.float32()), ("h", pa.float32())])
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
