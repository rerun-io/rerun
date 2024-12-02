# Official Rerun examples

* [C++](cpp)
* [Python](python)
* [Rust](rust)

> Note: Make sure your SDK version matches the code in the examples.
For example, if your SDK version is `0.4.0`, check out the matching tag
for this repository by running `git checkout v0.4.0`.

## Documentation

The rendered examples documentation can be seen [here](https://rerun.io/examples).

The examples currently use the following structure:
```
examples/
  python/
    <name>/
      README.md
      main.py
      requirements.txt
  rust/
    <name>/
      README.md
      Cargo.toml
      src/
        main.rs
```

The important part is that each example has a `README.md` file. The contents of this `README.md` is used to render the examples in [the documentation](https://rerun.io/examples).
Check out [`examples/python/template/README.md`](python/template/README.md) to see its format.

You are also encourage to add a _short_ `DESCRIPTION = """…"""` markdown to the top of the `main.py` and then log it with:
```py
rr.log("description", rr.TextDocument(DESCRIPTION, media_type=rr.MediaType.MARKDOWN), timeless=True)
```

## Adding a new example

You can base your example off of `python/template` or `rust/template`.
Once it's ready to be displayed in the docs, add it to the [manifest](./manifest.toml).

The `manifest.toml` file describes the structure of the examples contained in this repository. Only the examples which appear in the manifest are included in the [generated documentation](https://rerun.io/examples). The file contains a description of its own format.

If you want to run the example on CI and include it in the in-viewer example page,
add a `channel` entry to its README frontmatter. The available channels right now are:
- `main` for simple/fast examples built on each merge to `main`
- `nightly` for heavier examples built once per day
- `release` for very heavy examples built once per release

These channels are defined in: https://github.com/rerun-io/rerun/blob/18189a436271d58efe55a9c58fb3ff4d29098fd2/crates/build/re_dev_tools/src/build_examples/example.rs#L150-L158


If `channel` is missing, the example is never built on CI.
