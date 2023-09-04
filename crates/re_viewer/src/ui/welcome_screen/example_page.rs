use super::large_text_button;
use egui::{NumExt, Ui};
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
                "efb301d64eef6f25e8f6ae29294bd003c0cda3a7_detect_and_track_objects_480w.png",
                include_bytes!(
                    "../../../data/example_thumbnails/efb301d64eef6f25e8f6ae29294bd003c0cda3a7\
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
            re_ui::icons::Icon::new(
                "ea7a9ab2f716bd37d1bbc1eabf3f55e4f526660e_helix_480w.png",
                include_bytes!(
                    "../../../data/example_thumbnails/ea7a9ab2f716bd37d1bbc1eabf3f55e4f526660e\
                    _helix_480w.png"
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

const MARGINS: f32 = 40.0;
const MIN_COLUMN_WIDTH: f32 = 250.0;
const MAX_COLUMN_WIDTH: f32 = 340.0;
const MAX_COLUMN_COUNT: usize = 3;
const COLUMN_HSPACE: f32 = 22.0;
const TITLE_TO_GRID_VSPACE: f32 = 20.0;
const THUMBNAIL_TO_DESCRIPTION_VSPACE: f32 = 10.0;
const DESCRIPTION_TO_BUTTON_VSPACE: f32 = 4.0;
const ROW_VSPACE: f32 = 40.0;

pub(super) fn example_page_ui(
    re_ui: &re_ui::ReUi,
    ui: &mut egui::Ui,
    command_sender: &re_viewer_context::CommandSender,
) {
    //TODO(ab): deduplicate from welcome screen
    let mut margin = egui::Margin::same(MARGINS);
    margin.bottom = MARGINS - ROW_VSPACE;
    egui::Frame {
        inner_margin: margin,
        ..Default::default()
    }
    .show(ui, |ui| {
        // vertical spacing isn't homogeneous so it's handled manually

        let grid_spacing = egui::vec2(COLUMN_HSPACE, 0.0);
        let column_count = (((ui.available_width() + grid_spacing.x)
            / (MIN_COLUMN_WIDTH + grid_spacing.x))
            .floor() as usize)
            .clamp(1, MAX_COLUMN_COUNT);
        let column_width = ((ui.available_width() + grid_spacing.x) / column_count as f32
            - grid_spacing.x)
            .floor()
            .at_most(MAX_COLUMN_WIDTH);

        // this space is added on the left so that the grid is centered
        let centering_space = (ui.available_width()
            - column_count as f32 * column_width
            - (column_count - 1) as f32 * grid_spacing.x)
            .max(0.0)
            / 2.0;

        ui.horizontal(|ui| {
            ui.add_space(centering_space);

            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new("Examples.")
                                .strong()
                                .text_style(re_ui::ReUi::welcome_screen_h1()),
                        )
                        .wrap(false),
                    );

                    ui.add(
                        egui::Label::new(
                            egui::RichText::new("Learn from the community.")
                                .text_style(re_ui::ReUi::welcome_screen_h1()),
                        )
                        .wrap(false),
                    );
                });

                ui.add_space(TITLE_TO_GRID_VSPACE);

                egui::Grid::new("example_page_grid")
                    .spacing(grid_spacing)
                    .min_col_width(column_width)
                    .max_col_width(column_width)
                    .show(ui, |ui| {
                        EXAMPLE_MANIFEST.chunks(column_count).for_each(|examples| {
                            for example in examples {
                                let thumbnail = example
                                    .thumbnail
                                    .as_ref()
                                    .expect("examples without thumbnails are filtered out");
                                let width = thumbnail.width as f32;
                                let height = thumbnail.height as f32;
                                ui.vertical(|ui| {
                                    example_thumbnail(
                                        re_ui,
                                        ui,
                                        example,
                                        egui::vec2(column_width, height * column_width / width),
                                    );

                                    ui.add_space(THUMBNAIL_TO_DESCRIPTION_VSPACE);
                                });
                            }

                            ui.end_row();

                            for example in examples {
                                ui.vertical(|ui| {
                                    example_description(ui, example);

                                    ui.add_space(DESCRIPTION_TO_BUTTON_VSPACE);
                                });
                            }

                            ui.end_row();

                            for example in examples {
                                ui.vertical(|ui| {
                                    example_tags(ui, example);

                                    ui.add_space(DESCRIPTION_TO_BUTTON_VSPACE);
                                });
                            }

                            ui.end_row();

                            for example in examples {
                                ui.vertical(|ui| {
                                    example_button(ui, example, command_sender);

                                    ui.add_space(ROW_VSPACE);
                                });
                            }

                            ui.end_row();
                        });
                    });
            });
        });
    });
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
                (1.0, egui::Color32::from_gray(44)),
            );
        }
    }
}

fn example_description(ui: &mut Ui, example: &ExampleDesc) {
    ui.label(
        egui::RichText::new(example.title.clone())
            .strong()
            .text_style(re_ui::ReUi::welcome_screen_body()),
    );

    ui.add(egui::Label::new(example.description.clone()).wrap(true));
}

fn example_tags(ui: &mut Ui, example: &ExampleDesc) {
    // TODO(ab): use design tokens
    ui.horizontal_wrapped(|ui| {
        ui.style_mut().spacing.button_padding = egui::vec2(4.0, 2.0);
        for tag in &example.tags {
            ui.add(
                egui::Button::new(tag)
                    .sense(egui::Sense::hover())
                    .fill(egui::Color32::from_rgb(26, 29, 30))
                    .stroke(egui::Stroke::new(
                        1.0,
                        egui::Color32::WHITE.gamma_multiply(0.086),
                    ))
                    .wrap(false),
            );
        }
    });
}

fn example_button(ui: &mut Ui, example: &ExampleDesc, command_sender: &CommandSender) {
    if large_text_button(ui, "Load example recording")
        .on_hover_text(format!(
            "Download and open the {} example's recording",
            &example.title
        ))
        .clicked()
    {
        let data_source = re_data_source::DataSource::RrdHttpUrl(example.rrd_url.clone());
        command_sender.send_system(re_viewer_context::SystemCommand::LoadDataSource(
            data_source,
        ));
    }
}
