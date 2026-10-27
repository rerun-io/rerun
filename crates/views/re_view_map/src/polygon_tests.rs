use super::*;
use re_log_types::{TimeInt, TimePoint, Timeline};
use re_sdk_types::RowId;
use re_sdk_types::archetypes::GeoPolygons;
use re_sdk_types::components::{Color, GeoPolygon};
use re_test_context::TestContext;
use re_test_viewport::TestContextExt as _;
use re_viewport_blueprint::ViewBlueprint;

#[test]
fn polygon_holes_transparency_and_timeline_updates() {
    // Match native App startup before constructing reqwest 0.13's tile client.
    #[cfg(not(target_arch = "wasm32"))]
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
    let mut test_context = TestContext::new_with_view_class::<MapView>();
    let timeline = Timeline::new_sequence("frame");
    test_context.set_active_timeline(*timeline.name());
    let frame = |value: i64| {
        TimePoint::default().with(timeline, TimeInt::from_sequence(value.try_into().unwrap()))
    };
    let polygon = GeoPolygon::from_lat_lon([
        vec![
            [0.0, 0.0],
            [0.0, 6.0],
            [2.0, 6.0],
            [2.0, 2.0],
            [6.0, 2.0],
            [6.0, 0.0],
        ],
        vec![[0.5, 0.5], [0.5, 1.5], [1.5, 1.5], [1.5, 0.5]],
    ]);
    let overlapping = GeoPolygon::from_lat_lon([[[2.5, 1.0], [2.5, 4.0], [4.5, 4.0], [4.5, 1.0]]]);
    test_context.log_entity("regions", |builder| {
        builder.with_archetype(
            RowId::new(),
            frame(1),
            &GeoPolygons::new([polygon, overlapping]).with_colors([
                Color::from_unmultiplied_rgba(220, 60, 50, 160),
                Color::from_unmultiplied_rgba(40, 100, 220, 160),
            ]),
        )
    });
    test_context.log_entity("regions", |builder| {
        builder.with_archetype(
            RowId::new(),
            frame(2),
            &GeoPolygons::new([GeoPolygon::from_lat_lon([[
                [0.0, 0.0],
                [0.0, 6.0],
                [6.0, 0.0],
            ]])])
            .with_colors([Color::from_rgb(30, 180, 80)]),
        )
    });
    test_context.log_entity("regions", |builder| {
        builder.with_archetype(
            RowId::new(),
            frame(3),
            &GeoPolygons::update_fields().with_colors([Color::from_rgb(230, 180, 30)]),
        )
    });
    test_context.log_entity("regions", |builder| {
        builder.with_archetype(
            RowId::new(),
            frame(4),
            &GeoPolygons::new(Vec::<GeoPolygon>::new()),
        )
    });
    let view_id = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(MapView::identifier()))
    });
    test_context.set_time(1);
    let mut harness = test_context
        .setup_kittest_for_rendering_3d(egui::vec2(320.0, 320.0))
        .build_ui(|ui| {
            {
                let mut states = test_context.view_states.lock();
                let state =
                    states.get_mut_or_create(&test_context.recording_store_id, view_id, &MapView);
                let state = state.as_any_mut().downcast_mut::<MapViewState>().unwrap();
                // Initialize textures on this harness's context and disable external tile downloads.
                if state.tiles.is_none() {
                    state.tiles = Some(HttpTiles::with_options(
                        walkers::sources::OpenStreetMap,
                        walkers::HttpOptions {
                            max_parallel_downloads: walkers::MaxParallelDownloads(0),
                            ..Default::default()
                        },
                        ui.ctx().clone(),
                    ));
                }
            }
            test_context.run_with_single_view(ui, view_id);
        });
    harness.run();
    harness.snapshot("geo_polygons_holes_alpha");
    let mesh_stats = || {
        let states = test_context.view_states.lock();
        let state = states
            .get(&test_context.recording_store_id, view_id)
            .unwrap();
        let state = state.as_any().downcast_ref::<MapViewState>().unwrap();
        (
            state.polygon_draw_cache.mesh_count(),
            state.polygon_draw_cache.upload_count(),
        )
    };
    assert_eq!(mesh_stats(), (2, 2));
    let map_rect = harness
        .output()
        .shapes
        .iter()
        .find_map(|shape| {
            if let egui::Shape::Callback(callback) = &shape.shape {
                Some(callback.rect)
            } else {
                None
            }
        })
        .expect("MapView must produce a native rendering callback");
    let [hole, outside, red, blue, overlap] = {
        let states = test_context.view_states.lock();
        let state = states
            .get(&test_context.recording_store_id, view_id)
            .unwrap();
        let state = state.as_any().downcast_ref::<MapViewState>().unwrap();
        let projector =
            walkers::Projector::new(map_rect, &state.map_memory, state.last_center_position);
        [[1.0, 1.0], [5.0, 5.0], [3.0, 0.5], [3.0, 3.0], [3.0, 1.5]].map(|[lat, lon]| {
            let position = projector.project(walkers::lat_lon(lat, lon));
            egui::pos2(position.x, position.y)
        })
    };
    harness.step();
    let image = harness.render().unwrap();
    let scale = harness.ctx.pixels_per_point();
    let pixel =
        |point: egui::Pos2| *image.get_pixel((point.x * scale) as u32, (point.y * scale) as u32);
    assert_eq!(
        pixel(hole),
        pixel(outside),
        "The hole must retain the map background"
    );
    assert_ne!(pixel(red), pixel(outside), "The exterior must be filled");
    assert_ne!(pixel(overlap), pixel(red));
    assert_ne!(
        pixel(overlap),
        pixel(blue),
        "Transparent overlap must blend both polygons"
    );
    for (position, expected_instance) in [(red, Some(0u64)), (overlap, Some(1u64)), (hole, None)] {
        harness.hover_at(position);
        // Native picking is read back asynchronously. Rendering waits for submitted GPU work.
        for _ in 0..6 {
            harness.step();
            harness.render().unwrap();
        }
        let states = test_context.view_states.lock();
        let state = states
            .get(&test_context.recording_store_id, view_id)
            .unwrap();
        let state = state.as_any().downcast_ref::<MapViewState>().unwrap();
        let expected = expected_instance.map(|instance| {
            re_entity_db::InstancePathHash::instance(
                &EntityPath::from("regions"),
                re_log_types::Instance::from(instance),
            )
        });
        assert_eq!(state.last_gpu_picking_result, expected);
    }
    assert_eq!(
        mesh_stats(),
        (2, 2),
        "Idle frames and highlights must reuse GPU meshes"
    );
    harness.hover_at(egui::pos2(-100.0, -100.0));
    test_context.set_time(2);
    harness.run();
    harness.snapshot("geo_polygons_updated");
    assert_eq!(mesh_stats(), (1, 3));
    test_context.set_time(3);
    harness.run();
    harness.snapshot("geo_polygons_recolored");
    assert_eq!(
        mesh_stats(),
        (1, 4),
        "A colors-only update must replace the cached material"
    );
    test_context.set_time(4);
    harness.run();
    harness.snapshot("geo_polygons_cleared");
    assert_eq!(mesh_stats(), (0, 4), "Clearing must release cached meshes");
}
