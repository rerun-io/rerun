use egui::vec2;
use egui::Color32;
use egui::{NumExt as _, Ui};
use ehttp::{fetch, Request};
use poll_promise::Promise;

use re_ui::icons::ARROW_DOWN;
use re_ui::ReUi;
use re_viewer_context::SystemCommandSender;

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
const THUMBNAIL_TO_DESCRIPTION_VSPACE: f32 = 8.0;
const ROW_VSPACE: f32 = 32.0;
const THUMBNAIL_RADIUS: f32 = 4.0;

/// Structure to track both an example description and its layout in the grid.
///
/// For layout purposes, each example spans multiple cells in the grid. This structure is used to
/// track the rectangle that spans the block of cells used for the corresponding example, so hover/
/// click can be detected.
struct ExampleDescLayout {
    desc: ExampleDesc,
    rect: egui::Rect,

    /// We do an async HEAD request to get the size of the RRD file
    /// so we can show it to the user.
    rrd_byte_size_promise: Promise<Option<u64>>,
}

impl ExampleDescLayout {
    fn new(egui_ctx: &egui::Context, desc: ExampleDesc) -> Self {
        ExampleDescLayout {
            rrd_byte_size_promise: load_file_size(egui_ctx, desc.rrd_url.clone()),
            desc,
            rect: egui::Rect::NOTHING,
        }
    }

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

fn load_manifest(egui_ctx: &egui::Context, url: String) -> ManifestPromise {
    let (sender, promise) = Promise::new();
    let egui_ctx = egui_ctx.clone(); // So we can wake up the ui thread

    fetch(Request::get(url), move |response| {
        match response {
            Ok(response) => sender.send(
                serde_json::from_slice::<ManifestJson>(&response.bytes)
                    .map(|examples| {
                        examples
                            .into_iter()
                            .map(|example| ExampleDescLayout::new(&egui_ctx, example))
                            .collect()
                    })
                    .map_err(LoadError::Deserialize),
            ),
            Err(err) => sender.send(Err(LoadError::Fetch(err))),
        }
        egui_ctx.request_repaint();
    });

    promise
}

/// Do a HEAD request to get the size of a file.
///
/// In case of an error, it is logged as DEBUG and
/// the promise is resolved to `None`.
fn load_file_size(egui_ctx: &egui::Context, url: String) -> Promise<Option<u64>> {
    let (sender, promise) = Promise::new();
    let egui_ctx = egui_ctx.clone(); // So we can wake up the ui thread

    let request = Request {
        method: "HEAD".into(),
        ..Request::get(url.clone())
    };

    fetch(request, move |response| {
        match response {
            Ok(response) => {
                if response.ok {
                    let headers = &response.headers;
                    let content_length = headers
                        .get("content-length")
                        .or_else(|| headers.get("x-goog-stored-content-length"))
                        .and_then(|s| s.parse::<u64>().ok());
                    sender.send(content_length);
                } else {
                    re_log::debug!(
                        "Failed to load file size of {url:?}: {} {}",
                        response.status,
                        response.status_text
                    );
                    sender.send(None);
                }
            }
            Err(err) => {
                re_log::debug!("Failed to load file size of {url:?}: {err}");
                sender.send(None);
            }
        }
        egui_ctx.request_repaint();
    });

    promise
}

pub(super) struct ExampleSection {
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
    let short_sha = build_info.short_git_hash();

