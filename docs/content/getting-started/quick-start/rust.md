---
title: Rust
order: 3
---

## Setup

The Rerun SDK for Rust requires a working installation of Rust 1.81+.

After you have [installed the viewer](../installing-viewer.md#installing-the-viewer) you can simply add [the Rerun crate](https://crates.io/crates/rerun) to your project with `cargo add rerun`.

Let's try it out in a brand new Rust project:

```bash
$ cargo init cube && cd cube && cargo add rerun
```

## Logging some data

Add the following code to your `main.rs`
(This example also lives in the `rerun` source tree [example](https://github.com/rerun-io/rerun/tree/latest/examples/rust/minimal/src/main.rs))

```rust
use rerun::{demo_util::grid, external::glam};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_minimal").spawn()?;

    let points = grid(glam::Vec3::splat(-10.0), glam::Vec3::splat(10.0), 10);
    let colors = grid(glam::Vec3::ZERO, glam::Vec3::splat(255.0), 10)
        .map(|v| rerun::Color::from_rgb(v.x as u8, v.y as u8, v.z as u8));

    rec.log(
        "my_points",
        &rerun::Points3D::new(points)
            .with_colors(colors)
            .with_radii([0.5]),
    )?;

    Ok(())
}
```

Now run your application:

```
cargo run
```

Once everything finishes compiling, you will see the points in the Rerun Viewer:

<picture>
  <img src="https://static.rerun.io/intro_rust_result/cc780eb9bf014d8b1a68fac174b654931f92e14f/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/intro_rust_result/cc780eb9bf014d8b1a68fac174b654931f92e14f/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/intro_rust_result/cc780eb9bf014d8b1a68fac174b654931f92e14f/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/intro_rust_result/cc780eb9bf014d8b1a68fac174b654931f92e14f/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/intro_rust_result/cc780eb9bf014d8b1a68fac174b654931f92e14f/1200w.png">
</picture>

## Using the Viewer

Try out the following to interact with the viewer:

-   Click and drag in the main view to rotate the cube.
-   Zoom in and out with the scroll wheel.
-   Mouse over the "?" icons to find out about more controls.
-   Click on the cube to select all of the points.
-   Hover and select individual points to see more information.

If you're facing any difficulties, don't hesitate to [open an issue](https://github.com/rerun-io/rerun/issues/new/choose) or [join the Discord server](https://discord.gg/PXtCgFBSmH).

## What's next

If you're ready to move on to more advanced topics, check out the [Viewer Walkthrough](../navigating-the-viewer.md) or our
more advanced guide for [Logging Data in Rust](../data-in/rust.md) where we will explore the core concepts that make
Rerun tick and log our first non-trivial dataset.

If you'd rather learn from examples, check out the [example gallery](/examples) for some more realistic examples, or browse the [Types](../../reference/types.md) section for more simple examples of how to use the main datatypes.
