use super::*;
use re_log_types::{TimeInt, TimePoint, Timeline};
use re_sdk_types::RowId;
use re_sdk_types::archetypes::{GeoLineStrings, GeoPoints, GeoPolygons};
use re_sdk_types::components::{Color, GeoLineString, GeoPolygon, Radius};
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
    let polygons = [polygon, overlapping];
    test_context.log_entity("regions", |builder| {
        builder.with_archetype(
            RowId::new(),
            frame(1),
            &GeoPolygons::new(polygons.clone()).with_colors([
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
    let mirrored = polygons.clone().map(|mut polygon| {
        for position in &mut polygon.0.lat_lon {
            position.0[1] = 6.0 - position.0[1];
        }
        polygon
    });
    for (value, polygons) in [(5, mirrored), (6, polygons)] {
        test_context.log_entity("regions", |builder| {
            builder.with_archetype(
                RowId::new(),
                frame(value),
                &GeoPolygons::new(polygons).with_colors([
                    Color::from_unmultiplied_rgba(220, 60, 50, 160),
                    Color::from_unmultiplied_rgba(40, 100, 220, 160),
                ]),
            )
        });
    }
    test_context.log_entity("markers", |builder| {
        builder.with_archetype(
            RowId::new(),
            frame(6),
            &GeoPoints::from_lat_lon([[5.0, 1.0], [5.0, 5.0]])
                .with_colors([
                    Color::from_rgb(255, 0, 255),
                    Color::from_unmultiplied_rgba(220, 60, 50, 160),
                ])
                .with_radii([Radius::new_ui_points(6.0)]),
        )
    });
    test_context.log_entity("boundary", |builder| {
        builder.with_archetype(
            RowId::new(),
            frame(6),
            &GeoLineStrings::from_lat_lon([GeoLineString::from_iter([[0.25, 2.0], [0.25, 5.0]])])
                .with_colors([Color::from_rgb(0, 255, 255)])
                .with_radii([Radius::new_ui_points(3.0)]),
        )
    });
    let view_id = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(MapView::identifier()))
    });
    // Simulate the empty first query of a recording whose chunks arrive asynchronously.
    test_context.set_time(0);
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
    test_context.set_time(1);
    harness.run();
    {
        let states = test_context.view_states.lock();
        let state = states
            .get(&test_context.recording_store_id, view_id)
            .unwrap();
        let state = state.as_any().downcast_ref::<MapViewState>().unwrap();
        assert!(
            state.map_memory.zoom() < 10.0,
            "An initial empty query must not pin the temporary fallback zoom"
        );
    }
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
    let project = |[lat, lon]: [f64; 2]| {
        let states = test_context.view_states.lock();
        let state = states
            .get(&test_context.recording_store_id, view_id)
            .unwrap();
        let state = state.as_any().downcast_ref::<MapViewState>().unwrap();
        let projector =
            walkers::Projector::new(map_rect, &state.map_memory, state.last_center_position);
        let position = projector.project(walkers::lat_lon(lat, lon));
        egui::pos2(position.x, position.y)
    };
    let [hole, outside, red, blue, overlap] =
        [[1.0, 1.0], [5.0, 5.0], [3.0, 0.5], [3.0, 3.0], [3.0, 1.5]].map(project);
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
    let alpha = 160.0 / 255.0;
    for (channel, source) in [220.0, 60.0, 50.0].into_iter().enumerate() {
        let expected = source * alpha + f32::from(pixel(outside).0[channel]) * (1.0 - alpha);
        assert!(
            (f32::from(pixel(red).0[channel]) - expected).abs() <= 4.0,
            "Transparent fill channel {channel} was multiplied by alpha more than once: actual={}, expected={expected}",
            pixel(red).0[channel],
        );
    }
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
    test_context.set_time(5);
    harness.run();
    harness.snapshot("geo_polygons_mirrored_order");
    harness.step();
    let mirrored_image = harness.render().unwrap();
    let mirrored_overlap = project([3.0, 4.5]);
    let mirrored_pixel = mirrored_image.get_pixel(
        (mirrored_overlap.x * scale) as u32,
        (mirrored_overlap.y * scale) as u32,
    );
    for channel in 0..3 {
        assert!(
            (i16::from(mirrored_pixel.0[channel]) - i16::from(pixel(overlap).0[channel])).abs()
                <= 3,
            "Mirroring the same polygons must preserve alpha layer order",
        );
    }
    test_context.set_time(6);
    harness.run();
    harness.snapshot("geo_polygons_point_line_overlays");
    harness.step();
    let overlay_image = harness.render().unwrap();
    let alpha_point = project([5.0, 5.0]);
    let alpha_pixel = overlay_image.get_pixel(
        (alpha_point.x * scale) as u32,
        (alpha_point.y * scale) as u32,
    );
    for (channel, source) in [220.0, 60.0, 50.0].into_iter().enumerate() {
        let expected = source * alpha + f32::from(pixel(outside).0[channel]) * (1.0 - alpha);
        assert!(
            (f32::from(alpha_pixel.0[channel]) - expected).abs() <= 4.0,
            "Alpha-blended point colors must be premultiplied in linear space"
        );
    }
    for (position, color, entity) in [
        ([5.0, 1.0], [255u8, 0, 255], "markers"),
        ([0.25, 3.0], [0u8, 255, 255], "boundary"),
    ] {
        let position = project(position);
        let actual =
            overlay_image.get_pixel((position.x * scale) as u32, (position.y * scale) as u32);
        for (channel, expected) in color.into_iter().enumerate() {
            assert!(
                (i16::from(actual.0[channel]) - i16::from(expected)).abs() <= 3,
                "Point and line overlays must remain above filled polygons"
            );
        }
        harness.hover_at(position);
        for _ in 0..6 {
            harness.step();
            harness.render().unwrap();
        }
        let states = test_context.view_states.lock();
        let state = states
            .get(&test_context.recording_store_id, view_id)
            .unwrap();
        let state = state.as_any().downcast_ref::<MapViewState>().unwrap();
        assert_eq!(
            state.last_gpu_picking_result,
            Some(re_entity_db::InstancePathHash::instance(
                &EntityPath::from(entity),
                re_log_types::Instance::from(0u64)
            ))
        );
    }
}
