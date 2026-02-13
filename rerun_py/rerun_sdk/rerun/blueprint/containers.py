from __future__ import annotations

from typing import TYPE_CHECKING

from .api import Container, View
from .components.container_kind import ContainerKind

if TYPE_CHECKING:
    from collections.abc import Iterable

    from ..datatypes import BoolLike, Float32ArrayLike, Utf8Like


class Horizontal(Container):
    """A horizontal container."""

    def __init__(
        self,
        *args: Container | View,
        contents: Iterable[Container | View] | None = None,
        column_shares: Float32ArrayLike | None = None,
        name: Utf8Like | None = None,
        visible: BoolLike | None = None,
    ) -> None:
        """
        Construct a new horizontal container.

        Parameters
        ----------
        *args:
            All positional arguments are forwarded to the `contents` parameter for convenience.
        contents:
            The contents of the container. Each item in the iterable must be a `View` or a `Container`.
            This can only be used if no positional arguments are provided.
        column_shares
            The layout shares of the columns in the container. The share is used to determine what fraction of the total width each
            column should take up. The column with index `i` will take up the fraction `shares[i] / total_shares`.
        name
            The name of the container
        visible:
            Whether this container is visible.

            Defaults to true if not specified.

        """
        super().__init__(
            *args,
            contents=contents,
            kind=ContainerKind.Horizontal,
            column_shares=column_shares,
            name=name,
            visible=visible,
        )


class Vertical(Container):
    """A vertical container."""

    def __init__(
        self,
        *args: Container | View,
        contents: Iterable[Container | View] | None = None,
        row_shares: Float32ArrayLike | None = None,
        name: Utf8Like | None = None,
        visible: BoolLike | None = None,
    ) -> None:
        """
        Construct a new vertical container.

        Parameters
        ----------
        *args:
            All positional arguments are forwarded to the `contents` parameter for convenience.
        contents:
            The contents of the container. Each item in the iterable must be a `View` or a `Container`.
            This can only be used if no positional arguments are provided.
        row_shares
            The layout shares of the rows in the container. The share is used to determine what fraction of the total height each
            row should take up. The row with index `i` will take up the fraction `shares[i] / total_shares`.
        name
            The name of the container
        visible:
            Whether this container is visible.

            Defaults to true if not specified.

        """
        super().__init__(
            *args,
            contents=contents,
            kind=ContainerKind.Vertical,
            row_shares=row_shares,
            name=name,
            visible=visible,
        )


class Grid(Container):
    """A grid container."""

    def __init__(
        self,
        *args: Container | View,
        contents: Iterable[Container | View] | None = None,
        column_shares: Float32ArrayLike | None = None,
        row_shares: Float32ArrayLike | None = None,
        grid_columns: int | None = None,
        name: Utf8Like | None = None,
        visible: BoolLike | None = None,
    ) -> None:
        """
        Construct a new grid container.

        Parameters
        ----------
        *args:
            All positional arguments are forwarded to the `contents` parameter for convenience.
        contents:
            The contents of the container. Each item in the iterable must be a `View` or a `Container`.
            This can only be used if no positional arguments are provided.
        column_shares
            The layout shares of the columns in the container. The share is used to determine what fraction of the total width each
            column should take up. The column with index `i` will take up the fraction `shares[i] / total_shares`.
        row_shares
            The layout shares of the rows in the container. The share is used to determine what fraction of the total height each
            row should take up. The row with index `i` will take up the fraction `shares[i] / total_shares`.
        grid_columns
            The number of columns in the grid.
        name
            The name of the container
        visible:
            Whether this container is visible.

            Defaults to true if not specified.

        """
        super().__init__(
            *args,
            contents=contents,
            kind=ContainerKind.Grid,
            column_shares=column_shares,
            row_shares=row_shares,
            grid_columns=grid_columns,
            name=name,
            visible=visible,
        )


class Tabs(Container):
    """A tab container."""

    def __init__(
        self,
        *args: Container | View,
        contents: Iterable[Container | View] | None = None,
        active_tab: int | str | None = None,
        name: Utf8Like | None = None,
        visible: BoolLike | None = None,
    ) -> None:
        """
        Construct a new tab container.

        Parameters
        ----------
        *args:
            All positional arguments are forwarded to the `contents` parameter for convenience.
        contents:
            The contents of the container. Each item in the iterable must be a `View` or a `Container`.
            This can only be used if no positional arguments are provided.
        active_tab:
            The index or name of the active tab.
        name
            The name of the container
        visible:
            Whether this container is visible.

            Defaults to true if not specified.

        """
        super().__init__(
            *args,
            contents=contents,
            kind=ContainerKind.Tabs,
            active_tab=active_tab,
            name=name,
            visible=visible,
        )
