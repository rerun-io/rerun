from __future__ import annotations

from typing import Iterable, Optional

from ..datatypes import Utf8Like
from .api import Container, SpaceView
from .components import ColumnShareArrayLike, RowShareArrayLike
from .components.container_kind import ContainerKind


class Horizontal(Container):
    """A horizontal container."""

    def __init__(
        self,
        *args: Container | SpaceView,
        contents: Optional[Iterable[Container | SpaceView]] = None,
        column_shares: Optional[ColumnShareArrayLike] = None,
        name: Utf8Like | None = None,
    ):
        """
        Construct a new horizontal container.

        Parameters
        ----------
        *args:
            All positional arguments are forwarded to the `contents` parameter for convenience.
        contents:
            The contents of the container. Each item in the iterable must be a `SpaceView` or a `Container`.
            This can only be used if no positional arguments are provided.
        column_shares
            The layout shares of the columns in the container. The share is used to determine what fraction of the total width each
            column should take up. The column with index `i` will take up the fraction `shares[i] / total_shares`.
        name
            The name of the container

        """
        super().__init__(
            *args, contents=contents, kind=ContainerKind.Horizontal, column_shares=column_shares, name=name
        )


class Vertical(Container):
    """A vertical container."""

    def __init__(
        self,
        *args: Container | SpaceView,
        contents: Optional[Iterable[Container | SpaceView]] = None,
        row_shares: Optional[RowShareArrayLike] = None,
        name: Utf8Like | None = None,
    ):
        """
        Construct a new vertical container.

        Parameters
        ----------
        *args:
            All positional arguments are forwarded to the `contents` parameter for convenience.
        contents:
            The contents of the container. Each item in the iterable must be a `SpaceView` or a `Container`.
            This can only be used if no positional arguments are provided.
        row_shares
            The layout shares of the rows in the container. The share is used to determine what fraction of the total height each
            row should take up. The row with index `i` will take up the fraction `shares[i] / total_shares`.
        name
            The name of the container

        """
        super().__init__(*args, contents=contents, kind=ContainerKind.Vertical, row_shares=row_shares, name=name)


class Grid(Container):
    """A grid container."""

    def __init__(
        self,
        *args: Container | SpaceView,
        contents: Optional[Iterable[Container | SpaceView]] = None,
        column_shares: Optional[ColumnShareArrayLike] = None,
        row_shares: Optional[RowShareArrayLike] = None,
        grid_columns: Optional[int] = None,
        name: Utf8Like | None = None,
    ):
        """
        Construct a new grid container.

        Parameters
        ----------
        *args:
            All positional arguments are forwarded to the `contents` parameter for convenience.
        contents:
            The contents of the container. Each item in the iterable must be a `SpaceView` or a `Container`.
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

        """
        super().__init__(
            *args,
            contents=contents,
            kind=ContainerKind.Grid,
            column_shares=column_shares,
            row_shares=row_shares,
            grid_columns=grid_columns,
            name=name,
        )


class Tabs(Container):
    """A tab container."""

    def __init__(
        self,
        *args: Container | SpaceView,
        contents: Optional[Iterable[Container | SpaceView]] = None,
        active_tab: Optional[int | str] = None,
        name: Utf8Like | None = None,
    ):
        """
        Construct a new tab container.

        Parameters
        ----------
        *args:
            All positional arguments are forwarded to the `contents` parameter for convenience.
        contents:
            The contents of the container. Each item in the iterable must be a `SpaceView` or a `Container`.
            This can only be used if no positional arguments are provided.
        active_tab:
            The index or name of the active tab.
        name
            The name of the container

        """
        super().__init__(*args, contents=contents, kind=ContainerKind.Tabs, active_tab=active_tab, name=name)
