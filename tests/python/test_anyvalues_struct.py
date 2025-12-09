#!/usr/bin/env python3
"""Test script for AnyValues StructArray tree view in SelectionPanel.

This script creates a nested StructArray similar to the user's example and logs it
via rr.AnyValues. When viewed in the Rerun viewer, clicking on the component should
now show a collapsible tree view instead of static strings.

Usage:
    python tests/python/test_anyvalues_struct.py
    # Then open the resulting .rrd file in the Rerun viewer
"""

import pyarrow as pa

import rerun as rr


def main() -> None:
    rr.init("test_anyvalues_struct", spawn=False)
    rr.save("test_anyvalues_struct.rrd")

    # Create nested struct data similar to the user's example.
    nested_data = [
        {"id": 1, "nested": {"x": 10, "y": 20}, "name": "item1"},
        {"id": 2, "nested": {"x": 30, "y": 40}, "name": "item2"},
        {"id": 3, "nested": {"x": 50, "y": 60}, "name": "item3"},
    ]

    # Build the nested StructArray.
    nested_arrays = [
        pa.array([item["nested"]["x"] for item in nested_data]),
        pa.array([item["nested"]["y"] for item in nested_data]),
    ]
    nested_fields = [
        pa.field("x", pa.int64()),
        pa.field("y", pa.int64()),
    ]
    nested_struct = pa.StructArray.from_arrays(nested_arrays, fields=nested_fields)

    main_arrays = [
        pa.array([item["id"] for item in nested_data]),
        nested_struct,
        pa.array([item["name"] for item in nested_data]),
    ]
    main_fields = [
        pa.field("id", pa.int64()),
        pa.field(
            "nested",
            pa.struct(
                [
                    pa.field("x", pa.int64()),
                    pa.field("y", pa.int64()),
                ]
            ),
        ),
        pa.field("name", pa.string()),
    ]

    struct_array = pa.StructArray.from_arrays(main_arrays, fields=main_fields)

    # Log the StructArray using AnyValues.
    rr.log(
        "test_data",
        rr.AnyValues(
            data=struct_array,
        ),
    )

    print("Saved test_anyvalues_struct.rrd")
    print("Open it with: rerun test_anyvalues_struct.rrd")
    print()
    print("Expected behavior after fix:")
    print("  1. Click on 'test_data' entity in the left panel")
    print("  2. In the SelectionPanel (right side), click on the 'data' component")
    print("  3. You should see a collapsible tree view with expandable entries:")
    print("     - 0: {id: 1, nested: ..., name: 'item1'}")
    print("     - 1: {id: 2, nested: ..., name: 'item2'}")
    print("     - 2: {id: 3, nested: ..., name: 'item3'}")
    print("  4. Each entry should be expandable to show individual fields")


if __name__ == "__main__":
    main()

