use super::large_text_button;
use egui::Ui;
use egui_extras::Size;
use re_ui::ReUi;
use re_viewer_context::{CommandSender, SystemCommandSender};
use std::collections::HashMap;

static EXAMPLE_MANIFEST: once_cell::sync::Lazy<Vec<ExampleDesc>> =
    once_cell::sync::Lazy::new(|| {
        // keep only examples with thumbnails
        load_example_manifest()
            .into_iter()
            .filter(|e| e.thumbnail.is_some())
            .collect::<Vec<_>>()
    });

// TODO(emilk/egui#3291): replace this by loading image from the web
static THUMBNAIL_CACHE: once_cell::sync::Lazy<HashMap<&'static str, re_ui::icons::Icon>> =
    once_cell::sync::Lazy::new(|| {
        [
            re_ui::icons::Icon::new(
                "04a244d056f9cfb2ac496830392916d613902def_detect_and_track_objects_480w.png",
                include_bytes!(
                    "../../../data/example_thumbnails/04a244d056f9cfb2ac496830392916d613902def\
                    _detect_and_track_objects_480w.png"
                ),
            ),
            re_ui::icons::Icon::new(
                "8b90a80c72b27fad289806b7e5dff0c9ac97e87c_arkit_scenes_480w.png",
                include_bytes!(
                    "../../../data/example_thumbnails/8b90a80c72b27fad289806b7e5dff0c9ac97e87c\
                    _arkit_scenes_480w.png"
                ),
            ),
            re_ui::icons::Icon::new(
                "033edff752f86bcdc9a81f7877e0b4411ff4e6c5_structure_from_motion_480w.png",
                include_bytes!(
                    "../../../data/example_thumbnails/033edff752f86bcdc9a81f7877e0b4411ff4e6c5\
                    _structure_from_motion_480w.png"
                ),
            ),
            re_ui::icons::Icon::new(
                "277b9c72da1d0d0ae9d221f7552dede9c4d5b2fa_human_pose_tracking_480w.png",
                include_bytes!(
                    "../../../data/example_thumbnails/277b9c72da1d0d0ae9d221f7552dede9c4d5b2fa\
                    _human_pose_tracking_480w.png"
                ),
            ),
            re_ui::icons::Icon::new(
                "b8b25dd01e892e6daf5177e6fc05ff5feb19ee8d_dicom_mri_480w.png",
                include_bytes!(
                    "../../../data/example_thumbnails/b8b25dd01e892e6daf5177e6fc05ff5feb19ee8d\
                    _dicom_mri_480w.png"
                ),
            ),
            re_ui::icons::Icon::new(
                "ca0c72df93d70c79b0e640fb4b7c33cdc0bfe5f4_plots_480w.png",
                include_bytes!(
                    "../../../data/example_thumbnails/ca0c72df93d70c79b0e640fb4b7c33cdc0bfe5f4\
                    _plots_480w.png"
                ),
            ),
        ]
        .into_iter()
        .map(|icon| (icon.id, icon))
        .collect()
    });

#[derive(Debug, serde::Deserialize)]
struct ExampleThumbnail {
    url: String,
    width: u32,
    height: u32,
}

#[derive(Debug, serde::Deserialize)]
#[allow(unused)]
struct ExampleDesc {
    name: String,
    title: String,
    description: String,
    tags: Vec<String>,
    demo_url: String,
    rrd_url: String,
    thumbnail: Option<ExampleThumbnail>,
}

// TODO(#3190): we should attempt to update the manifest based on the online version
fn load_example_manifest() -> Vec<ExampleDesc> {
    serde_json::from_str(include_str!("../../../data/examples_manifest.json"))
        .expect("Failed to parse data/examples_manifest.json")
}

