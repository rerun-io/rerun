from __future__ import annotations

import subprocess
import sys
from pathlib import Path
from typing import TYPE_CHECKING

import pyarrow as pa
import pytest
from rerun.catalog import AlreadyExistsError, NotFoundError, RegisteredBlueprint
from rerun.chunk import RrdReader

if TYPE_CHECKING:
    from rerun.chunk import LazyChunkStream

    from e2e_redap_tests.conftest import EntryFactory

BLUEPRINTS_DIR = Path(__file__).parent / "resources" / "blueprints"


def _entity_paths(stream: LazyChunkStream) -> set[str]:
    """
    The entity paths of the chunks of a blueprint stream.

    Views and containers get random ids, so two blueprints saved separately have different entity paths.
    """
    return {chunk.entity_path for chunk in stream.to_chunks()}


def _local_blueprint_stream(path: Path) -> LazyChunkStream:
    reader = RrdReader(path)
    [blueprint] = reader.blueprints()
    return reader.stream(store=blueprint)


def test_configure_blueprint_dataset(entry_factory: EntryFactory, resource_prefix: str) -> None:
    """Test configuring a blueprint dataset."""
    rbl_uri = resource_prefix + "blueprints/table_blueprint.rbl"
    rbl_uri2 = resource_prefix + "blueprints/table_blueprint2.rbl"

    ds = entry_factory.create_dataset("my_new_dataset")
    ds.register_prefix(resource_prefix + "dataset").wait()

    bds = ds.blueprint_dataset()
    assert bds is not None

    first = ds.register_blueprint(rbl_uri)

    assert len(bds.segment_ids()) == 1
    assert ds.default_blueprint() == first

    second = ds.register_blueprint(rbl_uri2, set_default=False)

    assert len(bds.segment_ids()) == 2
    assert set(ds.blueprints()) == {first, second}
    assert ds.default_blueprint() == first

    ds.set_default_blueprint(second)
    assert ds.default_blueprint() == second


def test_register_blueprint_stores_kind(entry_factory: EntryFactory, resource_prefix: str) -> None:
    """
    `register_table_blueprint` registers a table blueprint and `register_blueprint` a recording blueprint.

    Each one sets the default blueprint of its own kind.
    Registering the same .rbl again with the other method changes its kind.
    """
    rbl_uri = resource_prefix + "blueprints/table_blueprint.rbl"
    rbl_uri2 = resource_prefix + "blueprints/table_blueprint2.rbl"

    ds = entry_factory.create_dataset("blueprint_kind")

    table_blueprint = ds.register_table_blueprint(rbl_uri, set_default=False, name="table")
    assert table_blueprint.kind == "table"
    assert ds.default_segment_table_blueprint() is None
    assert ds.get_blueprint_by_name("table") == table_blueprint

    recording_blueprint = ds.register_blueprint(rbl_uri2, name="recording")
    assert recording_blueprint.kind == "recording"
    assert recording_blueprint.id != table_blueprint.id
    assert ds.default_blueprint() == recording_blueprint
    assert ds.default_segment_table_blueprint() is None

    assert ds.register_table_blueprint(rbl_uri, name="table") == table_blueprint
    assert ds.default_segment_table_blueprint() == table_blueprint
    assert ds.default_blueprint() == recording_blueprint

    made_recording = ds.register_blueprint(rbl_uri, set_default=False, name="table")
    assert made_recording == RegisteredBlueprint(id=table_blueprint.id, name="table", kind="recording")
    assert ds.get_blueprint_by_name("table") == made_recording


def test_reregister_same_blueprint(entry_factory: EntryFactory, resource_prefix: str) -> None:
    """Re-registering the same blueprint does not raise, and the dataset still has one blueprint."""
    rbl_uri = resource_prefix + "blueprints/table_blueprint.rbl"

    ds = entry_factory.create_dataset("reregister_blueprint_dataset")
    ds.register_prefix(resource_prefix + "dataset").wait()

    first = ds.register_blueprint(rbl_uri)

    bds = ds.blueprint_dataset()
    assert bds is not None
    assert len(bds.segment_ids()) == 1

    # Re-register the exact same blueprint — this should not raise
    assert ds.register_blueprint(rbl_uri) == first
    assert ds.blueprints() == [first]


