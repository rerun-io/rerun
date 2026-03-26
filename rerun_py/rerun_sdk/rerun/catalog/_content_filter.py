from __future__ import annotations


def _build_path_expr(path: str, subtree: bool) -> str:
    if not path.startswith("/"):
        raise ValueError(f"Entity path expression must start with '/', got: {path!r}")
    if subtree and not path.endswith("/**"):
        path = f"{path}/**"
    return path


class ContentFilter:
    r"""
    Immutable builder for entity path filter expressions passed to `filter_contents()`.

    Start from `everything()` or `nothing()`, then chain `include()`, `exclude()`,
    and `include_properties()` calls to build up the filter. Each method returns a
    new `ContentFilter`, leaving the original unchanged.

    Examples
    --------
    ```python
    # Include everything except raw robot data, but keep one specific path:
    view = dataset.filter_contents(
        ContentFilter.everything()
        .exclude("/robot/raw/**")
        .include("/robot/raw/i_need_this")
    )

    # Include nothing, then allow specific subtrees:
    view = dataset.filter_contents(
        ContentFilter.nothing()
        .include("/points", subtree=True)
        .include("/world/camera")
    )

    # Path segments with special characters are auto-escaped via include_path/exclude_path:
    f = ContentFilter.everything().exclude_path(["robot", "raw data", "sensor!"], subtree=True)
    # equivalent to: exclude("/robot/raw\ data/sensor\!/**")
    ```

    """

    def __init__(self, exprs: tuple[str, ...] | list[str]) -> None:
        self._exprs: tuple[str, ...] = tuple(exprs)

    @classmethod
    def everything(cls) -> ContentFilter:
        """
        Start with all entity paths included (auto-excludes `__properties`, see `include_properties()`).

        Equivalent to `filter_contents("/**")`.
        """
        return cls(("/**",))

    @classmethod
    def nothing(cls) -> ContentFilter:
        """
        Start with all entity paths excluded.

        Equivalent to `filter_contents([])`.
        """
        return cls(())

    def include(self, path: str, *, subtree: bool = False) -> ContentFilter:
        r"""
        Include entity paths matching `path`.

        Parameters
        ----------
        path
            A pre-formed entity path expression string (e.g. `"/robot/raw/**"`).
            Must start with `"/"`.
        subtree
            If `True`, appends `/**` to match the path and all its descendants.
            Ignored if the path string already ends with `/**`.

        """
        return ContentFilter((*self._exprs, _build_path_expr(path, subtree)))

    def exclude(self, path: str, *, subtree: bool = False) -> ContentFilter:
        r"""
        Exclude entity paths matching `path`.

        Parameters
        ----------
        path
            A pre-formed entity path expression string (e.g. `"/robot/raw/**"`).
            Must start with `"/"`.
        subtree
            If `True`, appends `/**` to match the path and all its descendants.
            Ignored if the path string already ends with `/**`.

        """
        return ContentFilter((*self._exprs, f"-{_build_path_expr(path, subtree)}"))

    def include_properties(self) -> ContentFilter:
        """
        Include the `__properties/**` subtree.

        By default `__properties` is auto-excluded. Calling this method suppresses
        that auto-exclusion for the entire subtree. See the `filter_contents()` docs
        for details on `__properties` handling.
        """
        return ContentFilter((*self._exprs, "/__properties/**"))

    def to_exprs(self) -> list[str]:
        """Return the accumulated list of filter expression strings."""
        return list(self._exprs)

    def __eq__(self, other: object) -> bool:
        if not isinstance(other, ContentFilter):
            return NotImplemented
        return self._exprs == other._exprs

    def __hash__(self) -> int:
        return hash(self._exprs)

    def __repr__(self) -> str:
        return f"ContentFilter({list(self._exprs)!r})"
