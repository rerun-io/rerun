use egui::{NumExt as _, Ui};

use ehttp::{fetch, Request};
use poll_promise::Promise;
use re_log_types::LogMsg;
use re_smart_channel::ReceiveSet;
use re_viewer_context::SystemCommandSender;

use super::WelcomeScreenResponse;

#[derive(Debug, serde::Deserialize)]
struct ExampleThumbnail {
    url: String,
    width: u32,
    height: u32,
}

#[derive(Debug, serde::Deserialize)]
struct ExampleDesc {
    /// snake_case version of the example name
    name: String,

    /// human readable version of the example name
    title: String,

    description: String,
    tags: Vec<String>,

    rrd_url: String,
    thumbnail: ExampleThumbnail,
}

// TODO(ab): use design tokens
const MIN_COLUMN_WIDTH: f32 = 250.0;
const MAX_COLUMN_WIDTH: f32 = 340.0;
const MAX_COLUMN_COUNT: usize = 3;
const COLUMN_HSPACE: f32 = 24.0;
const TITLE_TO_GRID_VSPACE: f32 = 32.0;
const THUMBNAIL_TO_DESCRIPTION_VSPACE: f32 = 10.0;
const DESCRIPTION_TO_TAGS_VSPACE: f32 = 10.0;
const ROW_VSPACE: f32 = 32.0;
const THUMBNAIL_RADIUS: f32 = 4.0;

/// Structure to track both an example description and its layout in the grid.
///
/// For layout purposes, each example spans multiple cells in the grid. This structure is used to
/// track the rectangle that spans the block of cells used for the corresponding example, so hover/
/// click can be detected.
#[derive(Debug)]
struct ExampleDescLayout {
    desc: ExampleDesc,
    rect: egui::Rect,
}

impl ExampleDescLayout {
    /// Saves the top left corner of the hover/click area for this example.
    fn set_top_left(&mut self, pos: egui::Pos2) {
        self.rect.min = pos;
    }

    /// Saves the bottom right corner of the hover/click area for this example.
    fn set_bottom_right(&mut self, pos: egui::Pos2) {
        self.rect.max = pos;
    }

    fn clicked(&self, ui: &egui::Ui, id: egui::Id) -> bool {
        ui.interact(self.rect, id.with(&self.desc.name), egui::Sense::click())
            .clicked()
    }

    fn hovered(&self, ui: &egui::Ui, id: egui::Id) -> bool {
        ui.interact(self.rect, id.with(&self.desc.name), egui::Sense::hover())
            .hovered()
    }
}

impl From<ExampleDesc> for ExampleDescLayout {
    fn from(desc: ExampleDesc) -> Self {
        ExampleDescLayout {
            desc,
            rect: egui::Rect::NOTHING,
        }
    }
}

type ManifestJson = Vec<ExampleDesc>;
type Manifest = Vec<ExampleDescLayout>;
type ManifestPromise = Promise<Result<Manifest, LoadError>>;

enum LoadError {
    Deserialize(serde_json::Error),
    Fetch(String),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::Deserialize(err) => {
                write!(f, "manifest is invalid, it may be outdated: {err}")
            }
            LoadError::Fetch(err) => f.write_str(err),
        }
    }
}

fn load_manifest(url: String) -> ManifestPromise {
    let (sender, promise) = Promise::new();

    fetch(Request::get(url), move |response| match response {
        Ok(response) => sender.send(
            serde_json::from_slice::<ManifestJson>(&response.bytes)
                .map(|examples| examples.into_iter().map(ExampleDescLayout::from).collect())
                .map_err(LoadError::Deserialize),
        ),
        Err(err) => sender.send(Err(LoadError::Fetch(err))),
    });

    promise
}

pub(super) struct ExamplePage {
    id: egui::Id,
    manifest_url: String,
    examples: Option<ManifestPromise>,
}

fn default_manifest_url() -> String {
    // Sometimes we want the default to point somewhere else, such as when doing nightly builds.
    if let Some(url) = option_env!("DEFAULT_EXAMPLES_MANIFEST_URL") {
        return url.into();
    }

    let build_info = re_build_info::build_info!();

    // Always point to `version/main` for rerun devs,
    // because the current commit's manifest is unlikely to be uploaded to GCS.
    if build_info.is_in_rerun_workspace {
        return "https://app.rerun.io/version/main/examples_manifest.json".into();
    }

    // Otherwise point to the current commit
    let short_sha = build_info.short_git_hash();
    format!("https://app.rerun.io/commit/{short_sha}/examples_manifest.json")
}

impl Default for ExamplePage {
    fn default() -> Self {
        Self {
            id: egui::Id::new("example_page"),
            manifest_url: default_manifest_url(),
            examples: None,
        }
    }
}

impl ExamplePage {
    pub fn set_manifest_url(&mut self, url: String) {
        if self.manifest_url != url {
            self.manifest_url = url.clone();
            self.examples = Some(load_manifest(url));
        }
    }

