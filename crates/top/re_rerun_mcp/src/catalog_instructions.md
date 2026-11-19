Catalog tools (`catalog_*`) describe the contents of a Rerun catalog server (does not need nor affect a viewer): its datasets, their segments (one per recording), schemas, and segment properties.

- Start with `catalog_list_entries` for the dataset names, then `catalog_get_dataset_info` for an overview. Name a dataset by its name or its id.
- Never guess an entity path or a component name: read them with `catalog_get_dataset_schema`.
- If no server matches what the user names, ask for the URL rather than guessing one.
