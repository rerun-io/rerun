<h1 align="center">
  <a href="https://www.rerun.io/">
    <img alt="banner" src="https://user-images.githubusercontent.com/1148717/218142418-1d320929-6b7a-486e-8277-fbeef2432529.png">
  </a>
</h1>

<h1 align="center">
  <a href="https://crates.io/crates/rerun-cli">                           <img alt="Latest version" src="https://img.shields.io/crates/v/rerun-cli.svg">                            </a>
  <a href="https://docs.rs/rerun-cli">                                    <img alt="Documentation"  src="https://docs.rs/rerun-cli/badge.svg">                                      </a>
  <a href="https://github.com/rerun-io/rerun/blob/master/LICENSE-MIT">    <img alt="MIT"            src="https://img.shields.io/badge/license-MIT-blue.svg">                        </a>
  <a href="https://github.com/rerun-io/rerun/blob/master/LICENSE-APACHE"> <img alt="Apache"         src="https://img.shields.io/badge/license-Apache-blue.svg">                     </a>
  <a href="https://discord.gg/Gcm8BbTaAj">                                <img alt="Rerun Discord"  src="https://img.shields.io/discord/1062300748202921994?label=Rerun%20Discord"> </a>
</h1>

## Rerun command-line tool
You can install the binary with `cargo install rerun-cli`

This can act either as a server, a viewer, or both, depending on which options you use when you start it.

Running `rerun` with no arguments will start the viewer, waiting for an SDK to connect to it over TCP.

Run `rerun --help` for more.


## What is Rerun?
- [Examples](https://github.com/rerun-io/rerun/tree/latest/examples/rust)
- [High-level docs](http://rerun.io/docs)
- [Rust API docs](https://docs.rs/rerun/)
- [Troubleshooting](https://www.rerun.io/docs/getting-started/troubleshooting)


### Running a web viewer
```sh
rerun --web-viewer ../nyud.rrd
```