    pub(super) fn ui(
        &mut self,
        ui: &mut egui::Ui,
        re_ui: &re_ui::ReUi,
        rx: &re_smart_channel::ReceiveSet<re_log_types::LogMsg>,
        command_sender: &re_viewer_context::CommandSender,
    ) -> WelcomeScreenResponse {
        let examples = self
            .examples
            .get_or_insert_with(|| load_manifest(self.manifest_url.clone()));

        let Some(examples) = examples.ready_mut() else {
            ui.spinner();
            return WelcomeScreenResponse::default();
        };

        let examples = match examples {
            Ok(examples) => examples,
            Err(err) => {
                ui.label(re_ui.error_text(format!("Failed to load examples: {err}")));
                return WelcomeScreenResponse::default();
            }
        };

        if examples.is_empty() {
            ui.label("No examples found.");
            return WelcomeScreenResponse::default();
        }

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
        let centering_hspace = (ui.available_width()
            - column_count as f32 * column_width
            - (column_count - 1) as f32 * grid_spacing.x)
            .max(0.0)
            / 2.0;

        ui.horizontal(|ui| {
            ui.add_space(centering_hspace);

            ui.vertical(|ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.add(egui::Label::new(
                        egui::RichText::new("Examples.")
                            .strong()
                            .line_height(Some(32.0))
                            .text_style(re_ui::ReUi::welcome_screen_h1()),
                    ));

                    ui.add(egui::Label::new(
                        egui::RichText::new("Explore what you can build.")
                            .line_height(Some(32.0))
                            .text_style(re_ui::ReUi::welcome_screen_h1()),
                    ));
                });

                ui.add_space(TITLE_TO_GRID_VSPACE);

                egui::Grid::new("example_page_grid")
                    .spacing(grid_spacing)
                    .min_col_width(column_width)
                    .max_col_width(column_width)
                    .show(ui, |ui| {
                        examples
                            .chunks_mut(column_count)
                            .for_each(|example_layouts| {
                                for example in &mut *example_layouts {
                                    // this is the beginning of the first cell for this example
                                    example.set_top_left(ui.cursor().min);

                                    let thumbnail = &example.desc.thumbnail;
                                    let width = thumbnail.width as f32;
                                    let height = thumbnail.height as f32;
                                    ui.vertical(|ui| {
                                        let size =
                                            egui::vec2(column_width, height * column_width / width);

                                        example_thumbnail(
                                            ui,
                                            rx,
                                            &example.desc,
                                            size,
                                            example.hovered(ui, self.id),
                                        );

                                        ui.add_space(THUMBNAIL_TO_DESCRIPTION_VSPACE);
                                    });
                                }

                                ui.end_row();

                                for example in &mut *example_layouts {
                                    ui.vertical(|ui| {
                                        example_description(
                                            ui,
                                            &example.desc,
                                            example.hovered(ui, self.id),
                                        );

                                        ui.add_space(DESCRIPTION_TO_TAGS_VSPACE);
                                    });
                                }

                                ui.end_row();

                                for example in &mut *example_layouts {
                                    ui.vertical(|ui| {
                                        example_tags(ui, &example.desc);

                                        // this is the end of the last cell for this example
                                        example.set_bottom_right(egui::pos2(
                                            ui.cursor().min.x + column_width,
                                            ui.cursor().min.y,
                                        ));

                                        ui.add_space(ROW_VSPACE);
                                    });
                                }

                                ui.end_row();
                            });
                    });

                for example in examples {
                    if example.clicked(ui, self.id) {
                        let data_source =
                            re_data_source::DataSource::RrdHttpUrl(example.desc.rrd_url.clone());
                        command_sender.send_system(
                            re_viewer_context::SystemCommand::LoadDataSource(data_source),
                        );
                    }
                }
            });
        });

        WelcomeScreenResponse::default()
    }
}

fn is_loading(rx: &ReceiveSet<LogMsg>, example: &ExampleDesc) -> bool {
    rx.sources().iter().any(|s| {
        if let re_smart_channel::SmartChannelSource::RrdHttpStream { url } = s.as_ref() {
            url == &example.rrd_url
        } else {
            false
        }
    })
}

fn example_thumbnail(
    ui: &mut Ui,
    rx: &ReceiveSet<LogMsg>,
    example: &ExampleDesc,
    size: egui::Vec2,
    hovered: bool,
) {
    let rounding = egui::Rounding::same(THUMBNAIL_RADIUS);

    let response = ui.add(
        egui::Image::new(&example.thumbnail.url)
            .rounding(rounding)
            .fit_to_exact_size(size),
    );

    // TODO(ab): use design tokens
    let border_color = if hovered {
        ui.visuals_mut().widgets.hovered.fg_stroke.color
    } else {
        egui::Color32::from_gray(44)
    };

    ui.painter()
        .rect_stroke(response.rect, rounding, (1.0, border_color));

    // Show spinner overlay while loading the example:
    if is_loading(rx, example) {
        ui.painter().rect_filled(
            response.rect,
            rounding,
            egui::Color32::BLACK.gamma_multiply(0.75),
        );

        let spinner_size = response.rect.size().min_elem().at_most(72.0);
        let spinner_rect =
            egui::Rect::from_center_size(response.rect.center(), egui::Vec2::splat(spinner_size));
        ui.allocate_ui_at_rect(spinner_rect, |ui| {
            ui.add(egui::Spinner::new().size(spinner_size));
        });
    }
}

fn example_description(ui: &mut Ui, example: &ExampleDesc, hovered: bool) {
    ui.label(
        egui::RichText::new(example.title.clone())
            .strong()
            .line_height(Some(22.0))
            .text_style(re_ui::ReUi::welcome_screen_body()),
    );

    ui.add_space(4.0);

    let mut desc_text = egui::RichText::new(example.description.clone()).line_height(Some(19.0));
    if hovered {
        desc_text = desc_text.strong();
    }

    ui.add(egui::Label::new(desc_text).wrap(true));
}

fn example_tags(ui: &mut Ui, example: &ExampleDesc) {
    // TODO(ab): use design tokens
    ui.horizontal_wrapped(|ui| {
        ui.style_mut().spacing.button_padding = egui::vec2(4.0, 2.0);
        ui.style_mut().spacing.item_spacing = egui::vec2(4.0, 4.0);
        for tag in &example.tags {
            ui.add(
                egui::Button::new(tag)
                    .sense(egui::Sense::hover())
                    .rounding(6.0)
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
