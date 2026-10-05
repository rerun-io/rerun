use serde::{Deserialize, Serialize};

/// Commands that a control panel sends to the app.
///
/// Each WebSocket text frame carries one message as JSON, in serde's default externally tagged form,
/// e.g. `{"DynamicPosition": {"radius": 0.2, "offset": 1.0}}`.
/// The browser panel builds these by hand in `web/main.js`, so renaming a variant or field breaks it.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Message {
    Point3d {
        path: String,
        position: (f32, f32, f32),
        radius: f32,
    },
    Box3d {
        path: String,
        half_size: (f32, f32, f32),
        position: (f32, f32, f32),
    },
    DynamicPosition {
        radius: f32,
        offset: f32,
    },
}
