from __future__ import annotations

import logging
from typing import Any

import rerun.log.extension_components
from rerun import bindings
from rerun.components.experimental.text_box import TextBoxArray
from rerun.components.instance import InstanceArray
from rerun.log.log_decorator import log_decorator

# Fully qualified to avoid circular import


@log_decorator
def log_text_box(
    entity_path: str,
    text: str,
    *,
    ext: dict[str, Any] | None = None,
    timeless: bool = False,
) -> None:
    """
    Log a textbox.

    This is intended to be used for multi-line text entries to be displayed in their own view.

    Parameters
    ----------
    entity_path:
        The object path to log the text entry under.
    text:
        The text to log.
    ext:
        Optional dictionary of extension components. See [rerun.log_extension_components][]
    timeless:
        Whether the text-box should be timeless.
    """

    instanced: dict[str, Any] = {}
    splats: dict[str, Any] = {}

    if text:
        instanced["rerun.text_box"] = TextBoxArray.from_bodies([(text,)])
    else:
        logging.warning(f"Null text entry in log_text_entry('{entity_path}') will be dropped.")

    if ext:
        rerun.log.extension_components._add_extension_components(instanced, splats, ext, None)

    if splats:
        splats["rerun.instance_key"] = InstanceArray.splat()
        bindings.log_arrow_msg(entity_path, components=splats, timeless=timeless)

    # Always the primary component last so range-based queries will include the other data. See(#1215)
    if instanced:
        bindings.log_arrow_msg(entity_path, components=instanced, timeless=timeless)
