# re_dev_tools
Crate that combines several development utilities.
To get an overview over all tools run `pixi run dev-tools --help`.

We keep all smaller Rust "scripts" in this single crate so we don't needlessly
increase the number of such utility crates and to make it easy to get
an overview over all build tooling written in Rust.

## Adding a new tool
* Create a folder under `src` with your new tool
* Add a new enum entry to `main.rs`
