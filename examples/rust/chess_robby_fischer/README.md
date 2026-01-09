<!--[metadata]
title = "Robby fischer"
tags = ["3D", "URDF", "Blueprint"]
source = "https://github.com/02alexander/robby-fischer/tree/urdf-vis"
thumbnail = "https://static.rerun.io/robby_thumbnail/71d2d57e9720e7a96e35a43467b5d2c45aa716d9/480w.png"
thumbnail_dimensions = [480, 385]
-->

https://vimeo.com/989548054?autoplay=1&loop=1&autopause=0&background=1&muted=1&ratio=1920:1080

## Used Rerun types

[`Transform3D`](https://www.rerun.io/docs/reference/types/archetypes/transform3d), [`Points2D`](https://www.rerun.io/docs/reference/types/archetypes/points2d), [`Boxes3D`](https://www.rerun.io/docs/reference/types/archetypes/boxes3d), [`Pinhole`](https://www.rerun.io/docs/reference/types/archetypes/pinhole), [`Image`](https://www.rerun.io/docs/reference/types/archetypes/image), [`Mesh3D`](https://www.rerun.io/docs/reference/types/archetypes/mesh3d), [`LineStrips3D`](https://www.rerun.io/docs/reference/types/archetypes/line_strips3d),

## Background

Robby Fischer is an autonomous robot arm that you can play chess against, created by Alexander Berntsson and Herman Lauenstein. It detects the human's move by using a camera that watches which squares has a piece on it and what color that piece is. It doesn't need to see recognize different roles (pawn, rook, etc..) visually because it knows the start position so it can always figure out what piece stand on what square. However, this is a bit problematic if the human promotes a pawn because robot must figure out which piece the pawn was promoted to. This is why it also looks at the adjacent white board, where it has a specific location associated with each piece, so if the human promotes to a queen the queen square will be empty and Robby can figure out that the human promoted to a queen.

To find out if a piece stands on a square we must determine what part of the image may only contain the piece that stands on that square. This is necessary to deal with the fact that some pieces are tall and block part of adjacent squares, e.g. if a king stands on `e2`, its head will block part of the `e1` square in the image. The mask that determines this is logged to `images/mask` and is shown in the bottom left corner along with the detected pieces.

## Logging and visualizing with Rerun

### Create recording

First we create the recording and store it as the thread local recording in each thread.

```rust

let app_id = "RobbyFischer";
let rec_id = uuid::Uuid::new_v4().to_string();
let rec = rerun::RecordingStreamBuilder::new(app_id)
    .recording_id(&rec_id)
    .connect_grpc()
    .unwrap();

// …

// Will be retrieved later using `rerun::RecordingStream::thread_local`
RecordingStream::set_thread_local(rerun::StoreKind::Recording, Some(rec.clone()));

// …

// Thread that does all the image processing.
let to_be_moved_rec = rec.clone();
let _vision_handle = std::thread::spawn(move || {
    RecordingStream::set_thread_local(rerun::StoreKind::Recording, Some(to_be_moved_rec));
    // …
});
```

### Robot arm

Then, we install the official [URDF dataloader](https://github.com/rerun-io/rerun-loader-python-example-urdf) and use it to log the URDF model.

```rust
// Rerun will find the dataloader in the `PATH` and use it to log `arm.urdf`.
rec.log_file_from_path("arm.urdf", None, None, false).unwrap();

// Sets the position of the arm and rotates it 180 degrees.
rec.log(
    "arm.urdf",
    &rerun::Transform3D::from_translation_rotation(
        [-0.185, 0.130, 0.04],
        Rotation3D::AxisAngle(RotationAxisAngle::new([0., 0., 1.], Angle::Degrees(180.0))),
    ),
)
```

To log joint positions we must convert the joint positions to link transformations, we will do this with the help of the [k](https://docs.rs/k/latest/k/) crate which supports forward kinematics and is capable of loading URDF files.

```rust

let chain = k::Chain::<f32>::from_urdf_file(URDF_PATH).unwrap();

// …

chain.set_joint_positions(&positions).unwrap();
chain.update_transforms();

for link_name in chain.iter_links().map(|link| link.name.clone()) {
    // …
    // Extracts translation and rotation of `link_name` relative to it's parent and it's entity_path.
    // …

    rec.log(
        entity_path,
        &rerun::Transform3D::from_translation_rotation(
            Vec3D::new(translation[0], translation[1], translation[2]),
            Rotation3D::Quaternion(Quaternion(quat.coords.as_slice().try_into().unwrap())),
        ),
    ).unwrap();
}
```

It's planned trajectory is visualized using [LineStrips3D](https://rerun.io/docs/reference/types/archetypes/line_strips3d).

```rust
let strip: Vec<Vec3> = // …
rec.log(
    "a8origin/trajectory",
    &rerun::LineStrips3D::new([strip])
        .with_radii([rerun::Radius::new_scene_units(0.002)]),
).unwrap();

// Move arm along trajectory …

// Remove trajectory after we've moved along it.
rec.log("a8origin/trajectory", &rerun::Clear::flat())
    .unwrap();

```

### Board and pieces

First it logs the position and meshes of the boards.

```rust
// The board is stored as a GLTF file so we use the `log_node` function from this example: https://github.com/rerun-io/rerun/tree/main/examples/rust/raw_mesh to log it.
log_node(rec, "a8origin/board", self.board_scene.clone()).unwrap();

// Align the board mesh so that the origin of `a8origin` appears on the a8 square.
// Henceforth any positions logged to a8origin/ will be relative to the center of the a8 square.
rec.log(
    "a8origin/board",
    &rerun::Transform3D::from_translation(Into::<[f32; 3]>::into(board_center)),
).unwrap();

// `holder` refers to the adjacent white board that holds captured pieces.
rec.log(
    "a8origin/holder",
    &rerun::Transform3D::from_translation(Into::<[f32; 3]>::into(
        board_center + board_center_to_holder_center,
    )),
).unwrap();
rec.log("a8origin/holder/mesh", &self.holder_mesh).unwrap();
```

Then we log the position for the center of each square, including squares on the holder board.

```rust
for file in 0..14 { // The holder board has 6 files
    for rank in 0..8 {
        let cord = board_to_real_cord(Square::new(file, rank));
        rec.log(
            format!("a8origin/pieces/{file}/{rank}/"),
            &rerun::Transform3D::from_translation_rotation_scale(
                cord,
                rerun::Rotation3D::IDENTITY,

                // The models for the pieces are stored in millimeters but
                // the rest in meters, so we must scale the models down.
                Scale3D::Uniform(0.001),
            ),
        ).unwrap();
    }
}
```

To log the piece models we convert them from `.stl` files to [`rerun::Mesh3D`](https://www.rerun.io/docs/reference/types/archetypes/mesh3d) by first reading the `.stl` files using [stl_io](https://docs.rs/stl_io/latest/stl_io/) and then convert them to [`rerun::Mesh3D`](https://www.rerun.io/docs/reference/types/archetypes/mesh3d) using the function below.

```rust
fn stl_to_mesh3d(mesh: &IndexedMesh, color: impl Into<rerun::Color> + Clone) -> Mesh3D {
    // The normals are not included in the stl files so we have to compute them
    // ourselves here. It's not strictly necessary to pass the normals
    // to Mesh3D but makes it the models look so much better.

    // calculate normals …

    rerun::Mesh3D::new(vertices)
        .with_triangle_indices(mesh.faces.iter().map(|face| {
            rerun::TriangleIndices(UVec3D::new(
                face.vertices[0] as u32,
                face.vertices[1] as u32,
                face.vertices[2] as u32,
            ))
        }))
        .with_vertex_colors(std::iter::repeat_n(color, mesh.vertices.len()))
        .with_vertex_normals(normals)
}
```

Every time we make a move we log the changes to like this:

```rust
pub fn log_piece_positions(&self, board: &Board) {
    let rec = rerun::RecordingStream::thread_local(rerun::StoreKind::Recording).unwrap();
    for file in 0..14 {
        for rank in 0..8 {
            if let Some(piece) = board.position[file][rank] {
                if piece.role != Role::Duck {
                    let piece_model_info = self.piece_meshes.get(&piece).unwrap();

                    // Calls the log method defined below.
                    piece_model_info.log(&rec, &format!("a8origin/pieces/{file}/{rank}"));
                }
            } else {
                // To remove the bounding box/mesh from the square we moved the piece from.
                // You could clear both in one call using rerun::Clear::recursive but that
                // would clear the transformation logged to "a8origin/pieces/{file}/{rank}"
                // which is why it isn't done here.
                rec.log(
                    format!("a8origin/pieces/{file}/{rank}/mesh"),
                    &rerun::Clear::flat(),
                ).unwrap();
                rec.log(
                    format!("a8origin/pieces/{file}/{rank}/bounding_box"),
                    &rerun::Clear::flat(),
                ).unwrap();
            }
        }
    }
}

// …

impl PieceModelInfo {
    pub fn log(&self, rec: &rerun::RecordingStream, entity_path: &str) {
        self.bounding_box
            .log(rec, &format!("{entity_path}/bounding_box"));
        rec.log(format!("{entity_path}/mesh"), &self.model).unwrap();
    }
}
```

### Image

To see the image projection in the 3D view we must log the cameras transformation and it's intrinsic parameters.

```rust
// Computes the transformation `camera_to_a8` that goes from camera coordinates to board coordinates using the fiducial markers located at the corners of the board.
// …

let (_scale, rotation, translation) = camera_to_a8.to_scale_rotation_translation();
rec.log(
    "a8origin/pinhole",
    &rerun::Transform3D::from_translation_rotation(translation, rotation),
).unwrap();
rec.log(
    "a8origin/pinhole",
    &rerun::Pinhole::from_focal_length_and_resolution(
        [color_param.row(0)[0], color_param.row(1)[1]],
        [640.0, 480.0],
    ),
).unwrap();
```

Then we can log the image to this path and it will be shown in the 3D view.

```rust
rec.log(
    "a8origin/pinhole/image",
    &rerun::Image::try_from(color_img.clone()).unwrap(),
).unwrap();
```

Logs the mask along with the detected pieces.

```rust
rec.log("images/mask", &rerun::Image::try_from(mask).unwrap())
                .unwrap();
rec.log(
    "images/points",
    &Points2D::new(square_mid_points)
        .with_labels(self.count_avg.iter().map(|cnt| cnt.to_string()))
        .with_radii(with_pieces.iter().map(|b| if *b { 10.0 } else { 2.0 }))
        .with_colors(is_white.iter().map(|&w| if w { [220; 3] } else { [50; 3] })),
).unwrap();
```

### Blueprint

As of writing, there isn't a Rust API for blueprints so instead we have to create and log the blueprint from python (see [#5521](https://github.com/rerun-io/rerun/issues/5521)).
This was done by creating a script called "blueprint.py"

```py
#!/usr/bin/env python3

import rerun as rr
import rerun.blueprint as rrb
import argparse

view_defaults = [
    rr.components.AxisLength(0.0), # To hide the axes of all the transformations.
    rr.components.ImagePlaneDistance(0.3),
]

blueprint = rrb.Blueprint(
    rrb.Horizontal(
        rrb.Vertical(
            rrb.Spatial2DView(
                origin="a8origin/pinhole/image"
            ),
            rrb.Spatial2DView(
                contents=[
                    "images/mask",
                    "images/points",
                ]
            ),
        ),
        rrb.Vertical(
            rrb.Spatial2DView(
                origin="external_camera",
            ),
            # View that follows the claw
            rrb.Spatial3DView(
                origin="/arm.urdf/base_link/glid_platta_1/bas_1/gemensam_vagg_1/botten_snurr_1/kortarm_kopia_1/led_1/led_axel_1/lang_arm_1/mount_1/ram_1",
                contents="/**",
                defaults=view_defaults
            )
        ),
        rrb.Spatial3DView(
            defaults=view_defaults
        ),
        column_shares=[2,2,3]
    ),
    auto_views=False,
    collapse_panels=True,
)

parser = argparse.ArgumentParser()
parser.add_argument("--recording-id", type=str)
parser.add_argument("--application-id", type=str)

args = parser.parse_args()
rr.init(args.application_id, recording_id=args.recording_id)
rr.connect_grpc()
rr.send_blueprint(blueprint)
```

and then running it from rust like this:

```rust
// Creates and logs blueprint.
std::process::Command::new("../../blueprint.py")
    .arg("--recording-id")
    .arg(&rec_id)
    .arg("--application-id")
    .arg(&app_id)
    .spawn()
```
