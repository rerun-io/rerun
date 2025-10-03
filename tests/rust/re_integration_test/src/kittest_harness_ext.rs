use std::sync::Arc;

use egui_kittest::kittest::Queryable as _;
use re_sdk::{
    Component as _, ComponentDescriptor, EntityPath, EntityPathPart, RecordingInfo, StoreId,
    StoreKind,
    external::{
        re_log_types::{SetStoreInfo, StoreInfo},
        re_tuid::Tuid,
    },
    log::Chunk,
};
use re_viewer::{
    SystemCommand, SystemCommandSender as _,
    external::{
        re_chunk::{ChunkBuilder, LatestAtQuery},
        re_entity_db::EntityDb,
        re_types,
        re_viewer_context::{self, ViewerContext, blueprint_timeline},
    },
    viewer_test_utils::AppTestingExt as _,
};
use re_viewport_blueprint::ViewportBlueprint;

pub trait HarnessExt {
    fn clear_current_blueprint(&mut self);

    fn setup_viewport_blueprint(
        &mut self,
        setup_blueprint: impl FnOnce(&ViewerContext<'_>, &mut ViewportBlueprint) + 'static,
    );

    fn run_with_viewer_context(&mut self, func: impl FnOnce(&ViewerContext<'_>) + 'static);

    fn log_entity(
        &mut self,
        entity_path: impl Into<EntityPath>,
        build_chunk: impl FnOnce(ChunkBuilder) -> ChunkBuilder,
    );

    fn init_recording(&mut self);

    fn click_label(&mut self, label: &str);

    #[allow(unused)]
    fn debug_viewer_state(&mut self);

    fn toggle_blueprint_panel(&mut self) {
        self.click_label("Blueprint panel toggle");
    }

    fn toggle_time_panel(&mut self) {
        self.click_label("Time panel toggle");
    }

    fn toggle_selection_panel(&mut self) {
        self.click_label("Selection panel toggle");
    }

    fn init_recording_environment(&mut self) {
        self.toggle_blueprint_panel();
        self.toggle_time_panel();
        self.toggle_selection_panel();
        self.init_recording();
    }
}

impl HarnessExt for egui_kittest::Harness<'_, re_viewer::App> {
    fn clear_current_blueprint(&mut self) {
        self.setup_viewport_blueprint(|_viewer_context, blueprint| {
            for item in blueprint.contents_iter() {
                blueprint.remove_contents(item);
            }
        });
    }

    fn setup_viewport_blueprint(
        &mut self,
        setup_blueprint: impl FnOnce(&ViewerContext<'_>, &mut ViewportBlueprint) + 'static,
    ) {
        self.run_with_viewer_context(|viewer_context| {
            let blueprint_query = LatestAtQuery::latest(blueprint_timeline());
            let mut viewport_blueprint =
                ViewportBlueprint::from_db(viewer_context.blueprint_db(), &blueprint_query);
            setup_blueprint(viewer_context, &mut viewport_blueprint);
            viewport_blueprint.save_to_blueprint_store(viewer_context);
        });
    }

    fn run_with_viewer_context(&mut self, func: impl FnOnce(&ViewerContext<'_>) + 'static) {
        self.state_mut()
            .testonly_set_test_hook(Some(Box::new(func)));
        self.run_ok();
    }

    fn log_entity(
        &mut self,
        entity_path: impl Into<EntityPath>,
        build_chunk: impl FnOnce(ChunkBuilder) -> ChunkBuilder,
    ) {
        let app = self.state_mut();
        let builder = build_chunk(Chunk::builder(entity_path));
        let store_hub = app.testonly_get_store_hub();
        let active_recording = store_hub
            .active_recording_mut()
            .expect("active_recording should be initialized");
        active_recording
            .add_chunk(&Arc::new(
                builder.build().expect("chunk should be successfully built"),
            ))
            .expect("chunk should be successfully added");
    }

    fn init_recording(&mut self) {
        let app = self.state_mut();
        let store_hub = app.testonly_get_store_hub();

        let store_info = StoreInfo::testing();
        let application_id = store_info.application_id().clone();
        let recording_store_id = store_info.store_id.clone();
        let mut recording_store = EntityDb::new(recording_store_id.clone());

        recording_store.set_store_info(SetStoreInfo {
            row_id: Tuid::new(),
            info: store_info,
        });
        {
            // Set RecordingInfo:
            recording_store
                .set_recording_property(
                    EntityPath::properties(),
                    RecordingInfo::descriptor_name(),
                    &re_types::components::Name::from("Test recording"),
                )
                .expect("Failed to set recording name");
            recording_store
                .set_recording_property(
                    EntityPath::properties(),
                    RecordingInfo::descriptor_start_time(),
                    &re_types::components::Timestamp::now(),
                )
                .expect("Failed to set recording start time");
        }
        {
            // Set some custom recording properties:
            recording_store
                .set_recording_property(
                    EntityPath::properties() / EntityPathPart::from("episode"),
                    ComponentDescriptor {
                        archetype: None,
                        component: "location".into(),
                        component_type: Some(re_types::components::Text::name()),
                    },
                    &re_types::components::Text::from("Swallow Falls"),
                )
                .expect("Failed to set recording property");
            recording_store
                .set_recording_property(
                    EntityPath::properties() / EntityPathPart::from("episode"),
                    ComponentDescriptor {
                        archetype: None,
                        component: "weather".into(),
                        component_type: Some(re_types::components::Text::name()),
                    },
                    &re_types::components::Text::from("Cloudy with meatballs"),
                )
                .expect("Failed to set recording property");
        }

        let blueprint_id = StoreId::random(StoreKind::Blueprint, application_id);
        let blueprint_store = EntityDb::new(blueprint_id.clone());

        store_hub.insert_entity_db(recording_store);
        store_hub.insert_entity_db(blueprint_store);
        store_hub.set_active_recording_id(recording_store_id.clone());
        store_hub
            .set_cloned_blueprint_active_for_app(&blueprint_id)
            .expect("Failed to set blueprint as active");

        app.command_sender.send_system(SystemCommand::SetSelection(
            re_viewer_context::Item::StoreId(recording_store_id.clone()).into(),
        ));
        self.run_ok();
    }

    fn click_label(&mut self, label: &str) {
        self.get_by_label(label).click();
        self.run_ok();
    }

    fn debug_viewer_state(&mut self) {
        println!(
            "Active recording: {:#?}",
            self.state_mut().testonly_get_store_hub().active_recording()
        );
        println!(
            "Active blueprint: {:#?}",
            self.state_mut().testonly_get_store_hub().active_blueprint()
        );
        self.setup_viewport_blueprint(|_viewer_context, blueprint| {
            println!("Blueprint view count: {}", blueprint.views.len());
            for id in blueprint.view_ids() {
                println!("View id: {id}");
            }
            println!(
                "Display mode: {:?}",
                _viewer_context.global_context.display_mode
            );
        });
    }
}
