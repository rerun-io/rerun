---
title: "Table columns can use any component UI and be edited"
hidden: true
type: breaking
---

### Table columns show and edit any component type

A table blueprint column can now be shown as any component, e.g. `rr.components.Text`, `rr.components.Url` or `rr.components.Thumbnail`.
Boolean columns show as checkboxes by default, and `rr.components.Flag` shows them as flag buttons.
Editing is no longer limited to flags: any editable column whose component type has an editor in the Viewer can be edited, including plain text and number columns.

<video width="100%" autoplay loop muted controls>
    <source src="https://static.rerun.io/5f7a1167b2174451397129941509c166b25ef807_table_column_types_v2.mp4" type="video/mp4" />
</video>

`Column(cell=…)` is renamed to `Column(column_type=…)`, and `LinkCell`, `ThumbnailCell`, `FlagCell` and `EntryKindCell` are removed in favor of component classes:

```python
# Before
rrb.table.Column("reviewed", editable=True, cell=rrb.table.FlagCell())

# After
rrb.table.Column("reviewed", editable=True, column_type=rr.components.Flag)
```

Existing table blueprints will be automatically migrated.

See [Configure table layouts and segment previews](../howto/visualization/configure-table-blueprints.md#editing-columns).
