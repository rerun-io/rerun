from __future__ import annotations

import numpy as np
import pyarrow as pa
import rerun as rr


def test_archetype_builder() -> None:
    # Archetype builder and any_values share common conversion so variants are only checked for any values
    archetype = "new_custom_type"
    values = rr.DynamicArchetype(archetype=archetype, components={"foo": [1.0, 2.0, 3.0], "bar": "hello"})
    values.with_component_override("baz", rr.components.ScalarBatch._COMPONENT_TYPE, [1.2, 3.4, 5.6])

    batches = list(values.as_component_batches())

    foo_batch = batches[0]
    bar_batch = batches[1]
    baz_batch = batches[2]

    assert foo_batch.component_descriptor() == rr.ComponentDescriptor("foo").with_builtin_archetype(archetype)
    assert bar_batch.component_descriptor() == rr.ComponentDescriptor("bar").with_builtin_archetype(archetype)
    assert baz_batch.component_descriptor() == rr.ComponentDescriptor(
        "baz", component_type=rr.components.ScalarBatch._COMPONENT_TYPE
    ).with_builtin_archetype(archetype)
    assert len(foo_batch.as_arrow_array()) == 3
    assert len(bar_batch.as_arrow_array()) == 1
    assert len(baz_batch.as_arrow_array()) == 3
    assert np.all(foo_batch.as_arrow_array().to_numpy() == np.array([1.0, 2.0, 3.0]))


def test_dynamic_archetype_columns_scalar() -> None:
    cols = rr.DynamicArchetype.columns(
        archetype="test_columns_scalar",
        components={"scalars": [1.0, 2.0, 3.0]},
    )
    column_list = list(cols)
    assert len(column_list) == 1

    arrow = column_list[0].as_arrow_array()

    # 3 rows, each containing a single scalar
    assert len(arrow) == 3
    assert pa.types.is_floating(arrow.type.value_type)
    for i, expected in enumerate([1.0, 2.0, 3.0]):
        assert arrow[i].as_py() == [expected]


def test_dynamic_archetype_columns_list_of_lists() -> None:
    cols = rr.DynamicArchetype.columns(
        archetype="test_columns_lol",
        components={"arrays": [[1, 2, 3], [4, 5], [6]]},
    )
    column_list = list(cols)
    assert len(column_list) == 1

    arrow = column_list[0].as_arrow_array()

    # 3 rows with variable-length partitions
    assert len(arrow) == 3
    assert arrow[0].as_py() == [1, 2, 3]
    assert arrow[1].as_py() == [4, 5]
    assert arrow[2].as_py() == [6]

    # The element type should be int64, NOT list<int64>
    assert pa.types.is_integer(arrow.type.value_type)
