use super::protocol::ViewerEvent;
use tokio::sync::mpsc;

/// Handle for sending interaction events from the viewer to the application.
///
/// Cheap to clone and thread-safe.
#[derive(Clone)]
pub struct InteractionHandle {
    tx: mpsc::UnboundedSender<ViewerEvent>,
}

impl InteractionHandle {
    /// Create a new handle from a channel sender.
    pub fn new(tx: mpsc::UnboundedSender<ViewerEvent>) -> Self {
        Self { tx }
    }

    /// Send a click event to the application.
    pub fn send_click(
        &self,
        position: [f32; 3],
        entity_path: Option<String>,
        view_id: String,
        is_2d: bool,
    ) {
        let event = ViewerEvent::Click {
            position,
            entity_path,
            view_id,
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            is_2d,
        };

        if let Err(e) = self.tx.send(event) {
            eprintln!("Failed to send click event: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handle_send_click() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let handle = InteractionHandle::new(tx);

        handle.send_click(
            [1.0, 2.0, 3.0],
            Some("world/robot".to_string()),
            "view_123".to_string(),
            false,
        );

        let event = rx.try_recv().unwrap();
        match event {
            ViewerEvent::Click {
                position,
                entity_path,
                view_id,
                is_2d,
                ..
            } => {
                assert_eq!(position, [1.0, 2.0, 3.0]);
                assert_eq!(entity_path, Some("world/robot".to_string()));
                assert_eq!(view_id, "view_123");
                assert!(!is_2d);
            }
        }
    }

    #[test]
    fn test_handle_is_cloneable() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let handle1 = InteractionHandle::new(tx);
        let _handle2 = handle1.clone();
    }
}
