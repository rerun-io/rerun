from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    import datafusion
    import pyarrow as pa


def segment_stable_snapshot(df: datafusion.DataFrame) -> str:
    """Create a stable snapshot of a segment DataFrame by sorting and dropping unstable columns."""
    return str(df.drop("rerun_storage_urls", "rerun_last_updated_at").sort("rerun_segment_id"))


def sorted_schema_str(schema: pa.Schema, with_metadata: bool = False) -> str:
    """A version of pa.Schema.__str__ that has stable field / metadata order."""

    # Iterate through every field in order. Print the field name and type,
    # then print its metadata in sorted order.
    lines = []
    for field in sorted(schema, key=lambda f: f.name):
        lines.append(f"{field.name}: {field.type}")
        if with_metadata and field.metadata:
            lines.append("  -- field metadata --")
            for key, value in sorted(field.metadata.items(), key=lambda kv: kv[0]):
                lines.append(f"  {key.decode('utf-8')}: '{value.decode('utf-8')}'")

    # Finally print the top-level schema metadata in sorted order.
    if with_metadata and schema.metadata:
        lines.append("-- schema metadata --")
        for key, value in sorted(schema.metadata.items(), key=lambda kv: kv[0]):
            lines.append(f"{key.decode('utf-8')}: '{value.decode('utf-8')}'")

    return "\n".join(lines)