@pytest.mark.min_hub_version("0.18.0")  # blueprint names need segment properties
def test_reregister_renames_blueprint(entry_factory: EntryFactory, resource_prefix: str) -> None:
    """
    Re-registering a blueprint under a new name renames it.

    Re-registering it without a name keeps the name it has.
    """
    rbl_uri = resource_prefix + "blueprints/table_blueprint.rbl"

    ds = entry_factory.create_dataset("reregister_renames_blueprint_dataset")

    first = ds.register_blueprint(rbl_uri, "first")
    assert ds.register_blueprint(rbl_uri, "first") == first

    renamed = ds.register_blueprint(rbl_uri, "renamed")
    assert renamed == RegisteredBlueprint(id=first.id, name="renamed")
    assert ds.blueprints() == [renamed]

    assert ds.register_blueprint(rbl_uri) == renamed
    assert ds.blueprints() == [renamed]


@pytest.mark.min_hub_version("0.18.0")  # blueprint names need segment properties
def test_unnamed_blueprint(entry_factory: EntryFactory, resource_prefix: str) -> None:
    """
    A blueprint registered without a name has no name, and can be named later.

    Several blueprints of a dataset can be without a name.
    """
    ds = entry_factory.create_dataset("unnamed_blueprint_dataset")

    first = ds.register_blueprint(resource_prefix + "blueprints/table_blueprint.rbl")
    second = ds.register_blueprint(resource_prefix + "blueprints/table_blueprint2.rbl", set_default=False)
    assert first.name is None
    assert second.name is None
    assert set(ds.blueprints()) == {first, second}
    assert ds.get_blueprint_by_name(first.id) is None

    named = first.rename("overview")
    assert named == RegisteredBlueprint(id=first.id, name="overview")
    assert ds.get_blueprint_by_name("overview") == named
    assert set(ds.blueprints()) == {named, second}


def test_register_blueprint_positional_set_default(entry_factory: EntryFactory, resource_prefix: str) -> None:
    """Passing `set_default` by position still works, with a deprecation warning."""
    ds = entry_factory.create_dataset("positional_set_default_dataset")

    with pytest.warns(DeprecationWarning, match="set_default"):
        blueprint = ds.register_blueprint(resource_prefix + "blueprints/table_blueprint.rbl", False)  # type: ignore[call-overload]

    assert ds.blueprints() == [blueprint]
    assert ds.default_blueprint() is None


@pytest.mark.min_hub_version("0.18.0")  # blueprint names need segment properties
def test_blueprint_names_are_unique(entry_factory: EntryFactory, resource_prefix: str) -> None:
    """
    Blueprints are named when registered and can be renamed later.

    A name another blueprint of the dataset has is refused, the refused blueprint is not kept, and the default blueprint stays as it was.
    """
    ds = entry_factory.create_dataset("blueprint_names_dataset")
    ds.register_prefix(resource_prefix + "dataset").wait()
    bds = ds.blueprint_dataset()
    assert bds is not None

    first = ds.register_blueprint(resource_prefix + "blueprints/table_blueprint.rbl", "overview")
    assert first.name == "overview"
    assert ds.get_blueprint_by_name("overview") == first

    with pytest.raises(AlreadyExistsError):
        ds.register_blueprint(resource_prefix + "blueprints/table_blueprint2.rbl", "overview")
    assert len(bds.segment_ids()) == 1
    assert ds.default_blueprint() == first

    second = ds.register_blueprint(resource_prefix + "blueprints/table_blueprint2.rbl", "plots", set_default=False)

    with pytest.raises(AlreadyExistsError):
        second.rename("overview")

    assert first.rename("overview") == first

    first = first.rename("everything")
    second = second.rename("overview")
    assert first.name == "everything"
    assert second.name == "overview"
    assert ds.get_blueprint_by_name("overview") == second
    assert ds.get_blueprint_by_name("plots") is None


