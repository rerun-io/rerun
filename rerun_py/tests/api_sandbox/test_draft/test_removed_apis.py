from __future__ import annotations

import rerun_draft as rr


def test_removed_apis() -> None:
    """Ensure that removed APIs are indeed removed."""
    with rr.server.Server() as server:
        client = server.client()

        assert "all_entries" not in dir(client)
        assert "dataset_entries" not in dir(client)
        assert "table_entries" not in dir(client)

        assert "get_dataset_entry" not in dir(client)
        assert "get_table_entry" not in dir(client)
        assert "create_table_entry" not in dir(client)

        assert "write_table" not in dir(client)
        assert "append_to_table" not in dir(client)
        assert "update_table" not in dir(client)

        ds = client.create_dataset("my_dataset")

        assert "update" not in dir(ds)  # now: `set_name`
        assert "manifest_url" not in dir(ds)

        assert "register_batch" not in dir(ds)  # me

        # These were renamed with `_index` -> `_search_index`
        assert "create_fts_index" not in dir(ds)
        assert "create_vector_index" not in dir(ds)
        assert "list_indexes" not in dir(ds)
        assert "delete_indexes" not in dir(ds)

        # Replaced by a better, simpler API outline in
        # https://linear.app/rerun/issue/RR-3018/improve-the-dataset-blueprint-apis-in-the-python-sdk
        assert "blueprint_dataset_id" not in dir(ds)
        assert "blueprint_dataset" not in dir(ds)
        assert "default_blueprint_segment_id" not in dir(ds)
        assert "set_default_blueprint_segment_id" not in dir(ds)
