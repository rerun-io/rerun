//! Demonstrates the most barebone usage of the Rerun SDK.

use rerun::{MsgSender, Point3D, Session};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut session = Session::new();

    MsgSender::new("my_point")
        .with_component(&[Point3D::new(1., 1., 1.)])?
        .send(&mut session)?;

    session.show()?;

    Ok(())
}
