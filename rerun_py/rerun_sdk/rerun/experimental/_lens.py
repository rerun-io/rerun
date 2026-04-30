from __future__ import annotations

from typing import Literal, TypeAlias

from rerun._baseclasses import ComponentDescriptor
from rerun_bindings import DeriveLensInternal, MutateLensInternal

from ._selector import Selector


class DeriveLens:
    """
    A derive lens that creates new component/time columns from an input component.

    Derive lenses extract fields from a component and produce new columns,
    optionally at a different entity and/or with new time columns.

    Pass `scatter=True` to enable 1:N row mapping (exploding lists).

    Example usage::

        lens = (
            DeriveLens("Imu:accel")
            .to_component(rr.Scalars.descriptor_scalars(), Selector(".x"))
        )

    To write to an explicit target entity::

        lens = (
            DeriveLens("Imu:accel", output_entity="/out/x")
            .to_component(rr.Scalars.descriptor_scalars(), Selector(".x"))
        )

    """

    _internal: DeriveLensInternal

    def __init__(
        self,
        input_component: str,
        *,
        output_entity: str | None = None,
        scatter: bool = False,
    ) -> None:
        """
        Create a new derive lens.

        Parameters
        ----------
        input_component:
            The component identifier to match (e.g. `"Imu:accel"`).
        output_entity:
            Optional target entity path. When set, output is written
            to this entity instead of the input entity.
        scatter:
            When `True`, use 1:N row mapping (explode lists).

        """
        self._internal = DeriveLensInternal(
            input_component,
            output_entity=output_entity,
            scatter=scatter,
        )

    def to_component(
        self,
        component: ComponentDescriptor | str,
        selector: Selector | str,
    ) -> DeriveLens:
        """
        Add a component output column.

        Parameters
        ----------
        component:
            A `ComponentDescriptor` or a component identifier string
            for the output column (e.g. `"Scalars:scalars"`).
        selector:
            A [`Selector`][rerun.experimental.Selector] or selector query string to apply to the
            input column.

        Returns
        -------
        A new [`DeriveLens`][rerun.experimental.DeriveLens] with the component added.

        """
        sel = _normalize_selector(selector)
        new = DeriveLens.__new__(DeriveLens)
        if isinstance(component, str):
            component = ComponentDescriptor(component)
        new._internal = self._internal.to_component(component, sel._internal)
        return new

    def to_timeline(
        self,
        timeline_name: str,
        timeline_type: Literal["sequence", "duration_ns", "timestamp_ns"],
        selector: Selector | str,
    ) -> DeriveLens:
        """
        Add a time extraction column.

        Parameters
        ----------
        timeline_name:
            Name of the timeline to create.
        timeline_type:
            Type of the timeline: `"sequence"`, `"duration_ns"`,
            or `"timestamp_ns"`.
        selector:
            A [`Selector`][rerun.experimental.Selector] or selector query string to extract time
            values (must produce `Int64` arrays).

        Returns
        -------
        A new [`DeriveLens`][rerun.experimental.DeriveLens] with the time column added.

        """
        sel = _normalize_selector(selector)
        new = DeriveLens.__new__(DeriveLens)
        new._internal = self._internal.to_timeline(timeline_name, timeline_type, sel._internal)
        return new


class MutateLens:
    """
    A mutate lens that modifies the input component in-place.

    Mutate lenses apply a selector transformation to the input component,
    replacing it in the chunk. By default, new row IDs are generated.
    Pass `keep_row_ids=True` to preserve original row IDs.

    Example usage::

        lens = MutateLens("Imu:accel", Selector(".x"))

    """

    _internal: MutateLensInternal

    def __init__(
        self,
        input_component: str,
        selector: Selector | str,
        *,
        keep_row_ids: bool = False,
    ) -> None:
        """
        Create a new mutate lens.

        Parameters
        ----------
        input_component:
            The component identifier to modify in-place.
        selector:
            A [`Selector`][rerun.experimental.Selector] or selector query string to apply.
        keep_row_ids:
            When `True`, preserve the original row IDs.

        """
        sel = _normalize_selector(selector)
        self._internal = MutateLensInternal(
            input_component,
            sel._internal,
            keep_row_ids=keep_row_ids,
        )


Lens: TypeAlias = DeriveLens | MutateLens
"""Union of all lens types."""


def _normalize_selector(selector: Selector | str) -> Selector:
    """Normalize a selector argument to a Selector object."""
    if isinstance(selector, str):
        return Selector(selector)
    return selector
