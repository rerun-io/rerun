# re_build_search_index

Builds a Meilisearch index from our documentation.

### Requirements

- `pixi`
- A nightly Rust compiler, used because `rustdoc` JSON output is unstable
  ```
  rustup install nightly
  rustup +nightly target add wasm32-unknown-unknown
  ```
- A local installation of `rerun_sdk` and `rerun_py/requirements-doc.txt`
  ```
  pixi run py-build
  pixi run pip install -r rerun_py/requirements-doc.txt
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
