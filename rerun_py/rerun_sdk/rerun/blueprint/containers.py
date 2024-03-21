from __future__ import annotations

from typing import Optional

from ..datatypes import Utf8Like
from .api import Container, SpaceView
from .components import ColumnShareArrayLike, RowShareArrayLike
from .components.container_kind import ContainerKind


class Horizontal(Container):
    """A horizontal container."""

    def __init__(
        self,
        *contents: Container | SpaceView,
        column_shares: Optional[ColumnShareArrayLike] = None,
        name: Utf8Like | None = None,
    ):
        """
        Construct a new horizontal container.

        Parameters
        ----------
        *contents:
            All positional arguments are the contents of the container, which may be either other containers or space views.
        column_shares
            The layout shares of the columns in the container. The share is used to determine what fraction of the total width each
            column should take up. The column with index `i` will take up the fraction `shares[i] / total_shares`.
        name
            The name of the container

        """
        super().__init__(*contents, kind=ContainerKind.Horizontal, column_shares=column_shares, name=name)


class Vertical(Container):
    """A vertical container."""

    def __init__(
        self,
        *contents: Container | SpaceView,
        row_shares: Optional[RowShareArrayLike] = None,
        name: Utf8Like | None = None,
    ):
        """
        Construct a new vertical container.

        Parameters
        ----------
        *contents:
            All positional arguments are the contents of the container, which may be either other containers or space views.
        row_shares
            The layout shares of the rows in the container. The share is used to determine what fraction of the total height each
            row should take up. The row with index `i` will take up the fraction `shares[i] / total_shares`.
        name
            The name of the container

        """
        super().__init__(*contents, kind=ContainerKind.Vertical, row_shares=row_shares, name=name)


class Grid(Container):
    """A grid container."""

    def __init__(
        self,
        *contents: Container | SpaceView,
        column_shares: Optional[ColumnShareArrayLike] = None,
        row_shares: Optional[RowShareArrayLike] = None,
        grid_columns: Optional[int] = None,
        name: Utf8Like | None = None,
    ):
        """
        Construct a new grid container.

        Parameters
        ----------
        *contents:
            All positional arguments are the contents of the container, which may be either other containers or space views.
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
            *contents,
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
        *contents: Container | SpaceView,
        active_tab: Optional[int | str] = None,
        name: Utf8Like | None = None,
    ):
        """
        Construct a new tab container.

        Parameters
        ----------
        *contents:
            All positional arguments are the contents of the container, which may be either other containers or space views.
        active_tab:
            The index or name of the active tab.
        name
            The name of the container

        """
        super().__init__(*contents, kind=ContainerKind.Tabs, active_tab=active_tab, name=name)
