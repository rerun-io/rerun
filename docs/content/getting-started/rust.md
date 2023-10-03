---
title: Rust Quick Start
order: 2
---

## Installing Rerun
The Rerun SDK for Rust requires a working installation of Rust 1.72+.

To use Rerun, you need to install the `rerun` binary with `cargo install rerun-cli`, and [the rerun crate](https://crates.io/crates/rerun) with `cargo add rerun`.

Let's try it out in a brand new Rust project:
```bash
$ cargo init cube && cd cube && cargo add rerun
```

## Starting the viewer
Just run `rerun` to start the [Rerun Viewer](../reference/viewer/overview.md). It will wait for your application to log some data to it.

## Logging some data
Add the following code to your `main.rs`
(This example also lives in the `rerun` source tree [example](https://github.com/rerun-io/rerun/tree/latest/examples/rust/minimal/src/main.rs))
```rust
use rerun::{
    archetypes::Points3D, components::Color, demo_util::grid, external::glam,
    RecordingStreamBuilder,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = RecordingStreamBuilder::new("rerun_example_minimal_rs").memory()?;

    let points = grid(glam::Vec3::splat(-10.0), glam::Vec3::splat(10.0), 10);
    let colors = grid(glam::Vec3::ZERO, glam::Vec3::splat(255.0), 10)
        .map(|v| Color::from_rgb(v.x as u8, v.y as u8, v.z as u8));

    rec.log(
        "my_points",
        &Points3D::new(points).with_colors(colors).with_radii([0.5]),
    )?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
```

Now run your application:
```
cargo run
```

Once everything finishes compiling, you will see the points in the Rerun Viewer:

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/intro_users1_result/40dca5343e79c4a214fdac277dc601c3da8fb491/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/intro_users1_result/40dca5343e79c4a214fdac277dc601c3da8fb491/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/intro_users1_result/40dca5343e79c4a214fdac277dc601c3da8fb491/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/intro_users1_result/40dca5343e79c4a214fdac277dc601c3da8fb491/1200w.png">
  <img src="https://static.rerun.io/intro_users1_result/40dca5343e79c4a214fdac277dc601c3da8fb491/full.png" alt="Rust getting started result">
</picture>


## Using the viewer
Try out the following to interact with the viewer:
 * Click and drag in the main view to rotate the cube.
 * Zoom in and out with the scroll wheel.
 * Mouse over the "?" icons to find out about more controls.
 * Click on the cube to select all of the points.
 * Hover and select individual points to see more information.

If you're facing any difficulties, don't hesitate to [open an issue](https://github.com/rerun-io/rerun/issues/new/choose) or [join the Discord server](https://discord.gg/PXtCgFBSmH).

## What's next

If you're ready to move on to more advanced topics, check out the [Viewer Walkthrough](viewer-walkthrough.md) or our
more advanced guide for [Logging Data in Rust](logging-rust.md) where we will explore the core concepts that make
Rerun tick and log our first non-trivial dataset.

If you'd rather learn from examples, check out the [example gallery](/examples) for some more realistic examples, or browse the [Loggable Data Types](../reference/data_types.md) section for more simple examples of how to use the main data types.
