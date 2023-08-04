use rerun::external::re_log_types::external::{arrow2, arrow2_convert};

// First define your own component type.
#[derive(arrow2_convert::ArrowField, arrow2_convert::ArrowSerialize)]
#[arrow_field(transparent)]
struct MyComponent(f32);

impl rerun::Component for MyComponent {
    fn name() -> rerun::ComponentName {
        "ext.my-component".into()
    }
}

// Then you can log it just like built-in components.
fn log_custom(session: &mut Session) -> Result<(), rerun::MsgSenderError> {
    MsgSender::new("your/entity/path")
        .with_splat(MyComponent(0.9))?
        .send(session)
}
