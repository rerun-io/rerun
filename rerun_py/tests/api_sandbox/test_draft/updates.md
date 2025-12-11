Should CatalogClient have a .connect() / when does it return errors?
Use address to connect to default catalog.

Review Client Factory / address API.

Look at 3 other things that do something similar. Ask ChapGPT for opinion.

Can we make discovery easier:

```
rerun auth login

python
> rr.list_servers()
```

Add single-string example to append.

3 APIs:
.append()
.overwrite()
.upsert() or .replace()

NO table.write())
NO cli.write_table()

All of this take recordbatchreader as a optional unnamed argument.
OR take named arguments as an implicit constructor.

.append_batches()
.overwrite_batches()
.upsert() or .replace()

Add upsert error example to API tests

IN DASET BASICS -> Add metadata