pub(super) fn example_page_ui(
    re_ui: &re_ui::ReUi,
    ui: &mut egui::Ui,
    command_sender: &re_viewer_context::CommandSender,
) {
    //TODO(ab): deduplicate from welcome screen
    let mut margin = egui::Margin::same(40.0);
    margin.bottom = 0.0;
    egui::Frame {
        inner_margin: margin,
        ..Default::default()
    }
    .show(ui, |ui| {
        ui.vertical(|ui| {
            ui.add(
                egui::Label::new(
                    egui::RichText::new("Examples")
                        .strong()
                        .text_style(re_ui::ReUi::welcome_screen_h1()),
                )
                .wrap(false),
            );

            ui.add_space(20.0);

            ui.spacing_mut().item_spacing = egui::vec2(16.0, 16.0);

            // account for margin between the two columns
            let available_width = ui.available_width() - ui.spacing().item_spacing.x;

            let first_column_width =
                egui::remap_clamp(available_width, 390.0..=780.0, 150.0..=300.0).floor();
            let second_column_width =
                egui::remap_clamp(available_width, 390.0..=780.0, 240.0..=480.0).floor();

            // compute all row heights based on the thumbnail size.
            let heights = EXAMPLE_MANIFEST
                .iter()
                .map(|e| {
                    let thumbnail = e
                        .thumbnail
                        .as_ref()
                        .expect("examples without thumbnails are filtered out");
                    let width = thumbnail.width as f32;
                    let height = thumbnail.height as f32;
                    height * second_column_width / width
                })
                .collect::<Vec<_>>();

            let mut strip_builder = egui_extras::StripBuilder::new(ui);
            for height in &heights {
                strip_builder = strip_builder.size(Size::exact(*height));
            }
            strip_builder.vertical(|mut strip| {
                for (example, height) in EXAMPLE_MANIFEST.iter().zip(heights) {
                    strip.strip(|builder| {
                        builder
                            .size(Size::exact(first_column_width))
                            .size(Size::exact(second_column_width))
                            .horizontal(|mut strip| {
                                strip.cell(|ui| {
                                    example_description(ui, example, command_sender);
                                });

                                strip.cell(|ui| {
                                    example_thumbnail(
                                        re_ui,
                                        ui,
                                        example,
                                        egui::vec2(second_column_width, height),
                                    );
                                });
                            });
                    });
                }
            });

            ui.add_space(20.0);
        });
    });
}

fn example_description(ui: &mut Ui, example: &ExampleDesc, command_sender: &CommandSender) {
    ui.label(
        egui::RichText::new(example.title.clone())
            .strong()
            .text_style(re_ui::ReUi::welcome_screen_h3()),
    );

    ui.add(
        egui::Label::new(
            egui::RichText::new(example.description.clone())
                .text_style(re_ui::ReUi::welcome_screen_body()),
        )
        .wrap(true),
    );

    if large_text_button(ui, "Launch example")
        .on_hover_text(format!("Download and open the {} example", &example.title))
        .clicked()
    {
        let data_source = re_data_source::DataSource::RrdHttpUrl(example.rrd_url.clone());
        command_sender.send_system(re_viewer_context::SystemCommand::LoadDataSource(
            data_source,
        ));
    }
}

fn example_thumbnail(re_ui: &ReUi, ui: &mut Ui, example: &ExampleDesc, size: egui::Vec2) {
    // TODO(emilk/egui#3291): pull from web rather than cache
    let file_name = example
        .thumbnail
        .as_ref()
        .expect("examples without thumbnails are filtered out")
        .url
        .split('/')
        .last();

    if let Some(file_name) = file_name {
        if let Some(icon) = THUMBNAIL_CACHE.get(file_name) {
            let image = re_ui.icon_image(icon);
            let texture_id = image.texture_id(ui.ctx());

            let rounding = egui::Rounding::same(8.);
            let resp = ui.add(egui::Image::new(texture_id, size).rounding(rounding));

            ui.painter().rect_stroke(
                resp.rect,
                rounding,
                // TODO(ab): use design tokens
                (1.0, egui::Color32::from_gray(34)),
            );
        }
    }
}
