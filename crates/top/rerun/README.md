<h1 align="center">
  <a href="https://www.rerun.io/">
    <img width="1000" height="200" alt="Banner with Rerun logo" src="https://static.rerun.io/d0f5443d4803cac65c73fcc064936c09f5e7f208_rerun_banner.png" />
  </a>
</h1>

<h1 align="center">
  <a href="https://crates.io/crates/rerun">                             <img alt="Latest version" src="https://img.shields.io/crates/v/rerun.svg">                               </a>
  <a href="https://docs.rs/rerun">                                      <img alt="Documentation"  src="https://docs.rs/rerun/badge.svg">                                         </a>
  <a href="https://github.com/rerun-io/rerun/blob/main/LICENSE-MIT">    <img alt="MIT"            src="https://img.shields.io/badge/license-MIT-blue.svg">                        </a>
  <a href="https://github.com/rerun-io/rerun/blob/main/LICENSE-APACHE"> <img alt="Apache"         src="https://img.shields.io/badge/license-Apache-blue.svg">                     </a>
  <a href="https://discord.gg/Gcm8BbTaAj">                              <img alt="Rerun Discord"  src="https://img.shields.io/discord/1062300748202921994?label=Rerun%20Discord"> </a>
</h1>

# Rerun Rust logging SDK
Rerun is an SDK for logging computer vision and robotics data paired with a visualizer for exploring that data over time. It lets you debug and understand the internal state and data of your systems with minimal code.

```shell
cargo add rerun
````

```rust
let rec = rerun::RecordingStream::global(rerun::StoreKind::Recording)?;
rec.log("points", &rerun::archetypes::Points3D::new(points).with_colors(colors))?;
rec.log("image", &rerun::archetypes::Image::new(image))?;
```

<p align="center">
  <img width="800" alt="Rerun Viewer" src="https://user-images.githubusercontent.com/1148717/218763490-f6261ecd-e19e-4520-9b25-446ce1ee6328.png">
</p>

## Getting started
- [Examples](https://github.com/rerun-io/rerun/tree/latest/examples/rust)
- [High-level docs](https://rerun.io/docs)
- [Rust API docs](https://docs.rs/rerun/)
- [Troubleshooting](https://www.rerun.io/docs/overview/installing-rerun/troubleshooting)

## Library
You can add the `rerun` crate to your project with `cargo add rerun`.

To get started, see [the examples](https://github.com/rerun-io/rerun/tree/latest/examples/rust).

## Native cryptographic provider

Rerun uses Ring by default for TLS and OAuth token verification.
To use AWS-LC instead, disable default features and select `crypto-aws-lc-rs` alongside the capabilities your application needs:

```toml
rerun = { version = "0.41", default-features = false, features = ["sdk", "native_viewer", "crypto-aws-lc-rs"] }
```

The same `crypto-ring` and `crypto-aws-lc-rs` features are available on `re_sdk`, `re_viewer`, `re_auth`, `rerun-cli`, and `rerun_py`.
Rerun initializes TLS before making requests and preserves a provider already installed by the embedding application.
If Cargo enables both provider features, AWS-LC is used for RSA verification and for TLS unless the application has already installed a provider.
Neither provider is built for browser targets.
Native OAuth RS256 verification requires one of the provider features; without either, RS256 verification returns an error.

AWS-LC has additional native build requirements; see its [installation documentation](https://aws.github.io/aws-lc-rs/requirements/).
Other dependencies in your application can still select Ring independently.
In particular, Lance currently does so: build `rerun_py` without its default `lance` feature to omit Ring, for example with `--no-default-features --features extension-module,crypto-aws-lc-rs`.
This configuration does not support Lance-backed local tables.
Standard Python wheels and CLI releases retain Ring and Lance support.

For source builds inside this repository, the ehttp patch in the workspace manifest is required until the upstream provider-selection change is released.
Cargo patches are not inherited by downstream workspaces; consumers testing this branch must also copy that patch into their workspace manifest.

## Binary
You can install the binary with `cargo install rerun-cli --locked --features nasm`.

**Note**: this requires the [`nasm`](https://github.com/netwide-assembler/nasm) CLI to be installed and available in your path.
Alternatively, you may skip enabling the `nasm` feature, but this may result in inferior video decoding performance.

The `rerun` CLI can act either as a server, a viewer, or both, depending on which options you use when you start it.

Running `rerun` with no arguments will start the viewer, waiting for an SDK to connect to it over gRPC.

Run `rerun --help` for more.

### Running a web viewer
The web viewer is an experimental feature, but you can try it out with:

```sh
rerun --web-viewer path/to/file.rrd
```
