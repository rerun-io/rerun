use std::mem;

use re_test_context::TestContext;
use re_test_viewport::TestContextExt as _;
use re_types::blueprint::{archetypes::SpatialInformation, components::Enabled};
use re_viewer_context::ViewClass as _;
use re_viewport_blueprint::{ViewBlueprint, ViewProperty};

fn get_test_context() -> TestContext {
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView3D>();

    // Make sure we can draw stuff in the hover tables.
    test_context.component_ui_registry = re_component_ui::create_component_ui_registry();
    // Also register the legacy UIs.
    re_data_ui::register_component_uis(&mut test_context.component_ui_registry);

    test_context
}

#[test]
fn test_eye_controls() {
    let mut test_context = get_test_context();

    let view_id = test_context.setup_viewport_blueprint(|ctx, blueprint| {
        let view_blueprint =
            ViewBlueprint::new_with_root_wildcard(re_view_spatial::SpatialView3D::identifier());

        let view_id = view_blueprint.id;
        blueprint.add_views(std::iter::once(view_blueprint), None, None);

        let information_property = ViewProperty::from_archetype::<SpatialInformation>(
            ctx.blueprint_db(),
            ctx.blueprint_query,
            view_id,
        );

        information_property.save_blueprint_component(
            ctx,
            &SpatialInformation::descriptor_show_axes(),
            &Enabled::from(true),
        );

        view_id
    });

    let size = egui::vec2(150.0, 150.0);

    let mut harness = test_context
        .setup_kittest_for_rendering_3d(size)
        .build_ui(|ui| test_context.run_with_single_view(ui, view_id));

    let name = "eye_controls";

    let mut cursor_pos = egui::pos2(size.x * 0.5, size.y * 0.5);

    {
        // Fallback view of the axes.
        harness.run();
        harness.snapshot(format!("{name}_0_fallback"));
    }

    harness
        .input_mut()
        .events
        .push(egui::Event::PointerMoved(cursor_pos));

    {
        // Zoom out.
        harness.input_mut().events.push(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -200.0),
            modifiers: egui::Modifiers::default(),
        });
        harness.run_steps(8);
        harness.snapshot(format!("{name}_1_zoom_out"));
    }

    {
        // Zoom in.
        harness.input_mut().events.push(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, 100.0),
            modifiers: egui::Modifiers::default(),
        });
        harness.run_steps(8);
        harness.snapshot(format!("{name}_2_zoom_in"));
    }

    {
        // Rotate by dragging.
        harness.input_mut().events.push(egui::Event::PointerButton {
            pos: cursor_pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::default(),
        });
        cursor_pos += egui::vec2(500.0, 500.0);
        harness
            .input_mut()
            .events
            .push(egui::Event::PointerMoved(cursor_pos));
        harness.input_mut().events.push(egui::Event::PointerButton {
            pos: cursor_pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::default(),
        });
        harness.run();
        harness.snapshot(format!("{name}_3_rotate"));
    }

    {
        // Pan view.
        harness.input_mut().events.push(egui::Event::PointerButton {
            pos: cursor_pos,
            button: egui::PointerButton::Secondary,
            pressed: true,
            modifiers: egui::Modifiers::default(),
        });
        cursor_pos -= egui::vec2(500.0, 0.0);
        harness
            .input_mut()
            .events
            .push(egui::Event::PointerMoved(cursor_pos));
        harness.input_mut().events.push(egui::Event::PointerButton {
            pos: cursor_pos,
            button: egui::PointerButton::Secondary,
            pressed: false,
            modifiers: egui::Modifiers::default(),
        });
        harness.run();
        harness.snapshot(format!("{name}_4_pan"));
    }

    {
        // Reset view.

        // Set double click delay really high to make sure this is a double click.
        let old_delay = harness
            .ctx
            .options_mut(|o| mem::replace(&mut o.input_options.max_double_click_delay, 1000.0));
        for _ in 0..2 {
            harness.input_mut().events.push(egui::Event::PointerButton {
                pos: cursor_pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::default(),
            });
            harness.input_mut().events.push(egui::Event::PointerButton {
                pos: cursor_pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::default(),
            });
        }
        harness.run_steps(8);
        harness.snapshot(format!("{name}_5_reset"));

        harness
            .ctx
            .options_mut(|o| o.input_options.max_double_click_delay = old_delay);
    }

    {
        // Roll view.
        harness.input_mut().events.push(egui::Event::PointerButton {
            pos: cursor_pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::ALT,
        });
        cursor_pos += egui::vec2(500.0, 0.0);
        harness
            .input_mut()
            .events
            .push(egui::Event::PointerMoved(cursor_pos));
        harness.input_mut().events.push(egui::Event::PointerButton {
            pos: cursor_pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::ALT,
        });
        harness.run();
        harness.snapshot(format!("{name}_6_pan"));
    }

    {
        // Move view with key inputs.
        harness.input_mut().events.push(egui::Event::Key {
            key: egui::Key::W,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        });
        harness.run_steps(2);
        harness.snapshot(format!("{name}_7_key_w"));
        harness.input_mut().events.push(egui::Event::Key {
            key: egui::Key::W,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        });

        harness.input_mut().events.push(egui::Event::Key {
            key: egui::Key::S,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        });
        harness.run_steps(3);
        harness.snapshot(format!("{name}_8_key_s"));
        harness.input_mut().events.push(egui::Event::Key {
            key: egui::Key::S,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        });

        harness.input_mut().events.push(egui::Event::Key {
            key: egui::Key::A,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        });
        harness.run_steps(3);
        harness.snapshot(format!("{name}_9_key_a"));
        harness.input_mut().events.push(egui::Event::Key {
            key: egui::Key::A,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        });

        harness.input_mut().events.push(egui::Event::Key {
            key: egui::Key::D,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        });
        harness.run_steps(3);
        harness.snapshot(format!("{name}_10_key_d"));
        harness.input_mut().events.push(egui::Event::Key {
            key: egui::Key::D,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        });

        harness.input_mut().events.push(egui::Event::Key {
            key: egui::Key::Q,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        });
        harness.run_steps(3);
        harness.snapshot(format!("{name}_11_key_q"));
        harness.input_mut().events.push(egui::Event::Key {
            key: egui::Key::Q,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        });

        harness.input_mut().events.push(egui::Event::Key {
            key: egui::Key::E,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        });
        harness.run_steps(3);
        harness.snapshot(format!("{name}_12_key_e"));
    }
}
