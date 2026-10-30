//! Exercises the public view harness and inspects clipped text output and chunk requests.

use re_chunk_store::Chunk;
use re_log_types::Timeline;
use re_sdk_types::Archetype as _;
use re_sdk_types::archetypes::TextLog;
use re_sdk_types::blueprint::archetypes::TextLogRows;
use re_test_context::TestContext;
use re_test_context::external::egui_kittest::Harness;
use re_test_viewport::TestContextExt as _;
use re_view_text_log::TextView;
use re_viewer_context::{BlueprintContext as _, ViewId};
use re_viewport_blueprint::ViewBlueprint;

/// Utility for testing text outputs.
#[derive(Debug, Default)]
pub struct PaintedText(Vec<egui::epaint::TextShape>);

impl PaintedText {
    fn from_output(output: &egui::FullOutput) -> Self {
        let mut painted = Self::default();
        for clipped in &output.shapes {
            painted.collect(&clipped.shape, clipped.clip_rect);
        }
        painted
    }

    pub fn contains(&self, label: &str) -> bool {
        self.0.iter().any(|text| text.galley.text() == label)
    }

    pub fn messages(&self) -> Vec<String> {
        self.0
            .iter()
            .map(|shape| shape.galley.text())
            .filter(|text| text.starts_with("message "))
            .map(str::to_owned)
            .collect()
    }

    pub fn assert_contains(&self, label: &str) {
        assert!(
            self.contains(label),
            "Expected {label:?} in painted text: {self:?}"
        );
    }

    pub fn assert_color(&self, label: &str, color: egui::Color32) {
        let text = self
            .0
            .iter()
            .find(|text| text.galley.text() == label)
            .unwrap_or_else(|| panic!("Expected {label:?} in painted text: {self:?}"));
        for section in &text.galley.job.sections {
            let actual = text.override_text_color.unwrap_or_else(|| {
                if section.format.color == egui::Color32::PLACEHOLDER {
                    text.fallback_color
                } else {
                    section.format.color
                }
            });
            assert_eq!(actual, color, "Unexpected color for {label:?}");
        }
    }

    fn collect(&mut self, shape: &egui::Shape, clip_rect: egui::Rect) {
        match shape {
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    self.collect(shape, clip_rect);
                }
            }
            egui::Shape::Text(text) if clip_rect.intersects(text.visual_bounding_rect()) => {
                self.0.push(text.clone());
            }
            _ => {}
        }
    }
}

/// Regular harness extended with some useful setup for testing text log views.
pub struct TextLogTestHarness {
    harness: Harness<'static, TextLogTestState>,
}

pub struct TextLogTestState {
    pub context: TestContext,
    pub view: ViewId,
    missing: Vec<re_chunk_store::ChunkId>,
    render_view: bool,
}

impl std::ops::Deref for TextLogTestHarness {
    type Target = TextLogTestState;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.harness.state()
    }
}

impl std::ops::DerefMut for TextLogTestHarness {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.harness.state_mut()
    }
}

impl TextLogTestHarness {
    pub fn new(manifest: Option<&[Chunk]>, resident: &[Chunk], time: i64) -> Self {
        use re_viewer_context::ViewClass as _;

        let mut context = TestContext::new_with_store_info_and_config(
            re_log_msg::StoreInfo::testing(),
            re_chunk_store::ChunkStoreConfig::COMPACTION_DISABLED,
        );
        context.register_view_class::<TextView>();
        if let Some(chunks) = manifest {
            context.add_rrd_manifest(
                re_chunk_index::RrdManifest::build_in_memory_from_chunks(
                    context.recording_store_id.clone(),
                    chunks.iter(),
                )
                .expect("valid test manifest"),
            );
        }
        context.add_chunks(resident.iter().cloned());
        context.set_active_timeline(*Timeline::log_tick().name());
        context.set_time(time);
        let view = context.setup_viewport_blueprint(|_, blueprint| {
            blueprint
                .add_view_at_root(ViewBlueprint::new_with_root_wildcard(TextView::identifier()))
        });
        let mut harness = Harness::builder()
            .with_size(egui::vec2(500.0, 300.0))
            .build_ui_state(
                |ui, state: &mut TextLogTestState| {
                    // Kittest's initialization frame must not advance view queries before frame().
                    if !state.render_view {
                        return;
                    }
                    egui::CentralPanel::default().show(ui, |ui| {
                        state.context.run(&ui.ctx().clone(), |ctx| {
                            ctx.recording()
                                .storage_engine()
                                .store()
                                .take_tracked_chunk_ids();
                            state.context.ui_for_single_view(ui, ctx, state.view);
                            let engine = ctx.recording().storage_engine();
                            let store = engine.store();
                            state.missing = store
                                .take_tracked_chunk_ids()
                                .missing_virtual
                                .keys()
                                .flat_map(|id| store.find_root_manifest_chunks(id))
                                .collect();
                        });
                    });
                },
                TextLogTestState {
                    context,
                    view,
                    missing: Vec::new(),
                    render_view: false,
                },
            );
        harness.state_mut().render_view = true;
        Self { harness }
    }

