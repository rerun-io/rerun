---
title: "Registered blueprints can have a name, and the blueprint APIs take and return `RegisteredBlueprint`"
hidden: true
type: breaking
---

### Registered blueprints can have a name, and the blueprint APIs take and return `RegisteredBlueprint`

`register_blueprint` on `DatasetEntry` and `TableEntry` takes an optional `name` as its second argument.
A name is unique among the blueprints of the dataset or table, and registering under a name another blueprint has raises `AlreadyExistsError`.
A blueprint registered without a name has none, and can be named later with `RegisteredBlueprint.rename`.
Registering a blueprint without a name logs a warning once per process.

`set_default` is now keyword-only.
Passing it by position still works, with a deprecation warning.

The blueprint methods take and return a `rerun.catalog.RegisteredBlueprint`, which has the blueprint's `id`, `name` and `kind`, instead of a blueprint ID string.
`kind` is `"recording"` or `"table"`.

This covers `register_blueprint`, `blueprints`, `default_blueprint`, `set_default_blueprint`, `default_segment_table_blueprint` and `set_default_segment_table_blueprint`.
`get_blueprint_by_name` is new on `DatasetEntry` and `TableEntry`.

A blueprint of the dataset's segment table is registered with the new `DatasetEntry.register_table_blueprint`.

#### Migration

Python, before:

```python
dataset.register_blueprint(uri)
dataset.register_blueprint(other_uri, False)
[first, second] = dataset.blueprints()
dataset.set_default_blueprint(second)
assert dataset.default_blueprint() == second
```

Python, after:

```python
dataset.register_blueprint(uri, "overview")
second = dataset.register_blueprint(other_uri, "timeseries", set_default=False)
dataset.set_default_blueprint(second)
assert dataset.default_blueprint() == second

second = second.rename("plots")
assert dataset.get_blueprint_by_name("plots") == second
assert {blueprint.name for blueprint in dataset.blueprints()} == {"overview", "plots"}
```

Code that used the blueprint ID string can read it from `RegisteredBlueprint.id`.
