from __future__ import annotations

import numpy as np
import rerun as rr


def test_archetype_builder() -> None:
    # Archetype builder and any_values share common conversion so variants are only checked for any values
    archetype = "new_custom_type"
    values = rr.ArchetypeBuilder(archetype=archetype, components={"foo": [1.0, 2.0, 3.0], "bar": "hello"})

    batches = list(values.as_component_batches())

    foo_batch = batches[0]
    bar_batch = batches[1]

    assert foo_batch.component_descriptor() == rr.ComponentDescriptor("foo", archetype=archetype)
    assert bar_batch.component_descriptor() == rr.ComponentDescriptor("bar", archetype=archetype)
    assert len(foo_batch.as_arrow_array()) == 3
    assert len(bar_batch.as_arrow_array()) == 1
    assert np.all(foo_batch.as_arrow_array().to_numpy() == np.array([1.0, 2.0, 3.0]))
