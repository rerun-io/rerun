from __future__ import annotations

from typing import TYPE_CHECKING, Literal

from rerun._baseclasses import ComponentDescriptor
from rerun_bindings import LensInternal, LensOutputInternal

from ._selector import Selector

if TYPE_CHECKING:
    from collections.abc import Sequence


class LensOutput:
    """
    Describes one output group of a lens.

    When `scatter=False` (default, 1:1), each input row produces exactly one
    output row. Times are inherited from the input chunk unchanged.

    When `scatter=True` (1:N), each input row produces N output rows by
    exploding list arrays. Existing times are replicated across output rows.
    Useful for flattening lists or exploding batches.

    In both modes, `.component()` and `.time()` work identically -- the
    difference is only in how the output chunk is assembled.

    Example usage::

        output = (
            LensOutput()
            .component("rerun.components.TextDocument:text", Selector("."))
        )

    """

    _internal: LensOutputInternal

    def __init__(
        self,
        *,
        scatter: bool = False,
        target_entity: str | None = None,
    ) -> None:
        """
        Create a new output group.

        Parameters
        ----------
        scatter:
            If `True`, use 1:N row mapping (explode lists). If `False`
            (default), use 1:1 row mapping.
        target_entity:
            Target entity path for the output. If `None`, uses the same
            entity path as the input.

        """
        self._internal = LensOutputInternal(scatter=scatter, target_entity=target_entity)

    def component(
        self,
        component: ComponentDescriptor | str,
        selector: Selector | str,
    ) -> LensOutput:
        """
        Add a component output column.

        Parameters
        ----------
        component:
            A [`ComponentDescriptor`][] or a component identifier string
            for the output column (e.g. `"Scalars:scalars"`).
            Using a full `ComponentDescriptor` preserves archetype and
            component type metadata in the output.
        selector:
            A [`Selector`][] or selector query string to apply to the
            input column.

        Returns
        -------
        A new [`LensOutput`][] with the component added.

        """
        sel = _normalize_selector(selector)
        new = LensOutput.__new__(LensOutput)
        if isinstance(component, str):
            component = ComponentDescriptor(component)
        new._internal = self._internal.component(component, sel._internal)
        return new

    def time(
        self,
        timeline_name: str,
        timeline_type: Literal["sequence", "duration_ns", "timestamp_ns"],
        selector: Selector | str,
    ) -> LensOutput:
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
            A [`Selector`][] or selector query string to extract time
            values (must produce `Int64` arrays).

        Returns
        -------
        A new [`LensOutput`][] with the time column added.

        """
        sel = _normalize_selector(selector)
        new = LensOutput.__new__(LensOutput)
        new._internal = self._internal.time(timeline_name, timeline_type, sel._internal)
        return new


class Lens:
    """
    A lens that transforms component data from one form to another.

    Lenses extract, transform, and restructure component data. They are
    applied to chunks whose entity path matches the content filter and
    that contain the specified input component.

    Example usage::

        lens = Lens(
            "example:Instruction:text",
            [
                LensOutput()
                .component("rerun.components.TextDocument:text", Selector("."))
            ],
        )

    To restrict which entities a lens applies to, use
    `stream.filter(content=...)` before `.lenses()`.

    """

    _internal: LensInternal

    def __init__(
        self,
        input_component: str,
        outputs: Sequence[LensOutput] | LensOutput,
    ) -> None:
        """
        Create a new lens.

        Parameters
        ----------
        input_component:
            The component identifier to match in input chunks.
        outputs:
            One or more [`LensOutput`][] objects describing the
            output transformations.

        """
        if isinstance(outputs, LensOutput):
            outputs = [outputs]

        output_internals = [o._internal for o in outputs]
        self._internal = LensInternal(input_component, outputs=output_internals)


def _normalize_selector(selector: Selector | str) -> Selector:
    """Normalize a selector argument to a Selector object."""
    if isinstance(selector, str):
        return Selector(selector)
    return selector
