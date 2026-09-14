use egui::Vec2;
use re_ui::list_item::ListItemContentButtonsExt as _;
use re_ui::modal::{ModalHandler, ModalWrapper};
use re_ui::{UiExt as _, list_item};

#[test]
pub fn test_modal_normal_should_match_snapshot() {
    run_modal_test(
        || {
            ModalWrapper::new("Modal with normal content")
                .min_width(250.0)
                .min_height(350.0)
        },
        |ui| {
            ui.label("Test content");
            ui.separator();
            let mut boolean = true;
            ui.re_checkbox(&mut boolean, "Checkbox");
        },
        "modal_normal",
    );
}

#[test]
pub fn test_modal_list_item_should_match_snapshot() {
    run_modal_test(
        || {
            ModalWrapper::new("Modal with full span content")
                .min_width(250.0)
                .min_height(350.0)
                .full_span_content(true)
        },
        |ui| {
            list_item::list_item_scope(ui, "scope", |ui| {
                ui.list_item_flat_noninteractive(list_item::LabelContent::new("Label content"));
                ui.list_item_flat_noninteractive(
                    list_item::PropertyContent::new("Property content")
                        .value_color(&egui::Color32::RED.to_array())
                        .with_action_button(&re_ui::icons::EDIT, "Edit", || {})
                        .with_always_show_buttons(true),
                );
            });
        },
        "modal_list_item",
    );
}

fn run_modal_test(
    mut make_modal: impl FnMut() -> ModalWrapper,
    mut content_ui: impl FnMut(&mut egui::Ui),
    test_name: &'static str,
) {
    let mut modal_handler = ModalHandler::default();
    modal_handler.open();

    let mut harness =
        re_ui::testing::new_harness(re_ui::testing::TestOptions::Gui, Vec2::new(700.0, 700.0))
            .build_ui(|ui| {
                re_ui::apply_style_and_install_loaders(ui.ctx());

                modal_handler.ui(ui.ctx(), &mut make_modal, |ui| content_ui(ui));
            });

    harness.run();
    harness.snapshot(test_name);
}
