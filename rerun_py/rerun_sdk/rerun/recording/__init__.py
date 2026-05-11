from __future__ import annotations

from typing import TYPE_CHECKING

from typing_extensions import deprecated

from rerun_bindings import (
    load_archive as _load_archive,
    load_recording as _load_recording,
)

from ._recording import Recording as Recording, RRDArchive as RRDArchive  # ty:ignore[deprecated]

if TYPE_CHECKING:
    from pathlib import Path


@deprecated(
    "load_recording is deprecated since 0.32. Use rerun.experimental.RrdReader(path).store() instead.",
)
def load_recording(path_to_rrd: str | Path) -> Recording:  # ty:ignore[deprecated]
    """
    Load a single recording from an RRD file.

    Will raise a `ValueError` if the file does not contain exactly one recording.

    Parameters
    ----------
    path_to_rrd:
        The path to the file to load.

    Returns
    -------
    Recording
        The loaded recording.

    """
    return Recording(_load_recording(path_to_rrd))  # ty: ignore[deprecated]


@deprecated(
    "load_archive is deprecated since 0.32. Use rerun.experimental.RrdReader(path) instead.",
)
def load_archive(path_to_rrd: str | Path) -> RRDArchive:  # ty:ignore[deprecated]
    """
    Load a rerun archive from an RRD file.

    Parameters
    ----------
    path_to_rrd:
        The path to the file to load.

    Returns
    -------
    RRDArchive
        The loaded archive.

    """
    return RRDArchive(_load_archive(path_to_rrd))  # ty: ignore[deprecated]
