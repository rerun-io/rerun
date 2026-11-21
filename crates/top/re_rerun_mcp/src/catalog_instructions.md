Catalog tools (`catalog_*`) work on the contents of a Rerun catalog server (does not need nor affect a viewer): its datasets, their segments (one per recording), schemas, segment properties, and data.

- Start with `catalog_list_entries` for the dataset names, then `catalog_get_dataset_info` for an overview. Name a dataset by its name or its id.
- Never guess an entity path or a component name: read them with `catalog_get_dataset_schema`.
- Read values with `catalog_query_dataset`: pick the `index` timeline, the `segments`, and the `entity_paths` you need, and aggregate in its `sql` rather than reading many rows. Large values such as images come back cut short or left out.
- Narrow queries to the segments, entities, and time range you need. Ask the user before scanning a whole large dataset if the scope is unclear.
- `catalog_list_servers` lists the catalog servers the user has added. If none matches what the user names, ask for the URL rather than guessing one.
- If `catalog_connect` says the server needs a login, call `catalog_login`, show the user its link, call `catalog_login` again to wait until they are done, then retry `catalog_connect`.
