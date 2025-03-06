from __future__ import annotations

from typing import Any

from ...error_utils import catch_and_log_exceptions
from .. import components as blueprint_components


class TensorViewFitExt:
    """Extension for [TensorViewFit][rerun.blueprint.archetypes.TensorViewFit]."""

    def __init__(self: Any, scaling: blueprint_components.ViewFitLike | None = None) -> None:
        """
        Create a new instance of the TensorViewFit archetype.

        Parameters
        ----------
        scaling:
            How the image is scaled to fit the view.

        """

        with catch_and_log_exceptions(context=self.__class__.__name__):
            self.__attrs_init__(scaling=scaling)
            return
        self.__attrs_clear__()
