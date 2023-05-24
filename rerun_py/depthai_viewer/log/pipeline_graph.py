# rerun.pipeline_graph

from typing import Any, Dict, Optional, Sequence

import numpy as np

from depthai_viewer import bindings
from depthai_viewer.components.color import ColorRGBAArray
from depthai_viewer.components.instance import InstanceArray
from depthai_viewer.components.label import LabelArray
from depthai_viewer.components.radius import RadiusArray
from depthai_viewer.components.scalar import ScalarArray, ScalarPlotPropsArray
from depthai_viewer.log import _normalize_colors
from depthai_viewer.log.extension_components import _add_extension_components
from depthai_viewer.log.log_decorator import log_decorator

__all__ = [
    "log_pipeline_graph",
]


@log_decorator
def log_pipeline_graph(
    entity_path: str,
    scalar: float,
    label: Optional[str] = None,
    color: Optional[Sequence[int]] = None,
    radius: Optional[float] = None,
    scattered: Optional[bool] = None,
    ext: Optional[Dict[str, Any]] = None,
) -> None:
    instanced: Dict[str, Any] = {}
    splats: Dict[str, Any] = {}

    instanced["rerun.pipeline_graph"] = ScalarArray.from_numpy(np.array([scalar]))

    if label:
        instanced["rerun.label"] = LabelArray.new([label])

    if color:
        colors = _normalize_colors(np.array([color]))
        instanced["rerun.colorrgba"] = ColorRGBAArray.from_numpy(colors)

    if radius:
        instanced["rerun.radius"] = RadiusArray.from_numpy(np.array([radius]))

    if scattered:
        props = [{"scattered": scattered}]
        instanced["rerun.scalar_plot_props"] = ScalarPlotPropsArray.from_props(props)

    if ext:
        _add_extension_components(instanced, splats, ext, None)

    if splats:
        splats["rerun.instance_key"] = InstanceArray.splat()
        bindings.log_arrow_msg(entity_path, components=splats, timeless=False)

    # Always the primary component last so range-based queries will include the other data. See(#1215)
    if instanced:
        bindings.log_arrow_msg(entity_path, components=instanced, timeless=False)