    if build_info.version.is_rc() || build_info.version.is_release() {
        // If this is versioned as a release or rc, always point to the versioned
        // example manifest. This applies even if doing a local source build.
        format!(
            "https://app.rerun.io/version/{version}/examples_manifest.json",
            version = build_info.version,
        )
    } else if build_info.is_in_rerun_workspace {
        // Otherwise, always point to `version/nightly` for rerun devs,
        // because the current commit's manifest is unlikely to be uploaded to GCS.
        // We could point to the main branch, but it's not always finished building, and so doesn't always work.
        "https://app.rerun.io/version/nightly/examples_manifest.json".into()
    } else if !short_sha.is_empty() {
        // If we have a sha, try to point at it.
        format!("https://app.rerun.io/commit/{short_sha}/examples_manifest.json")
    } else {
        // If all else fails, point to the nightly branch
        // We could point to the main branch, but it's not always finished building, and so doesn't always work.
        // TODO(#4729): this is better than nothing but still likely to have version
        // compatibility issues.
        "https://app.rerun.io/version/nightly/examples_manifest.json".into()
    }
}

impl Default for ExampleSection {
    fn default() -> Self {
        Self {
            id: egui::Id::new("example_section"),
            manifest_url: default_manifest_url(),
            examples: None,
        }
    }
}

impl ExampleSection {
    pub fn set_manifest_url(&mut self, egui_ctx: &egui::Context, url: String) {
        if self.manifest_url != url {
            self.manifest_url = url.clone();
            self.examples = Some(load_manifest(egui_ctx, url));
        }
    }

