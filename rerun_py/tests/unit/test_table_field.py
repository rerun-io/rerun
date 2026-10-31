from __future__ import annotations

import pyarrow as pa
import rerun as rr


def test_table_field_has_component_type_and_datatype() -> None:
    field = rr.components.Color.table_field("color")

    assert field.name == "color"
    assert field.type == pa.uint32()
    assert field.nullable
    assert field.metadata == {rr.SORBET_COMPONENT_TYPE: b"rerun.components.Color"}


def test_table_field_editable() -> None:
    field = rr.components.Flag.table_field("reviewed", editable=True, nullable=False)

    assert field.type == pa.bool_()
    assert not field.nullable
    assert field.metadata == {
        rr.SORBET_COMPONENT_TYPE: b"rerun.components.Flag",
        rr.SORBET_IS_EDITABLE: b"true",
    }


def test_table_fields_build_a_schema() -> None:
    schema = pa.schema([
        pa.field("id", pa.int64(), metadata={rr.SORBET_IS_TABLE_INDEX: "true"}),
        rr.components.Url.table_field("link"),
        rr.components.Text.table_field("notes", editable=True),
    ])

    assert schema.field("link").type == pa.utf8()
    assert schema.field("notes").metadata[rr.SORBET_IS_EDITABLE] == b"true"
