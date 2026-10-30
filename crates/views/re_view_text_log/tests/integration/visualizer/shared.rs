use re_chunk_store::{Chunk, RowId};
use re_log_types::{EntityPath, TimePoint, Timeline};
use re_sdk_types::archetypes::TextLog;
use re_sdk_types::blueprint::encodings::{ComponentSourceKind, VisualizerComponentMapping};
use re_sdk_types::components::{Color, Text};
use re_sdk_types::{ComponentDescriptor, Visualizer};
use re_test_context::VisualizerBlueprintContext as _;

use super::TextLogTestHarness;

#[test]
fn multiple_instructions_map_distinct_components_on_the_same_entity() {
    let first = ComponentDescriptor::partial("custom.first");
    let second = ComponentDescriptor::partial("custom.second");
    let chunk = Chunk::builder("logs")
        .with_component_batches(
            RowId::new(),
            [(Timeline::log_tick(), 10)],
            [
                (TextLog::descriptor_text(), &[Text::from("unmapped")] as _),
                (
                    first.clone(),
                    &[Text::from("first a"), Text::from("first b")] as _,
                ),
                (
                    second.clone(),
                    &[Text::from("second a"), Text::from("second b")] as _,
                ),
            ],
        )
        .build()
        .expect("valid mapped fixture");
    for manifest in [false, true] {
        let mut test = TextLogTestHarness::loaded(std::slice::from_ref(&chunk), 10, manifest);
        test.context.with_blueprint_ctx(|ctx, _| {
            ctx.save_visualizers(
                &EntityPath::from("logs"),
                test.view,
                [
                    (&first, Color::from_rgb(255, 0, 0)),
                    (&second, Color::from_rgb(0, 0, 255)),
                ]
                .into_iter()
                .map(|(source, color)| {
                    Visualizer::new("TextLog")
                        .with_overrides(&TextLog::update_fields().with_color(color))
                        .with_mappings([VisualizerComponentMapping {
                            target: TextLog::descriptor_text().component.as_str().into(),
                            source_kind: ComponentSourceKind::SourceComponent,
                            source_component: Some(source.component.as_str().into()),
                            selector: None,
                        }
                        .into()])
                }),
            );
        });
        test.refresh_blueprint();
        let painted = test.wait_until(|painted| {
            ["first a", "first b", "second a", "second b"]
                .iter()
                .all(|body| painted.contains(body))
        });
        assert!(!painted.contains("unmapped"));
        painted.assert_color("first a", egui::Color32::RED);
        painted.assert_color("first b", egui::Color32::RED);
        painted.assert_color("second a", egui::Color32::BLUE);
        painted.assert_color("second b", egui::Color32::BLUE);
    }
}

#[test]
fn failed_selectors_and_empty_text_batches_settle_without_rows() {
    for empty in [false, true] {
        let texts = if empty {
            Vec::new()
        } else {
            vec!["unprojected body"]
        };
        let chunk = Chunk::builder("logs")
            .with_archetype_auto_row(
                [(Timeline::log_tick(), 10)],
                &TextLog::update_fields().with_many_text(texts),
            )
            .build()
            .expect("valid empty or projected fixture");
        for manifest in [false, true] {
            let mut test = TextLogTestHarness::loaded(std::slice::from_ref(&chunk), 10, manifest);
            if !empty {
                test.context.with_blueprint_ctx(|ctx, _| {
                    ctx.save_visualizers(
                        &EntityPath::from("logs"),
                        test.view,
                        [
                            Visualizer::new("TextLog").with_mappings([
                                VisualizerComponentMapping {
                                    target: TextLog::descriptor_text().component.as_str().into(),
                                    source_kind: ComponentSourceKind::SourceComponent,
                                    source_component: None,
                                    selector: Some(".missing_field".into()),
                                }
                                .into(),
                            ]),
                        ],
                    );
                });
                test.refresh_blueprint();
            }
            let painted = test.wait_until(|painted| !painted.contains("Loading…"));
            assert!(test.missing_chunks().is_empty());
            assert!(!painted.contains("unprojected body"));
        }
    }
}

#[test]
fn color_changes_refresh_display() {
    let timeline = Timeline::log_tick();
    let red = Color::from_rgb(255, 0, 0);
    let blue = Color::from_rgb(0, 0, 255);
    let colors = |color| {
        Chunk::builder("logs")
            .with_archetype_auto_row([(timeline, 0)], &TextLog::update_fields().with_color(color))
            .build()
            .expect("valid test chunk")
    };
    let text = Chunk::builder("logs")
        .with_archetype_auto_row([(timeline, 10)], &TextLog::new("colored"))
        .build()
        .expect("valid test chunk");
    let mut test = TextLogTestHarness::loaded(&[text, colors(red)], 10, false);
    let before = test.wait_until(|painted| painted.contains("colored"));
    before.assert_color("colored", egui::Color32::RED);
    test.context.add_chunks(std::iter::once(colors(blue)));
    let (after, _) = test.frame();
    after.assert_color("colored", egui::Color32::BLUE);
}