    pub(super) fn ui(
        &mut self,
        ui: &mut egui::Ui,
        re_ui: &re_ui::ReUi,
        command_sender: &re_viewer_context::CommandSender,
    ) {
        let examples = self
            .examples
            .get_or_insert_with(|| load_manifest(ui.ctx(), self.manifest_url.clone()));

        let Some(examples) = examples.ready_mut() else {
            ui.spinner();
            return;
        };

        let examples = match examples {
            Ok(examples) => examples,
            Err(err) => {
                ui.label(re_ui.error_text(format!("Failed to load examples: {err}")));
                return;
            }
        };

        if examples.is_empty() {
            ui.label("No examples found.");
            return;
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

        // cursor is currently at the top of the section,
        // so we use it to check for visibility of the whole section.
        let example_section_rect = ui.cursor();
        let examples_visible = ui.is_rect_visible(ui.cursor().translate(vec2(0.0, 16.0)));

        let title_response = ui
            .horizontal(|ui| {
                ui.vertical_centered(|ui| {
                    ui.add(egui::Label::new(
                        egui::RichText::new("Examples")
                            .strong()
                            .line_height(Some(32.0))
                            .text_style(re_ui::ReUi::welcome_screen_h1()),
                    ))
                })
                .inner
            })
            .inner;
        ui.end_row();

        ui.horizontal(|ui| {
            // this space is added on the left so that the grid is centered
            let centering_hspace = (ui.available_width()
                - column_count as f32 * column_width
                - (column_count - 1) as f32 * grid_spacing.x)
                .max(0.0)
                / 2.0;
            ui.add_space(centering_hspace);

            ui.vertical(|ui| {
                ui.add_space(TITLE_TO_GRID_VSPACE);

                egui::Grid::new("example_section_grid")
                    .spacing(grid_spacing)
                    .min_col_width(column_width)
                    .max_col_width(column_width)
                    .show(ui, |ui| {
                        for example_layouts in examples.chunks_mut(column_count) {
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
                                        &example.desc,
                                        size,
                                        example.hovered(ui, self.id),
                                    );
                                });
                            }

                            ui.end_row();

                            for example in &mut *example_layouts {
                                ui.vertical(|ui| {
                                    example_title(ui, example);
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
                        }
                    });

                for example in examples {
                    if example.clicked(ui, self.id) {
                        // TODO(#5177): This workaround is needed to avoid the click to "leak"
                        // through the UI, potentially causing some views (e.g. timeseries or time
                        // panel to quit auto-zoom mode.
                        ui.input_mut(|i| i.pointer = Default::default());

                        let data_source =
                            re_data_source::DataSource::RrdHttpUrl(example.desc.rrd_url.clone());
                        command_sender.send_system(
                            re_viewer_context::SystemCommand::LoadDataSource(data_source),
                        );
                    }
                }
            });
        });

        if !examples_visible {
            let screen_rect = ui.ctx().screen_rect();
            let indicator_rect = example_section_rect
                .with_min_y(screen_rect.bottom() - 125.0)
                .with_max_y(screen_rect.bottom());

            let mut ui = ui.child_ui(
                indicator_rect,
                egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
            );

            ui.vertical_centered(|ui| {
                ui.add_space(16.0);

                ui.scope(|ui| {
                    ui.spacing_mut().button_padding = vec2(16.0, 8.0);
                    let response = ui.add(
                        egui::Button::image_and_text(
                            ARROW_DOWN
                                .as_image()
                                .tint(egui::Color32::BLACK)
                                .fit_to_exact_size(ReUi::small_icon_size()),
                            egui::RichText::new("See examples").color(egui::Color32::BLACK),
                        )
                        .rounding(16.0)
                        .fill(egui::Color32::from_gray(0xfa)),
                    );
                    if response.clicked() {
                        title_response.scroll_to_me(Some(egui::Align::Min));
                    }
                })
            });
        }
    }
}

fn example_thumbnail(
    ui: &mut Ui,
    example: &ExampleDesc,
    thumbnail_size: egui::Vec2,
    hovered: bool,
) {
    const ASPECT_RATIO: f32 = 16.0 / 6.75; // same as `rerun.io/examples`
    const PADDING_PCT: f32 = 0.07; // 7%

    let rounding = egui::Rounding {
        nw: THUMBNAIL_RADIUS,
        ne: THUMBNAIL_RADIUS,
        sw: 0.0,
        se: 0.0,
    };

    let clip_width = thumbnail_size.x;
    let clip_height = thumbnail_size.x / ASPECT_RATIO;
    let padding = thumbnail_size.x * PADDING_PCT;

    let clip_top_left = ui.cursor().left_top();
    let bottom_right = clip_top_left + vec2(clip_width, clip_height);
    let clip_rect = egui::Rect::from_min_max(clip_top_left, bottom_right);

    let thumbnail_top_left = clip_top_left + vec2(padding, 0.0);
    let thumbnail_rect = egui::Rect::from_min_max(
        thumbnail_top_left,
        thumbnail_top_left + thumbnail_size - vec2(padding * 2.0, padding * 2.0 / ASPECT_RATIO),
    );

    // manually clip the rect and paint the image
    let orig_clip_rect = ui.clip_rect();
    ui.set_clip_rect(orig_clip_rect.intersect(clip_rect));
    egui::Image::new(&example.thumbnail.url)
        .rounding(rounding)
        .paint_at(ui, thumbnail_rect);
    ui.advance_cursor_after_rect(clip_rect.expand2(vec2(0.0, THUMBNAIL_TO_DESCRIPTION_VSPACE)));
    ui.set_clip_rect(orig_clip_rect);

    // TODO(ab): use design tokens
    let border_color = if hovered {
        ui.visuals_mut().widgets.hovered.fg_stroke.color
    } else {
        egui::Color32::from_gray(44)
    };

    // paint border
    ui.painter().rect_stroke(
        clip_rect.intersect(thumbnail_rect),
        rounding,
        (1.0, border_color),
    );
    ui.painter().line_segment(
        [clip_rect.left_bottom(), clip_rect.right_bottom()],
        (1.0, border_color),
    );
}

fn example_title(ui: &mut Ui, example: &ExampleDescLayout) {
    let title = egui::RichText::new(example.desc.title.clone())
        .strong()
        .line_height(Some(22.0))
        .color(Color32::from_rgb(178, 178, 187))
        .text_style(re_ui::ReUi::welcome_screen_example_title());

    ui.horizontal(|ui| {
        ui.add(egui::Label::new(title).wrap(true));

        if let Some(Some(size)) = example.rrd_byte_size_promise.ready().cloned() {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(re_format::format_bytes(size as f64));
            });
        }
    });

    ui.add_space(1.0);
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
