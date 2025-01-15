# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/archetypes/series_line.fbs".

# You can extend this class by creating a "SeriesLineExt" class in "series_line_ext.py".

from __future__ import annotations

from typing import Any

from attrs import define, field

from .. import components, datatypes
from .._baseclasses import (
    Archetype,
)
from ..error_utils import catch_and_log_exceptions

__all__ = ["SeriesLine"]


@define(str=False, repr=False, init=False)
class SeriesLine(Archetype):
    """
    **Archetype**: Define the style properties for a line series in a chart.

    This archetype only provides styling information and should be logged as static
    when possible. The underlying data needs to be logged to the same entity-path using
    [`archetypes.Scalar`][rerun.archetypes.Scalar].

    Example
    -------
    ### Line series:
    ```python
    from math import cos, sin, tau

    import rerun as rr

    rr.init("rerun_example_series_line_style", spawn=True)

    # Set up plot styling:
    # They are logged as static as they don't change over time and apply to all timelines.
    # Log two lines series under a shared root so that they show in the same plot by default.
    rr.log("trig/sin", rr.SeriesLine(color=[255, 0, 0], name="sin(0.01t)", width=2), static=True)
    rr.log("trig/cos", rr.SeriesLine(color=[0, 255, 0], name="cos(0.01t)", width=4), static=True)

    # Log the data on a timeline called "step".
    for t in range(0, int(tau * 2 * 100.0)):
        rr.set_time_sequence("step", t)

        rr.log("trig/sin", rr.Scalar(sin(float(t) / 100.0)))
        rr.log("trig/cos", rr.Scalar(cos(float(t) / 100.0)))
    ```
    <center>
    <picture>
      <source media="(max-width: 480px)" srcset="https://static.rerun.io/series_line_style/d2616d98b1e46bdb85849b8669154fdf058e3453/480w.png">
      <source media="(max-width: 768px)" srcset="https://static.rerun.io/series_line_style/d2616d98b1e46bdb85849b8669154fdf058e3453/768w.png">
      <source media="(max-width: 1024px)" srcset="https://static.rerun.io/series_line_style/d2616d98b1e46bdb85849b8669154fdf058e3453/1024w.png">
      <source media="(max-width: 1200px)" srcset="https://static.rerun.io/series_line_style/d2616d98b1e46bdb85849b8669154fdf058e3453/1200w.png">
      <img src="https://static.rerun.io/series_line_style/d2616d98b1e46bdb85849b8669154fdf058e3453/full.png" width="640">
    </picture>
    </center>

    """

    def __init__(
        self: Any,
        *,
        color: datatypes.Rgba32Like | None = None,
        width: datatypes.Float32Like | None = None,
        name: datatypes.Utf8Like | None = None,
        aggregation_policy: components.AggregationPolicyLike | None = None,
    ):
        """
        Create a new instance of the SeriesLine archetype.

        Parameters
        ----------
        color:
            Color for the corresponding series.
        width:
            Stroke width for the corresponding series.
        name:
            Display name of the series.

            Used in the legend.
        aggregation_policy:
            Configures the zoom-dependent scalar aggregation.

            This is done only if steps on the X axis go below a single pixel,
            i.e. a single pixel covers more than one tick worth of data. It can greatly improve performance
            (and readability) in such situations as it prevents overdraw.

        """

        # You can define your own __init__ function as a member of SeriesLineExt in series_line_ext.py
        with catch_and_log_exceptions(context=self.__class__.__name__):
            self.__attrs_init__(color=color, width=width, name=name, aggregation_policy=aggregation_policy)
            return
        self.__attrs_clear__()

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            color=None,  # type: ignore[arg-type]
            width=None,  # type: ignore[arg-type]
            name=None,  # type: ignore[arg-type]
            aggregation_policy=None,  # type: ignore[arg-type]
        )

    @classmethod
    def _clear(cls) -> SeriesLine:
        """Produce an empty SeriesLine, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    @classmethod
    def update_fields(
        cls,
        *,
        clear: bool = False,
        color: datatypes.Rgba32Like | None = None,
        width: datatypes.Float32Like | None = None,
        name: datatypes.Utf8Like | None = None,
        aggregation_policy: components.AggregationPolicyLike | None = None,
    ) -> SeriesLine:
        """
        Update only some specific fields of a `SeriesLine`.

        Parameters
        ----------
        clear:
            If true, all unspecified fields will be explicitly cleared.
        color:
            Color for the corresponding series.
        width:
            Stroke width for the corresponding series.
        name:
            Display name of the series.

            Used in the legend.
        aggregation_policy:
            Configures the zoom-dependent scalar aggregation.

            This is done only if steps on the X axis go below a single pixel,
            i.e. a single pixel covers more than one tick worth of data. It can greatly improve performance
            (and readability) in such situations as it prevents overdraw.

        """

        kwargs = {
            "color": color,
            "width": width,
            "name": name,
            "aggregation_policy": aggregation_policy,
        }

        if clear:
            kwargs = {k: v if v is not None else [] for k, v in kwargs.items()}  # type: ignore[misc]

        return SeriesLine(**kwargs)  # type: ignore[arg-type]

    @classmethod
    def clear_fields(cls) -> SeriesLine:
        """Clear all the fields of a `SeriesLine`."""
        inst = cls.__new__(cls)
        inst.__attrs_init__(
            color=[],
            width=[],
            name=[],
            aggregation_policy=[],
        )
        return inst

    color: components.ColorBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.ColorBatch._optional,  # type: ignore[misc]
    )
    # Color for the corresponding series.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    width: components.StrokeWidthBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.StrokeWidthBatch._optional,  # type: ignore[misc]
    )
    # Stroke width for the corresponding series.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    name: components.NameBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.NameBatch._optional,  # type: ignore[misc]
    )
    # Display name of the series.
    #
    # Used in the legend.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    aggregation_policy: components.AggregationPolicyBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.AggregationPolicyBatch._optional,  # type: ignore[misc]
    )
    # Configures the zoom-dependent scalar aggregation.
    #
    # This is done only if steps on the X axis go below a single pixel,
    # i.e. a single pixel covers more than one tick worth of data. It can greatly improve performance
    # (and readability) in such situations as it prevents overdraw.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
