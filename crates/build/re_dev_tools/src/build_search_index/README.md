# build_search_index

Builds a Meilisearch index from our documentation.

### Requirements

- `pixi`

### Usage

Start a local `meilisearch` instance:
```
pixi run meilisearch
```

Index contents of the repository:
```
$ pixi run search-index build
```

Start a REPL against the local `meilisearch` instance:
```
$ pixi run search-index repl
```
