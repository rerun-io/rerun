from __future__ import annotations

from typing import Any

from typing_extensions import deprecated  # type: ignore[misc, unused-ignore]

from rerun._log import log
from rerun.any_value import AnyValues
from rerun.archetypes import TextLog
from rerun.log_deprecated import Color, _normalize_colors
from rerun.log_deprecated.log_decorator import log_decorator
from rerun.recording_stream import RecordingStream

# Fully qualified to avoid circular import

__all__ = [
    "log_text_entry",
]


@deprecated(
    """Please migrate to `rr.log(…, rr.TextLog(…))`.
  See: https://www.rerun.io/docs/reference/migration-0-9 for more details."""
)
@log_decorator
def log_text_entry(
    entity_path: str,
    text: str,
    *,
    level: str | None = "INFO",
    color: Color | None = None,
    ext: dict[str, Any] | None = None,
    timeless: bool = False,
    recording: RecordingStream | None = None,
) -> None:
    """
    Log a text entry, with optional level.

    !!! Warning "Deprecated"
        Please migrate to [rerun.log][] with [rerun.TextLog][].

        See [the migration guide](https://www.rerun.io/docs/reference/migration-0-9) for more details.

    Parameters
    ----------
    entity_path:
        The object path to log the text entry under.
    text:
        The text to log.
    level:
        The level of the text entry. This can technically
        be an arbitrary string, but it's recommended to use one of "CRITICAL", "ERROR", "WARN", "INFO", "DEBUG".
    color:
        Optional RGB or RGBA in sRGB gamma-space as either 0-1 floats or 0-255 integers, with separate alpha.
    ext:
        Optional dictionary of extension components. See [rerun.log_extension_components][]
    timeless:
        Whether the text entry should be timeless.
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """

    recording = RecordingStream.to_native(recording)

    if color is not None:
        color = _normalize_colors(color)

    return log(
        entity_path,
        TextLog(text, level=level, color=color),
        AnyValues(**(ext or {})),
        timeless=timeless,
        recording=recording,
    )
