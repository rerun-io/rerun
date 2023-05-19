from __future__ import annotations

from typing import Any, Sequence

import numpy as np
import numpy.typing as npt
import pyarrow as pa

from depthai_viewer.components import REGISTERED_COMPONENT_NAMES, ComponentTypeFactory

__all__ = [
    "ScalarArray",
    "ScalarType",
    "ScalarPlotPropsArray",
    "ScalarPlotPropsType",
]

# ---


class ScalarArray(pa.ExtensionArray):  # type: ignore[misc]
    def from_numpy(array: npt.NDArray[np.float64]) -> ScalarArray:
        """Build a `ScalarArray` from an numpy array."""
        storage = pa.array(array, type=ScalarType.storage_type)
        # TODO(john) enable extension type wrapper
        # return cast(ScalarArray, pa.ExtensionArray.from_storage(ScalarType(), storage))
        return storage  # type: ignore[no-any-return]


ScalarType = ComponentTypeFactory("ScalarType", ScalarArray, REGISTERED_COMPONENT_NAMES["rerun.scalar"])

pa.register_extension_type(ScalarType())

# ---


class ScalarPlotPropsArray(pa.ExtensionArray):  # type: ignore[misc]
    def from_props(props: Sequence[dict[str, Any]]) -> ScalarPlotPropsArray:
        """Build a `ScalarPlotPropsArray` from an numpy array."""
        storage = pa.array(props, type=ScalarPlotPropsType.storage_type)
        # TODO(john) enable extension type wrapper
        # return cast(ScalarPlotPropsArray, pa.ExtensionArray.from_storage(ScalarPlotPropsType(), storage))
        return storage  # type: ignore[no-any-return]


ScalarPlotPropsType = ComponentTypeFactory(
    "ScalarPlotPropsType", ScalarPlotPropsArray, REGISTERED_COMPONENT_NAMES["rerun.scalar_plot_props"]
)

pa.register_extension_type(ScalarPlotPropsType())
