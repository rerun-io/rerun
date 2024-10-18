# build_search_index

Builds a Meilisearch index from our documentation.

### Requirements

- `pixi`
- A nightly Rust compiler, used because `rustdoc` JSON output is unstable
  ```
  rustup install nightly
  rustup +nightly target add wasm32-unknown-unknown
  ```
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
