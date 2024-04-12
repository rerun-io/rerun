use egui::{NumExt as _, Ui};
use ehttp::{fetch, Request};
use itertools::Itertools as _;
use poll_promise::Promise;

use re_viewer_context::{CommandSender, SystemCommand, SystemCommandSender as _};

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

    /// human-readable version of the example name
    title: String,

    tags: Vec<String>,

    rrd_url: String,
    thumbnail: ExampleThumbnail,

    /// URL of the source code in GitHub
    source_url: Option<String>,
}

// TODO(ab): use design tokens
pub(super) const MIN_COLUMN_WIDTH: f32 = 250.0;
const MAX_COLUMN_WIDTH: f32 = 337.0;
const MAX_COLUMN_COUNT: usize = 3;
const COLUMN_HSPACE: f32 = 20.0;
const TITLE_TO_GRID_VSPACE: f32 = 32.0;
const ROW_VSPACE: f32 = 20.0;
const THUMBNAIL_RADIUS: f32 = 12.0;

const CARD_THUMBNAIL_ASPECT_RATIO: f32 = 337.0 / 250.0;

const CARD_DESCRIPTION_HEIGHT: f32 = 130.0;

const DESCRIPTION_INNER_MARGIN: f32 = 20.0;

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

    /// Move the egui cursor to the bottom of this example card.
    fn move_cursor_to_bottom(&self, ui: &mut Ui) {
        let vspace = (self.rect.max.y - ui.cursor().min.y).at_least(0.0);
        ui.add_space(vspace);
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

    if build_info.version.is_rc() || build_info.version.is_release() {
        // If this is versioned as a release or rc, always point to the versioned
        // example manifest. This applies even if doing a local source build.
        format!(
            "https://app.rerun.io/version/{version}/examples_manifest.json",
            version = build_info.version,
        )
    } else {
        // We don't build examples on each PR, so we don't have much to point to except for the nightly examples
        // We could point to the main branch, but it's not always finished building, and so doesn't always work.
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

    /// Draw the example section of the welcome screen.
    ///
    /// Layout:
    /// ```text
    ///      {MIN|MAX}_COLUMN_WIDTH      COLUMN_HSPACE
    /// ◀───────────────────────────────▶◀──▶
    /// ╔═══════════════════════════════╗    ┌────────
    /// ║ THUMBNAIL               ▲     ║    │
    /// ║                         │     ║    │
    /// ║                         │     ║    │
    /// ║                         │     ║    │
    /// ║         CARD_THUMBNAIL_ │     ║    │
    /// ║            ASPECT_RATIO │     ║    │
    /// ║                         │     ║    │
    /// ║                         │     ║    │
    /// ║                         ▼     ║    │
    /// ╠═══════════════════════════════╣    │
    /// ║                         ▲     ║    │
    /// ║   ┌─────────────────────┼─┐   ║    │
    /// ║   │DESCRIPTION          │ │   ║    │
    /// ║   │                     │ │   ║ DESCRIPTION_
    /// ║   │   CARD_DESCRIPTION_ │ │◀─▶║ INNER_
    /// ║   │              HEIGHT │ │   ║ MARGIN
    /// ║   └─────────────────────┼─┘   ║    │
    /// ║                         ▼     ║    │
    /// ╚═══════════════════════════════╝    └────────
    ///   ▲
    ///   │ ROW_VSPACE
    ///   ▼
    /// ┌───────────────────────────────┐    ┌────────
    /// │                               │    │
    /// │                               │    │
    /// ```
    pub(super) fn ui(
        &mut self,
        ui: &mut egui::Ui,
        _re_ui: &re_ui::ReUi,
        command_sender: &CommandSender,
        header_ui: &impl Fn(&mut Ui),
    ) {
        let examples = self
            .examples
            .get_or_insert_with(|| load_manifest(ui.ctx(), self.manifest_url.clone()));

        let Some(examples) = examples.ready_mut() else {
            // Still waiting for example to load

            header_ui(ui); // Always show the header

            ui.separator();

            ui.spinner(); // Placeholder for the examples
            return;
        };

        let examples = match examples {
            Ok(examples) => examples,
            Err(err) => {
                // Examples failed to load.

                header_ui(ui); // Always show the header

                re_log::warn_once!("Failed to load examples: {err}");

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
            .clamp(MIN_COLUMN_WIDTH, MAX_COLUMN_WIDTH);

        ui.horizontal(|ui| {
            // this space is added on the left so that the grid is centered
            let centering_hspace = (ui.available_width()
                - column_count as f32 * column_width
                - (column_count - 1) as f32 * grid_spacing.x)
                .max(0.0)
                / 2.0;
            ui.add_space(centering_hspace);

            ui.vertical(|ui| {
                header_ui(ui);

                ui.add(egui::Label::new(
                    egui::RichText::new("View example recordings")
                        .strong()
                        .line_height(Some(32.0))
                        .text_style(re_ui::ReUi::welcome_screen_h2()),
                ));

                ui.add_space(TITLE_TO_GRID_VSPACE);

                egui::Grid::new("example_section_grid")
                    .spacing(grid_spacing)
                    .min_col_width(column_width)
                    .max_col_width(column_width)
                    .show(ui, |ui| {
                        // Disable text selection so that hovering the example card only hovers the card
                        ui.style_mut().interaction.selectable_labels = false;

                        for row_of_examples in examples.chunks_mut(column_count) {
                            let mut row_example_responses: Vec<egui::Response> = vec![];

                            // Background and thumbnail
                            for example in &mut *row_of_examples {
                                // this is the beginning of the first cell for this example, we can
                                // fully compute its rect now
                                example.rect = egui::Rect::from_min_size(
                                    ui.cursor().min,
                                    egui::vec2(
                                        column_width,
                                        column_width / CARD_THUMBNAIL_ASPECT_RATIO
                                            + CARD_DESCRIPTION_HEIGHT,
                                    ),
                                );

                                let response = ui.interact(
                                    example.rect,
                                    self.id.with(&example.desc.name),
                                    egui::Sense::click(),
                                );

                                // paint background
                                ui.painter().rect_filled(
                                    example.rect,
                                    THUMBNAIL_RADIUS,
                                    //TODO(ab): as per figma, use design tokens instead
                                    egui::Color32::WHITE.gamma_multiply(0.04),
                                );

                                if response.clicked() {
                                    // TODO(#5177): This workaround is needed to avoid the click to "leak"
                                    // through the UI, potentially causing some views (e.g. timeseries or time
                                    // panel to quit auto-zoom mode.
                                    ui.input_mut(|i| i.pointer = Default::default());

                                    let open_in_new_tab = ui.input(|i| i.modifiers.any());
                                    open_example_url(
                                        ui.ctx(),
                                        command_sender,
                                        &example.desc.rrd_url,
                                        open_in_new_tab,
                                    );
                                } else if response.middle_clicked() {
                                    open_example_url(
                                        ui.ctx(),
                                        command_sender,
                                        &example.desc.rrd_url,
                                        true,
                                    );
                                }

                                row_example_responses.push(response);

                                ui.vertical(|ui| example.image_ui(ui, column_width));
                            }

                            ui.end_row();

                            // Title
                            for example in &*row_of_examples {
                                ui.vertical(|ui| example.tile_ui(ui));
                            }

                            ui.end_row();

                            // Tags
                            for example in &*row_of_examples {
                                ui.vertical(|ui| example.tags_ui(ui));
                            }

                            ui.end_row();

                            // Source code link and file size
                            for example in &*row_of_examples {
                                ui.vertical(|ui| {
                                    // The previous row (tags) may take one or two lines, depending
                                    // on wrapping, so we use the bottom of the example card as
                                    // reference to position the source link.
                                    example.move_cursor_to_bottom(ui);
                                    ui.add_space(-DESCRIPTION_INNER_MARGIN - 15.0);

                                    example.github_link_and_size_ui(ui);

                                    // Ensure the egui cursor is moved according to this card's
                                    // geometry.
                                    example.move_cursor_to_bottom(ui);

                                    // Manual spacing between rows.
                                    ui.add_space(ROW_VSPACE);
                                });
                            }

                            // Hover effect
                            for (example, response) in
                                itertools::izip!(&*row_of_examples, row_example_responses)
                            {
                                if response.hovered() {
                                    // We do the hover effect here, last, so we can make the whole card,
                                    // including the image, brighter.
                                    ui.painter().rect_filled(
                                        example.rect,
                                        THUMBNAIL_RADIUS,
                                        //TODO(ab): use design tokens
                                        egui::Color32::from_additive_luminance(25),
                                    );
                                }
                            }

                            ui.end_row();
                        }
                    });
            });
        });
    }
}

#[cfg(target_arch = "wasm32")]
fn open_in_background_tab(egui_ctx: &egui::Context, rrd_url: &str) {
    egui_ctx.open_url(egui::output::OpenUrl {
        url: format!("/?url={}", crate::web_tools::percent_encode(rrd_url)),
        new_tab: true,
    });
}

fn open_example_url(
    _egui_ctx: &egui::Context,
    command_sender: &CommandSender,
    rrd_url: &str,
    _open_in_new_tab: bool,
) {
    #[cfg(target_arch = "wasm32")]
    {
        if _open_in_new_tab {
            open_in_background_tab(_egui_ctx, rrd_url);
            return;
        }
    }

    let data_source = re_data_source::DataSource::RrdHttpUrl(rrd_url.to_owned());

    // If the user re-download an already open recording, clear it out first
    command_sender.send_system(SystemCommand::ClearSourceAndItsStores(
        re_smart_channel::SmartChannelSource::RrdHttpStream {
            url: rrd_url.to_owned(),
        },
    ));

    command_sender.send_system(SystemCommand::LoadDataSource(data_source));

    #[cfg(target_arch = "wasm32")]
    {
        // Ensure that the user returns to the welcome page after navigating to an example.
        use crate::web_tools;

        // So we know where to return to
        let welcome_screen_app_id = re_viewer_context::StoreHub::welcome_screen_app_id();
        let welcome_screen_url = format!(
            "?app_id={}",
            web_tools::percent_encode(&welcome_screen_app_id.to_string())
        );

        if web_tools::current_url_suffix()
            .unwrap_or_default()
            .is_empty()
        {
            // Replace, otherwise the user would need to hit back twice to return to
            // whatever linked them to `https://www.rerun.io/viewer` in the first place.
            web_tools::replace_history(&welcome_screen_url);
        } else {
            web_tools::push_history(&welcome_screen_url);
        }

        // Where we're going:
        web_tools::push_history(&format!("?url={}", web_tools::percent_encode(rrd_url)));
    }
}

impl ExampleDescLayout {
    fn image_ui(&self, ui: &mut Ui, column_width: f32) {
        // dimensions of the source image to use as thumbnail
        let image_width = self.desc.thumbnail.width as f32;
        let image_height = self.desc.thumbnail.height as f32;

        // the thumbnail rect is determined by the column width and a fixed aspect ratio
        let thumbnail_rect = egui::Rect::from_min_size(
            ui.cursor().left_top(),
            egui::vec2(column_width, column_width / CARD_THUMBNAIL_ASPECT_RATIO),
        );
        let thumbnail_width = thumbnail_rect.width();
        let thumbnail_height = thumbnail_rect.height();

        // compute image UV coordinates implementing a "cropping" scale to fit thumbnail rect
        let display_aspect_ratio = thumbnail_width / thumbnail_height;
        let image_aspect_ratio = image_width / image_height;
        let uv_rect = if image_aspect_ratio > display_aspect_ratio {
            let a = (image_width / image_height * thumbnail_height - thumbnail_width)
                / 2.0
                / image_width;
            egui::Rect::from_min_max(egui::Pos2::new(a, 0.0), egui::Pos2::new(1.0 - a, 1.0))
        } else {
            let a = (image_height / image_width * thumbnail_width - thumbnail_height)
                / 2.0
                / image_height;
            egui::Rect::from_min_max(egui::Pos2::new(0.0, a), egui::Pos2::new(1.0, 1.0 - a))
        };

        let rounding = egui::Rounding {
            nw: THUMBNAIL_RADIUS,
            ne: THUMBNAIL_RADIUS,
            sw: 0.0,
            se: 0.0,
        };
        egui::Image::new(&self.desc.thumbnail.url)
            .uv(uv_rect)
            .rounding(rounding)
            .paint_at(ui, thumbnail_rect);
        ui.advance_cursor_after_rect(thumbnail_rect);
    }

    fn tile_ui(&self, ui: &mut Ui) {
        let title = egui::RichText::new(self.desc.title.clone())
            .strong()
            .line_height(Some(16.0))
            .text_style(re_ui::ReUi::welcome_screen_example_title());

        ui.add_space(DESCRIPTION_INNER_MARGIN);
        egui::Frame {
            inner_margin: egui::Margin::symmetric(DESCRIPTION_INNER_MARGIN, 0.0),
            ..Default::default()
        }
        .show(ui, |ui| {
            ui.add(egui::Label::new(title).truncate(true));
        });
    }

    fn tags_ui(&self, ui: &mut Ui) {
        ui.add_space(10.0);

        egui::Frame {
            inner_margin: egui::Margin::symmetric(DESCRIPTION_INNER_MARGIN, 0.0),
            ..Default::default()
        }
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                // TODO(ab): use design tokens
                ui.style_mut().spacing.button_padding = egui::vec2(4.0, 2.0);
                ui.style_mut().spacing.item_spacing = egui::vec2(4.0, 4.0);
                for tag in self.desc.tags.iter().sorted() {
                    ui.add(
                        egui::Button::new(
                            egui::RichText::new(tag).text_style(re_ui::ReUi::welcome_screen_tag()),
                        )
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
        });
    }

    fn github_link_and_size_ui(&self, ui: &mut Ui) {
        let source_url = self.desc.source_url.as_deref();

        egui::Frame {
            inner_margin: egui::Margin::symmetric(DESCRIPTION_INNER_MARGIN, 0.0),
            ..Default::default()
        }
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        source_url.is_some(),
                        egui::Button::image_and_text(
                            re_ui::icons::GITHUB.as_image(),
                            "Source code",
                        ),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .on_disabled_hover_text("Source code is not available for this example")
                    .clicked()
                {
                    if let Some(source_url) = source_url {
                        ui.ctx().open_url(egui::output::OpenUrl {
                            url: source_url.to_owned(),
                            new_tab: true,
                        });
                    }
                }

                if let Some(Some(size)) = self.rrd_byte_size_promise.ready().cloned() {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new(re_format::format_bytes(size as f64)).weak());
                    });
                }
            });
        });
    }
}
