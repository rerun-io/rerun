from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from collections.abc import Callable

    import pyarrow as pa

from rerun_bindings import SelectorInternal


class Selector:
    """
    A jq-like query selector for Arrow arrays.

    Selectors provide a path-based query language (inspired by jq) that operates
    on Arrow arrays in a columnar fashion.

    Syntax overview:

    - `.field` — access a named field in a struct
    - `[]` — iterate over every element of a list
    - `[N]` — index into a list by position
    - `?` — error suppression / optional operator
    - `!` — assert non-null
    - `|` — pipe the output of one expression to another

    Example usage::

        selector = Selector(".location")
        result = selector.execute(my_struct_array)

    Selectors can also be piped into Python functions::

        selector = Selector(".values").pipe(lambda arr: pa.compute.multiply(arr, 2))
        result = selector.execute(my_struct_array)

    """

    _internal: SelectorInternal

    def __init__(self, query: str) -> None:
        """
        Parse a selector from a query string.

        Parameters
        ----------
        query:
            The selector query string (e.g. ".field", ".foo | .bar").

        """
        self._internal = SelectorInternal(query)

    def execute(self, source: pa.Array) -> pa.Array | None:
        """
        Execute this selector against a pyarrow array.

        Parameters
        ----------
        source:
            The input Arrow array to query.

        Returns
        -------
        The result array, or None if the selector's error was suppressed.

        """
        return self._internal.execute(source)

    def execute_per_row(self, source: pa.Array) -> pa.Array | None:
        """
        Execute this selector against each row of a pyarrow list array.

        The output is guaranteed to have the same number of rows as the input.

        Parameters
        ----------
        source:
            The input Arrow list array to query.

        Returns
        -------
        The result list array, or None if the selector's error was suppressed.

        """
        return self._internal.execute_per_row(source)

    def pipe(self, func: Callable[[pa.Array], pa.Array | None] | Selector) -> Selector:
        """
        Pipe the output of this selector through a transformation function or another selector.

        Returns a new selector; the original is not modified.

        Parameters
        ----------
        func:
            A callable that accepts a `pyarrow.Array` and returns a `pyarrow.Array`
            or `None`, or another [`Selector`][] to chain.

        Returns
        -------
        A new [`Selector`][] with the transformation applied.

        """
        new = Selector.__new__(Selector)
        if isinstance(func, Selector):
            new._internal = self._internal.pipe(func._internal)
        else:
            new._internal = self._internal.pipe(func)
        return new

    def __repr__(self) -> str:
        return repr(self._internal)

    def __str__(self) -> str:
        return str(self._internal)