@pytest.mark.local_only
def test_rejected_registration_keeps_existing_blueprint(entry_factory: EntryFactory, tmp_path: Path) -> None:
    """
    Registering a new version of a registered blueprint under a name another blueprint has is refused.

    The registered blueprint keeps its content and its name, and the default blueprint still points at it.
    """
    first_rbl = BLUEPRINTS_DIR / "table_blueprint.rbl"
    second_rbl = BLUEPRINTS_DIR / "table_blueprint2.rbl"

    ds = entry_factory.create_dataset("rejected_registration_dataset")
    bds = ds.blueprint_dataset()
    assert bds is not None

    first = ds.register_blueprint(first_rbl.absolute().as_uri(), "overview")
    second = ds.register_blueprint(second_rbl.absolute().as_uri(), "plots", set_default=False)
    assert ds.default_blueprint() == first

    first_paths = _entity_paths(_local_blueprint_stream(first_rbl))
    assert _entity_paths(bds.segment_store(first.id).stream()) == first_paths

    # The content of the second blueprint, stored under the recording id of the first one.
    edited_rbl = tmp_path / "edited.rbl"
    result = subprocess.run(
        [
            sys.executable,
            "-m",
            "rerun",
            "rrd",
            "route",
            "--recording-id",
            first.id,
            str(second_rbl),
            "-o",
            str(edited_rbl),
        ],
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode == 0, result.stdout + result.stderr
    [edited_store] = RrdReader(edited_rbl).blueprints()
    assert edited_store.recording_id == first.id
    assert _entity_paths(_local_blueprint_stream(edited_rbl)) != first_paths

    with pytest.raises(AlreadyExistsError):
        ds.register_blueprint(edited_rbl.absolute().as_uri(), "plots")

    assert set(ds.blueprints()) == {first, second}
    assert ds.get_blueprint_by_name("overview") == first
    assert ds.get_blueprint_by_name("plots") == second
    assert ds.default_blueprint() == first
    assert _entity_paths(bds.segment_store(first.id).stream()) == first_paths


@pytest.mark.min_hub_version("0.18.0")  # blueprint names need segment properties
def test_rename_unregistered_blueprint(entry_factory: EntryFactory, resource_prefix: str) -> None:
    """
    Renaming a blueprint that is no longer registered in the dataset fails.

    The name stays free, so a registered blueprint can get it afterwards.
    """
    ds = entry_factory.create_dataset("rename_unregistered_blueprint_dataset")
    bds = ds.blueprint_dataset()
    assert bds is not None

    unregistered = ds.register_blueprint(resource_prefix + "blueprints/table_blueprint2.rbl", set_default=False)
    bds.unregister(segments_to_drop=unregistered.id, layers_to_drop=[]).wait()

    with pytest.raises(NotFoundError):
        unregistered.rename("a")
    assert ds.get_blueprint_by_name("a") is None

    blueprint = ds.register_blueprint(resource_prefix + "blueprints/table_blueprint.rbl", "a")
    assert blueprint.name == "a"
    assert ds.blueprints() == [blueprint]
    assert ds.get_blueprint_by_name("a") == blueprint


@pytest.mark.min_hub_version("0.18.0")  # blueprint names need segment properties
def test_reregister_unregistered_blueprint_without_name(entry_factory: EntryFactory, resource_prefix: str) -> None:
    """
    Registering an unregistered blueprint again without a name gives it no name.

    Its old name stays with the blueprint that took it in the meantime.
    """
    ds = entry_factory.create_dataset("reregister_unregistered_blueprint_dataset")
    bds = ds.blueprint_dataset()
    assert bds is not None

    first = ds.register_blueprint(resource_prefix + "blueprints/table_blueprint.rbl", "overview")
    bds.unregister(segments_to_drop=first.id, layers_to_drop=[]).wait()
    second = ds.register_blueprint(resource_prefix + "blueprints/table_blueprint2.rbl", "overview")

    again = ds.register_blueprint(resource_prefix + "blueprints/table_blueprint.rbl")
    assert again == RegisteredBlueprint(id=first.id, name=None)
    assert set(ds.blueprints()) == {again, second}
    assert ds.get_blueprint_by_name("overview") == second


def test_rename_blueprint_without_dataset() -> None:
    """A `RegisteredBlueprint` built by hand has no dataset or table to rename it in."""
    with pytest.raises(ValueError, match="returned by a dataset or a table"):
        RegisteredBlueprint(id="typo").rename("a")


def test_configure_table_blueprint_dataset(entry_factory: EntryFactory, resource_prefix: str) -> None:
    """Test configuring a table blueprint dataset."""
    rbl_uri = resource_prefix + "blueprints/table_blueprint.rbl"
    rbl_uri2 = resource_prefix + "blueprints/table_blueprint2.rbl"

    table = entry_factory.create_table("table_with_blueprints", pa.schema([pa.field("col", pa.int32())]))

    assert table.blueprint_dataset() is not None
    assert table.blueprints() == []
    assert table.default_blueprint() is None

    first = table.register_blueprint(rbl_uri)

    blueprint_dataset = table.blueprint_dataset()
    assert blueprint_dataset is not None
    assert len(blueprint_dataset.segment_ids()) == 1
    assert table.blueprints() == [first]
    assert table.default_blueprint() == first

    second = table.register_blueprint(rbl_uri2, set_default=False)

    assert len(table.blueprints()) == 2
    assert table.default_blueprint() == first

    table.set_default_blueprint(second)
    assert table.default_blueprint() == second

    table.set_default_blueprint(None)
    assert table.default_blueprint() is None


@pytest.mark.min_hub_version("0.18.0")  # blueprint names need segment properties
def test_table_blueprint_names(entry_factory: EntryFactory, resource_prefix: str) -> None:
    """The blueprints of a table are named when registered, found by name, and can be renamed."""
    table = entry_factory.create_table("table_with_named_blueprints", pa.schema([pa.field("col", pa.int32())]))

    first = table.register_blueprint(resource_prefix + "blueprints/table_blueprint.rbl", "first")
    assert first.name == "first"
    assert table.get_blueprint_by_name("first") == first
    assert table.default_blueprint() == first

    with pytest.raises(AlreadyExistsError):
        table.register_blueprint(resource_prefix + "blueprints/table_blueprint2.rbl", "first")

    renamed = first.rename("layout")
    assert table.get_blueprint_by_name("layout") == renamed
    assert table.get_blueprint_by_name("first") is None
    assert table.blueprints() == [renamed]


def test_table_blueprint_set_default_false_creates_dataset_without_default(
    entry_factory: EntryFactory, resource_prefix: str
) -> None:
    """Registering the first table blueprint with set_default=False leaves default unset."""
    rbl_uri = resource_prefix + "blueprints/table_blueprint.rbl"

    table = entry_factory.create_table("table_blueprint_set_default_false", pa.schema([pa.field("col", pa.int32())]))

    assert table.blueprint_dataset() is not None
    assert table.default_blueprint() is None

    blueprint = table.register_blueprint(rbl_uri, set_default=False)

    assert blueprint.kind == "table"
    assert table.default_blueprint() is None
    blueprint_dataset = table.blueprint_dataset()
    assert blueprint_dataset is not None
    assert len(blueprint_dataset.segment_ids()) == 1
    assert table.blueprints() == [blueprint]


def test_table_default_blueprint_uses_creation_blueprint_dataset(entry_factory: EntryFactory) -> None:
    """Setting a table default blueprint uses the dataset created with the table."""
    table = entry_factory.create_table("table_default_with_blueprint_dataset", pa.schema([pa.field("col", pa.int32())]))

    missing = RegisteredBlueprint(id="missing_blueprint_segment")
    table.set_default_blueprint(missing)

    assert table.blueprint_dataset() is not None
    assert table.default_blueprint() == missing


def test_table_default_blueprint_rejects_deleted_blueprint_dataset(
    entry_factory: EntryFactory, resource_prefix: str
) -> None:
    """Setting a table default blueprint should fail if the referenced blueprint dataset is gone."""
    table = entry_factory.create_table("table_deleted_blueprint_dataset", pa.schema([pa.field("col", pa.int32())]))
    table.register_blueprint(resource_prefix + "blueprints/table_blueprint.rbl", set_default=False)

    blueprint_dataset = table.blueprint_dataset()
    assert blueprint_dataset is not None
    blueprint_dataset.delete()

    with pytest.raises(Exception, match="table blueprint dataset does not exist"):
        table.set_default_blueprint(RegisteredBlueprint(id="missing_blueprint_segment"))


def test_dataset_default_blueprint_rejects_deleted_blueprint_dataset(entry_factory: EntryFactory) -> None:
    """Setting a dataset default blueprint should fail if the referenced blueprint dataset is gone."""
    dataset = entry_factory.create_dataset("dataset_deleted_blueprint_dataset")
    blueprint_dataset = dataset.blueprint_dataset()
    assert blueprint_dataset is not None
    blueprint_dataset.delete()

    with pytest.raises(Exception, match="dataset blueprint dataset does not exist"):
        dataset.set_default_blueprint(RegisteredBlueprint(id="missing_blueprint_segment"))