    pub fn loaded(chunks: &[Chunk], time: i64, manifest: bool) -> Self {
        Self::new(manifest.then_some(chunks), chunks, time)
    }

    /// Evicts all recording data, forwarding the store events to the caches like the viewer does.
    pub fn gc_everything(&mut self) {
        let state = self.harness.state_mut();
        let mut store_hub = state.context.store_hub.lock();
        let store_id = &state.context.recording_store_id;
        let events = store_hub
            .entity_db_mut(store_id)
            .expect("recording exists")
            .gc_with_target(re_chunk_store::GarbageCollectionTarget::Everything, None);
        assert!(!events.is_empty(), "the fixture must evict recording data");
        let (entity_db, cache) = store_hub
            .entity_db_and_cache(store_id, &state.context.view_class_registry)
            .expect("recording exists");
        cache.on_store_events(&events, entity_db);
    }

    pub fn set_level_filter(&self, labels: &[&str]) {
        self.context.with_blueprint_ctx(|ctx, _| {
            let path = {
                let engine = ctx.current_blueprint().storage_engine();
                re_viewport_blueprint::entity_path_for_view_property(
                    self.view,
                    engine.store().entity_tree(),
                    TextLogRows::name(),
                )
            };
            ctx.save_blueprint_archetype(
                path,
                &TextLogRows::new().with_filter_by_log_level(labels.iter().copied()),
            );
        });
        self.context.handle_system_commands(&self.harness.ctx);
    }

    pub fn refresh_blueprint(&mut self) {
        self.context.handle_system_commands(&self.harness.ctx);
        self.context.setup_viewport_blueprint(|_, _| {});
    }

    pub fn frame(&mut self) -> (PaintedText, Vec<re_chunk_store::ChunkId>) {
        self.frame_with_events(Vec::new())
    }

    #[track_caller]
    pub fn wait_until(&mut self, predicate: impl Fn(&PaintedText) -> bool) -> PaintedText {
        self.harness.step();
        re_ui::testing::step_until("text log rows", &mut self.harness, |harness| {
            predicate(&PaintedText::from_output(harness.output()))
        });
        PaintedText::from_output(self.harness.output())
    }

    pub fn missing_chunks(&self) -> &[re_chunk_store::ChunkId] {
        &self.missing
    }

    /// Supplies requested manifest chunks until the rendered output satisfies the predicate.
    #[track_caller]
    pub fn load_until(
        &mut self,
        chunks: &[Chunk],
        predicate: impl Fn(&PaintedText) -> bool,
    ) -> PaintedText {
        re_ui::testing::step_until("text log chunk loading", &mut self.harness, |harness| {
            if predicate(&PaintedText::from_output(harness.output())) {
                return true;
            }
            let state = harness.state_mut();
            state.context.add_chunks(
                chunks
                    .iter()
                    .filter(|chunk| state.missing.contains(&chunk.id()))
                    .cloned(),
            );
            false
        });
        PaintedText::from_output(self.harness.output())
    }

    #[track_caller]
    pub fn settle(&mut self) -> PaintedText {
        self.harness.run();
        PaintedText::from_output(self.harness.output())
    }

    #[track_caller]
    pub fn settle_scroll(&mut self) -> PaintedText {
        re_ui::testing::step_until("text log scroll", &mut self.harness, |harness| {
            !harness.ctx.input(|input| input.is_scrolling())
        });
        // Hover tooltips must not be mistaken for table rows in subsequent frames.
        self.frame_with_events(vec![egui::Event::PointerGone]).0
    }

    pub fn scroll(&mut self, delta: f32) {
        self.frame_with_events(vec![
            egui::Event::PointerMoved(egui::pos2(250.0, 150.0)),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                phase: egui::TouchPhase::Move,
                source: egui::MouseWheelSource::Unknown,
                delta: egui::vec2(0.0, delta),
                modifiers: egui::Modifiers::NONE,
            },
        ]);
    }

    fn frame_with_events(
        &mut self,
        events: Vec<egui::Event>,
    ) -> (PaintedText, Vec<re_chunk_store::ChunkId>) {
        self.harness.input_mut().events.extend(events);
        self.harness.step();
        (
            PaintedText::from_output(self.harness.output()),
            self.missing.clone(),
        )
    }
}

pub fn logs(times: impl IntoIterator<Item = i64>) -> Chunk {
    let mut builder = Chunk::builder("logs");
    for time in times {
        builder = builder.with_archetype_auto_row(
            [(Timeline::log_tick(), time)],
            &TextLog::new(format!("message {time}")),
        );
    }
    builder.build().expect("valid resident text fixture")
}
