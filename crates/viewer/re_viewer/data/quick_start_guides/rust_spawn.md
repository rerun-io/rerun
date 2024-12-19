# Rust quick start

## Installing Rerun

After you have [installed the viewer](https://www.rerun.io/docs/getting-started/installing-viewer) you can simply add [the Rerun crate](https://crates.io/crates/rerun) to your project with `cargo add rerun`.

Let's try it out in a brand-new Rust project:

```sh
cargo init cube && cd cube && cargo add rerun
```

Note that the Rerun SDK requires a working installation of Rust 1.81+.

## Logging your own data

Add the following code to your `main.rs` file:

```rust
${EXAMPLE_CODE_RUST_SPAWN}
```

You can now run your application:

```shell
cargo run
```

Once everything finishes compiling, a new Viewer will start displaying these points:

![Demo recording](https://static.rerun.io/intro_rust_result/cc780eb9bf014d8b1a68fac174b654931f92e14f/768w.png)

${HOW_DOES_IT_WORK}
