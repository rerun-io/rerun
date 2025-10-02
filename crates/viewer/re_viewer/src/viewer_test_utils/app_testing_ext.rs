// #[allow(clippy::ptr_as_ptr)]

use std::sync::Arc;

use re_chunk::Chunk;
use re_chunk::ChunkBuilder;
use re_chunk::EntityPath;
use re_log_types::StoreId;
use re_log_types::StoreKind;
use re_viewer_context::DisplayMode;
use re_viewer_context::GlobalContext;
use re_viewer_context::StoreHub;
use re_viewer_context::ViewerContext;

use crate::{App, AppState};

pub trait AppTestingExt {
    fn testonly_state_mut(&mut self) -> &mut AppState;

    fn testonly_get_store_hub(&mut self) -> &mut StoreHub;

    fn testonly_set_test_hook(&mut self, func: Option<Box<dyn FnOnce(&ViewerContext<'_>)>>);

    fn testonly_init_recording(&mut self);

    fn testonly_log_entity(
        &mut self,
        entity_path: impl Into<EntityPath>,
        build_chunk: impl FnOnce(ChunkBuilder) -> ChunkBuilder,
    );
}

impl AppTestingExt for App {
    fn testonly_state_mut(&mut self) -> &mut AppState {
        &mut self.state
    }

    fn testonly_get_store_hub(&mut self) -> &mut StoreHub {
        self.store_hub
            .as_mut()
            .expect("store_hub should be initialized")
    }

    fn testonly_set_test_hook(&mut self, func: Option<Box<dyn FnOnce(&ViewerContext<'_>)>>) {
        self.state.test_hook = func;
    }

    fn testonly_init_recording(&mut self) {
        let store_hub = self
            .store_hub
            .as_mut()
            .expect("store_hub should be initialized");
        // store_hub.set_active_app("test_app".into());
        println!("Active recording: {:?}", store_hub.active_recording());
        store_hub.set_active_recording(StoreId::new(
            StoreKind::Recording,
            "test_app",
            "test_recording",
        ));
        println!("Active recording: {:?}", store_hub.active_recording());
    }

    /// Log an entity to the recording store.
    ///
    /// The provided closure should add content using the [`ChunkBuilder`] passed as argument.
    fn testonly_log_entity(
        &mut self,
        entity_path: impl Into<EntityPath>,
        build_chunk: impl FnOnce(ChunkBuilder) -> ChunkBuilder,
    ) {
        let builder = build_chunk(Chunk::builder(entity_path));
        let store_hub = self
            .store_hub
            .as_mut()
            .expect("store_hub should be initialized");
        let active_recording = store_hub
            .active_recording_mut()
            .expect("active_recording should be initialized");
        active_recording
            .add_chunk(&Arc::new(
                builder.build().expect("chunk should be successfully built"),
            ))
            .expect("chunk should be successfully added");
    }
}