#[test]
fn seeking_through_filtered_spans_reaches_visible_rows() {
    let chunks: Vec<_> = (0..8)
        .map(|span| {
            Chunk::builder("logs")
                .with_archetype_auto_row(
                    [(Timeline::log_tick(), span * 100)],
                    &TextLog::new(format!("message {span}")).with_level(if span == 7 {
                        "INFO"
                    } else {
                        "WARN"
                    }),
                )
                .build()
                .unwrap()
        })
        .collect();
    for manifest in [false, true] {
        let mut test = TextLogTestHarness::loaded(&chunks, 0, manifest);
        test.set_level_filter(&["INFO"]);
        let painted = test.wait_until(|painted| painted.contains("message 7"));
        assert_eq!(painted.messages(), ["message 7"]);
        assert!(!painted.contains("Loading…"));
    }
}

#[test]
fn static_text_is_ignored_in_both_query_paths() {
    let chunks = [
        Chunk::builder("logs")
            .with_archetype_auto_row([(Timeline::log_tick(), 100)], &TextLog::new("shadowed"))
            .build()
            .unwrap(),
        Chunk::builder("logs")
            .with_archetype_auto_row(TimePoint::STATIC, &TextLog::new("static body"))
            .build()
            .unwrap(),
        Chunk::builder("temporal")
            .with_archetype_auto_row([(Timeline::log_tick(), 100)], &TextLog::new("visible"))
            .build()
            .unwrap(),
    ];
    for manifest in [false, true] {
        let mut test = TextLogTestHarness::loaded(&chunks, 100, manifest);
        let painted = test.wait_until(|painted| painted.contains("visible"));
        assert!(!painted.contains("static body"));
        assert!(!painted.contains("shadowed"));
    }
}

/// Bootstrapping a level mapped from the text component must not index earlier texts again.
#[test]
fn level_mapped_from_text_does_not_duplicate_rows() {
    let chunks = [0, 100].map(|time| {
        Chunk::builder("logs")
            .with_archetype_auto_row(
                [(Timeline::log_tick(), time)],
                &TextLog::new(format!("message {time}")),
            )
            .build()
            .expect("valid text fixture")
    });
    for manifest in [false, true] {
        let mut test = TextLogTestHarness::loaded(&chunks, 100, manifest);
        test.context.with_blueprint_ctx(|ctx, _| {
            ctx.save_visualizers(
                &EntityPath::from("logs"),
                test.view,
                [
                    Visualizer::new("TextLog").with_mappings([VisualizerComponentMapping {
                        target: TextLog::descriptor_level().component.as_str().into(),
                        source_kind: ComponentSourceKind::SourceComponent,
                        source_component: Some(
                            TextLog::descriptor_text().component.as_str().into(),
                        ),
                        selector: None,
                    }
                    .into()]),
                ],
            );
        });
        test.refresh_blueprint();
        // The level column repeats each mapped text once.
        let expected = ["message 0", "message 0", "message 100", "message 100"];
        let sorted = |painted: &crate::common::PaintedText| {
            let mut messages = painted.messages();
            messages.sort();
            messages
        };
        test.wait_until(|painted| sorted(painted) == expected && !painted.contains("Loading…"));
    }
}

#[test]
fn cursor_seek_survives_preceding_span_refinement() {
    let timeline = Timeline::log_tick();
    let earlier = Chunk::builder("earlier")
        .with_archetype_auto_row(
            [(timeline, 0)],
            &TextLog::update_fields().with_many_text(["earlier"; 1000]),
        )
        .build()
        .expect("valid earlier fixture");
    let target = Chunk::builder("target")
        .with_archetype_auto_row([(timeline, 100)], &TextLog::new("target"))
        .build()
        .expect("valid target fixture");
    for manifest in [false, true] {
        let mut test =
            TextLogTestHarness::loaded(&[earlier.clone(), target.clone()], 100, manifest);
        test.wait_until(|painted| painted.contains("target"));
        assert!(test.missing_chunks().is_empty());
        // Refining the preceding span must not move the target out of the settled viewport.
        test.settle().assert_contains("target");
    }
}
