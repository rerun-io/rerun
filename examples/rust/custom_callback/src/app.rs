//! The external application that will be controlled by the extended viewer ui.

use core::f32;
use std::f32::consts::{PI, TAU};

use custom_callback::comms::{app::ControlApp, protocol::Message};

use rerun::{
    RecordingStream,
    external::{glam::Vec3, re_log, tokio},
};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut app = ControlApp::bind("127.0.0.1:8888").await?.run();
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_custom_callback")
        .connect_grpc_opts(
            "rerun+http://127.0.0.1:9877/proxy",
            rerun::default_flush_timeout(),
        )?;

    // Add a handler for incoming messages
    let add_rec = rec.clone();
    app.add_handler(move |msg| handle_message(&add_rec, msg))?;

    // spawn a task to log a point every 100ms
    // we then use a channel to control the point's position and radius using the control panel
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let snake_handle = tokio::spawn(animated_snake(rx, rec));

    // Add a handler for dynamic updates
    app.add_handler(move |msg| handle_dynamic_update(tx.clone(), msg))?;

    // Keep the server running
    tokio::signal::ctrl_c().await?;
    re_log::info!("Shutting down");
    snake_handle.abort();

    Ok(())
}

fn handle_dynamic_update(tx: UnboundedSender<Message>, message: &Message) {
    if let Message::DynamicPosition { .. } = message {
        tx.send(message.clone()).expect("failed to send message");
    }
}

fn handle_message(rec: &RecordingStream, message: &Message) {
    match message {
        Message::Point3d {
            path,
            position,
            radius,
        } => rec.log(
            path.to_string(),
            &rerun::Points3D::new([position]).with_radii([*radius]),
        ),
        Message::Box3d {
            path,
            half_size,
            position,
        } => rec.log(
            path.to_string(),
            &rerun::Boxes3D::from_half_sizes([half_size]).with_centers([position]),
        ),
        Message::Disconnect => {
            re_log::info!("Client disconnected");
            Ok(())
        }
        _ => Ok(()),
    }
    .expect("failed to handle message");
}

async fn animated_snake(mut rx: UnboundedReceiver<Message>, rec: RecordingStream) {
    let mut current_radius = 0.1;
    let mut current_offset = 0.5;

    let mut t = 0.0_f32;
    loop {
        // update the position and radius
        if let Ok(Message::DynamicPosition { radius, offset }) = rx.try_recv() {
            // ensure these values are never zero
            current_offset = offset.max(0.01);
            current_radius = radius.max(0.01);
        }

        let num_spheres = ((PI * current_offset) / current_radius.max(f32::EPSILON)).max(1.);
        let theta = TAU / num_spheres;

        let total_spheres = ((num_spheres as usize) / 3).max(1);
        let mut points = Vec::with_capacity(total_spheres);
        t -= (total_spheres - 1) as f32 * theta;

        for _ in 0..total_spheres {
            let x = current_offset * t.cos();
            let y = current_offset * t.sin();
            let z = 0.0;

            points.push(Vec3::new(x, y, z));

            t += theta;
        }

        // log the point
        rec.log(
            "dynamic".to_string(),
            &rerun::Points3D::new(points).with_radii(vec![current_radius; num_spheres as usize]),
        )
        .expect("failed to log dynamic");

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
}
