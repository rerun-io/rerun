---
title: Migrating from 0.35 to 0.36
order: 974
---

## `ParquetReader` loading options moved to `stream()`

The experimental `ParquetReader`'s constructor now takes only the file path.
All loading options (`entity_path_prefix`, `column_grouping`, `delimiter`, `prefixes`, `use_structs`, `static_columns`, `index_columns`) moved to `stream()`:

```python
# 0.35
ParquetReader(path, column_grouping="individual", index_columns=[IndexColumn.sequence("frame")]).stream()

# 0.36
ParquetReader(path).stream(column_grouping="individual", index_columns=[IndexColumn.sequence("frame")])
```

The reader is now a lightweight handle over the file, and each `stream()` call is independent — one reader can drive several differently-configured streams over the same file.

## `rerun mcap info` output changed

The `rerun mcap info` CLI command has been rewritten to output richer and more detailed file-level information instead of just diagnostic checks.
The diagnostic checks are now in a dedicated `rerun mcap check` subcommand instead.
