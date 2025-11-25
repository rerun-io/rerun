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

        assert "write_table" not in dir(client)
        assert "append_to_table" not in dir(client)

        ds = client.create_dataset("my_dataset")

        assert "update" not in dir(ds)  # now: `set_name`
        assert "manifest_url" not in dir(ds)

        assert "register_batch" not in dir(ds)  # merged with `register`
