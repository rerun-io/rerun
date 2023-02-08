//! Demonstrates the most barebone usage of the Rerun SDK.

use rerun::{MsgSender, MsgSenderError, Point3D, Session};

fn main() -> Result<(), MsgSenderError> {
    let mut session = Session::new();

    MsgSender::new("my_point")
        .with_component(&[Point3D::new(1., 1., 1.)])?
        .send(&mut session)?;

    // TODO: make that friendlier for the first impression
    let log_messages = session.drain_log_messages_buffer();
    rerun::viewer::show(log_messages).unwrap();

    Ok(())
}
