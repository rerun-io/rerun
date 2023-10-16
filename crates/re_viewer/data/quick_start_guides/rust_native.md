## Rust Quick Start

### Installing Rerun

To use the Rerun SDK in your project, you need the [rerun crate](https://crates.io/crates/rerun) which you can add with `cargo add rerun`.

Let's try it out in a brand-new Rust project:

```sh
cargo init cube && cd cube && cargo add rerun --features native_viewer
```

Note that the Rerun SDK requires a working installation of Rust 1.72+.

### Logging your own data

Add the following code to your `main.rs` file:

```rust
use rerun::{demo_util::grid, external::glam};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a new `RecordingStream` which sends data over TCP to the viewer process.
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_demo_rs")
        .connect("127.0.0.1:9876".parse()?, None)?;

    // Create some data using the `grid` utility function.
    let points = grid(glam::Vec3::splat(-10.0), glam::Vec3::splat(10.0), 10);
    let colors = grid(glam::Vec3::ZERO, glam::Vec3::splat(255.0), 10)
        .map(|v| rerun::Color::from_rgb(v.x as u8, v.y as u8, v.z as u8));

    // Log the "my_points" entity with our data, using the `Points3D` archetype.
    rec.log(
        "my_points",
        &rerun::Points3D::new(points)
            .with_colors(colors)
            .with_radii([0.5]),
    )?;

    Ok(())
}
```

You can now run your application:

```shell
cargo run
```

Once everything finishes compiling, you will see the points in this viewer:

![Demo recording](https://static.rerun.io/intro_rust_result/cc780eb9bf014d8b1a68fac174b654931f92e14f/768w.png)
