---
title: "Python API for table blueprints"
hidden: true
type: feature
---

### Python API for table blueprints

The new `rerun.blueprint.table` Python API lets you conveniently configure table and card layouts.
Customize column order, labels, visibility, and cell renderers, including recording previews and editable boolean flags:

```python
from pathlib import Path

import rerun as rr
import rerun.blueprint as rrb

blueprint = rrb.TableBlueprint(
    table_layout=rrb.table.TableLayout(
        # Hide a column
        columns=[rrb.table.Column("episode_notes", visible=False)],
    ),
    # Cards become the default layout unless default_layout="table".
    card_layout=rrb.table.CardLayout(
        title="uuid",  # Use the uuid column as the card title.
        link="recording_uri",  # Clicking the card should open the recording.
        fields=[
            rrb.table.Column(
                "recording_uri",
                name="Recording",
                # Show a 3D view for recordings in this row.
                cell=rrb.table.PreviewCell(rrb.Spatial3DView()),
            ),
            # Columns on the card layout are opt-in.
            rrb.table.Column("current_task"),
        ],
    ),
    # Configure the timeline used by previews.
    previews_config=rrb.table.PreviewsConfig(timeline="real_time"),
)
path = Path("table.rbl").resolve()
blueprint.save("my_app", path)

client = rr.catalog.CatalogClient("rerun+http://localhost:51234")
client.get_table("my_table").register_blueprint(path.as_uri())
```

<picture>
  <img src="https://static.rerun.io/table_blueprint_cards/c4375be1b19f3e5ed6a440aa0fceaca50cbf0a6d/full.png" alt="Card layout with the episode UUID as title, a 3D recording preview, and the current task on each card">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/table_blueprint_cards/c4375be1b19f3e5ed6a440aa0fceaca50cbf0a6d/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/table_blueprint_cards/c4375be1b19f3e5ed6a440aa0fceaca50cbf0a6d/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/table_blueprint_cards/c4375be1b19f3e5ed6a440aa0fceaca50cbf0a6d/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/table_blueprint_cards/c4375be1b19f3e5ed6a440aa0fceaca50cbf0a6d/1200w.png">
</picture>

The `.rbl` file must be accessible to the server.
For a remote server, upload it to shared storage and pass that URI to `register_blueprint` instead.
See [Configure table layouts and recording previews](../howto/visualization/configure-table-blueprints.md?speculative-link) for the full workflow, including dataset segment tables and remote storage.
For more configurations, see the [table blueprints example](https://github.com/rerun-io/rerun/blob/latest/examples/python/table_blueprints).
