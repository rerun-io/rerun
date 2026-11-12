from __future__ import annotations

from dataclasses import dataclass, field
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from rerun_bindings import DatasetEntryInternal, TableEntryInternal


@dataclass(frozen=True)
class RegisteredBlueprint:
    """
    A blueprint registered for a dataset or a table.

    This is the state at the time it was read, and does not follow later renames.
    """

    id: str
    """The ID of the blueprint, which is its segment ID in the blueprint dataset."""

    name: str | None = None
    """
    The name of the blueprint, unique among the blueprints of the dataset or table, or `None` if it has no name.

    Registering or renaming to a name another blueprint has raises `AlreadyExistsError`.
    Two calls that give different blueprints the same name at the same time can both succeed.
    On a server that doesn't store blueprint names, the name is `None`.
    """

    _entry: DatasetEntryInternal | TableEntryInternal | None = field(
        default=None, init=False, repr=False, compare=False
    )
    """The dataset or table that returned this blueprint."""

    def rename(self, name: str) -> RegisteredBlueprint:
        """
        Rename this blueprint on the server, and return the renamed blueprint.

        This object keeps its old name.

        If another blueprint has `name`, this raises `AlreadyExistsError`.
        If this blueprint isn't registered anymore, this raises `NotFoundError`.
        On a server that doesn't store blueprint names, this raises `RuntimeError`.
        A `RegisteredBlueprint` that didn't come from a dataset or a table raises `ValueError`.
        """

        blueprint_dataset = None if self._entry is None else self._entry.blueprint_dataset()
        if self._entry is None or blueprint_dataset is None:
            raise ValueError(
                "Only a blueprint returned by a dataset or a table can be renamed, "
                "for example by `register_blueprint` or `get_blueprint_by_name`"
            )

        return _registered_blueprint_from_internal(self._entry, blueprint_dataset.set_blueprint_name(self.id, name))


def _registered_blueprint_from_internal(
    entry: DatasetEntryInternal | TableEntryInternal, id_and_name: tuple[str, str | None]
) -> RegisteredBlueprint:
    id, name = id_and_name
    blueprint = RegisteredBlueprint(id=id, name=name)
    object.__setattr__(blueprint, "_entry", entry)
    return blueprint


def _optional_registered_blueprint(
    entry: DatasetEntryInternal | TableEntryInternal, id_and_name: tuple[str, str | None] | None
) -> RegisteredBlueprint | None:
    if id_and_name is None:
        return None
    return _registered_blueprint_from_internal(entry, id_and_name)
